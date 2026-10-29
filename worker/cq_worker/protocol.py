"""Line-delimited JSON transport between the CQ-editor GUI and the worker.

Requests arrive on stdin, one JSON object per line. Replies and events leave
through a private duplicate of the original stdout descriptor, also one JSON
object per line.

The worker handles one request at a time, but ``cancel`` and ``interrupt``
requests must reach it while it is busy: :meth:`Channel.cancel_requested` and
the watcher of :meth:`Channel.interruptible` read, without waiting, whatever
the GUI sent meanwhile, record cancellations and interruptions and keep the
other requests, in order, for :meth:`Channel.receive`. Nothing ever blocks
reading stdin besides :meth:`Channel.receive` itself: on Windows, a thread
blocked there would also block DLL loading.

User scripts and native libraries are free to print: descriptor 1 is pointed
at stderr (which the GUI shows in its log), and the Python-level
``sys.stdout`` / ``sys.stderr`` are replaced by :class:`OutputStream` objects
that forward text as ``output`` messages tagged with the request that
produced them.
"""

from __future__ import annotations

import collections
import contextlib
import io
import json
import os
import signal
import sys
import threading
import time
from collections.abc import Iterator
from typing import Any, BinaryIO, cast

Message = dict[str, Any]

#: Bumped whenever the message format changes incompatibly.
PROTOCOL_VERSION = 13

#: How often an interruptible request looks for an ``interrupt`` request.
INTERRUPT_POLL_SECONDS = 0.05

#: Captured output is sent regularly even when a script never writes ``\n``.
OUTPUT_CHUNK_CHARS = 64 * 1024
#: A runaway script cannot fill the GUI and its protocol queue indefinitely.
OUTPUT_CAPTURE_CHARS = 1024 * 1024
#: One Python logging record may include a traceback, but not arbitrary data.
LOG_MESSAGE_CHARS = 256 * 1024
#: Repeated logging calls share a per-request ceiling as well.
LOG_CAPTURE_CHARS = 1024 * 1024

_UNSET = object()


def truncate_text(text: str, limit: int, kind: str) -> str:
    """Keep a text protocol field bounded and make the data loss explicit."""

    if len(text) <= limit:
        return text
    return text[:limit] + f"\n[{kind} truncated after {limit} characters]\n"


class Channel:
    """Bidirectional message pipe to the GUI."""

    def __init__(self, reader: BinaryIO, writer: BinaryIO) -> None:
        self._reader = reader
        self._writer = writer
        self._lock = threading.Lock()
        #: Id of the request being handled; attached to output and log messages.
        self.current_id: int | None = None
        #: Requests the GUI asked to cancel, or to interrupt.
        self._cancelled: set[Any] = set()
        self._interrupted: set[Any] = set()
        #: Requests (or the errors parsing them) read ahead while polling,
        #: in arrival order.
        self._pending: collections.deque[Message | Exception] = collections.deque()
        self._partial = b""
        self._ended = False
        #: Guards the reading state: an interruptible request's watcher
        #: polls from its own thread.
        self._read_lock = threading.Lock()
        #: The main thread is blocked reading; nobody else may read then.
        self._receiving = False
        # Streams with a descriptor are read with os.read, so that what was
        # read is never hidden in a buffered reader's buffer; others (tests)
        # are read line by line and never polled.
        try:
            self._fd: int | None = reader.fileno()
        except (AttributeError, OSError, io.UnsupportedOperation):
            self._fd = None

    @classmethod
    def from_std_streams(cls) -> Channel:
        """Take over the process' standard streams for the protocol."""

        sys.stdout.flush()
        protocol_fd = os.dup(1)
        # Anything written to descriptor 1 from now on (OpenCascade warnings,
        # C extensions, child processes) lands in stderr instead of
        # corrupting the message stream.
        os.dup2(2, 1)
        return cls(sys.stdin.buffer, os.fdopen(protocol_fd, "wb"))

    def send(self, message: Message) -> None:
        data = json.dumps(
            message, ensure_ascii=False, separators=(",", ":"), allow_nan=False
        ).encode("utf-8")
        with self._lock:
            self._writer.write(data + b"\n")
            self._writer.flush()

    def receive(self) -> Message | None:
        """Block until the next request arrives; ``None`` once the GUI is gone."""

        while True:
            with self._read_lock:
                if self._pending:
                    message = self._pending.popleft()
                    if isinstance(message, Exception):
                        raise message
                    return message
                if self._ended:
                    return None
                self._receiving = True
            data = b""
            try:
                if self._fd is None:
                    data = self._reader.readline()
                else:
                    data = os.read(self._fd, 65536)
            finally:
                with self._read_lock:
                    self._receiving = False
                    if self._fd is None:
                        if data:
                            self._take_line(data)
                        else:
                            self._ended = True
                    else:
                        self._take(data)

    def cancel_requested(self, request_id: Any) -> bool:
        """Whether the GUI cancelled ``request_id``, reading without waiting
        whatever it has sent meanwhile."""

        self._poll()
        return request_id in self._cancelled

    def forget_cancel(self, request_id: Any) -> None:
        self._cancelled.discard(request_id)

    def interrupt_requested(self, request_id: Any) -> bool:
        """Whether the GUI asked to interrupt ``request_id``, reading without
        waiting whatever it has sent meanwhile."""

        self._poll()
        return request_id in self._interrupted

    @contextlib.contextmanager
    def interruptible(self, request_id: Any) -> Iterator[None]:
        """While handling ``request_id``, an ``interrupt`` request raises
        ``KeyboardInterrupt`` in the main thread, as Ctrl+C would: pure
        Python code stops at once, native code when it returns to Python.

        A thread polls for it every :data:`INTERRUPT_POLL_SECONDS`. An
        interrupt raised after the request ended is absorbed here, so it
        can never land in the next one."""

        stop = threading.Event()

        def watch() -> None:
            while not stop.wait(INTERRUPT_POLL_SECONDS):
                if self.interrupt_requested(request_id):
                    _interrupt_main_thread()
                    return

        watcher = threading.Thread(target=watch, name="interrupts", daemon=True)
        watcher.start()
        try:
            yield
        finally:
            while True:
                try:
                    stop.set()
                    watcher.join()
                    # Raises an interrupt the watcher sent too late.
                    time.sleep(0)
                    break
                except KeyboardInterrupt:
                    continue
            self._interrupted.discard(request_id)

    def _poll(self) -> None:
        with self._read_lock:
            while not self._ended and not self._receiving and self._fd is not None:
                available = _bytes_available(self._fd)
                if available == 0:
                    return
                self._take(os.read(self._fd, max(available, 1)))

    def _take(self, data: bytes) -> None:
        if not data:
            self._ended = True
            if self._partial.strip():
                self._take_line(self._partial)
            self._partial = b""
            return
        *lines, self._partial = (self._partial + data).split(b"\n")
        for line in lines:
            self._take_line(line)

    def _take_line(self, line: bytes) -> None:
        """Records a cancellation or an interruption at once; queues
        anything else."""

        line = line.strip()
        if not line:
            return
        try:
            message = json.loads(line)
        except ValueError as error:
            self._pending.append(error)
            return
        command = message.get("cmd") if isinstance(message, dict) else None
        if command == "cancel":
            self._cancelled.add(message.get("target"))
        elif command == "interrupt":
            self._interrupted.add(message.get("target"))
        else:
            self._pending.append(message)


def _interrupt_main_thread() -> None:
    """Sends SIGINT to the main thread, as Ctrl+C would: Python raises
    ``KeyboardInterrupt`` there, and waits such as ``time.sleep`` wake up.

    ``_thread.interrupt_main`` is not enough: on Windows it does not set the
    event those waits watch, which only the C signal handler does."""

    if os.name == "nt":
        # Runs the handler in this thread; Python acts on it in the main one.
        signal.raise_signal(signal.SIGINT)
    else:
        # The main thread itself, so its blocking calls return with EINTR.
        main_ident = threading.main_thread().ident
        if main_ident is not None:
            getattr(signal, "pthread_kill")(main_ident, signal.SIGINT)


def _bytes_available(fd: int) -> int:
    """How much can be read from ``fd`` right away (1 at the end of the
    stream, which reads as empty); 0 when that cannot be told. A thread
    blocked reading stdin is not an option on Windows: loading a DLL then
    waits for that read to finish."""

    if os.name == "nt":
        try:
            import _winapi
            import msvcrt

            # Python versions disagree on the number of trailing values;
            # the first is the byte count available without blocking.
            available = _winapi.PeekNamedPipe(msvcrt.get_osfhandle(fd), 0)[0]  # type: ignore[attr-defined]
            return cast(int, available)
        except OSError as error:
            # The other end is closed: the next read reports the end.
            return 1 if getattr(error, "winerror", None) == 109 else 0
    import select

    try:
        readable, _, _ = select.select([fd], [], [], 0)
    except (OSError, ValueError):
        return 0
    return 1 if readable else 0


class OutputStream(io.TextIOBase):
    """Text stream that forwards writes to the GUI as ``output`` messages.

    Text is buffered until a newline, an explicit flush or the chunk limit.
    Each request also has a total capture limit so runaway output cannot grow
    the worker, protocol queue or GUI indefinitely.
    """

    def __init__(self, channel: Channel, name: str) -> None:
        super().__init__()
        self._channel = channel
        self._name = name
        self._pending: list[str] = []
        self._pending_chars = 0
        self._captured_chars = 0
        self._request_id: int | None | object = _UNSET
        self._truncated = False
        self._lock = threading.RLock()

    @property
    def encoding(self) -> str:  # type: ignore[override]
        return "utf-8"

    def writable(self) -> bool:
        return True

    def write(self, text: str) -> int:
        if not isinstance(text, str):
            raise TypeError(f"write() argument must be str, not {type(text).__name__}")
        if not text:
            return 0
        with self._lock:
            self._sync_request()
            if self._truncated:
                return len(text)

            remaining = OUTPUT_CAPTURE_CHARS - self._captured_chars
            accepted = text[:remaining]
            was_truncated = len(text) > len(accepted)
            self._captured_chars += len(accepted)
            while accepted:
                room = OUTPUT_CHUNK_CHARS - self._pending_chars
                part, accepted = accepted[:room], accepted[room:]
                self._pending.append(part)
                self._pending_chars += len(part)
                if "\n" in part or self._pending_chars == OUTPUT_CHUNK_CHARS:
                    self._flush_pending()

            if was_truncated:
                self._truncate()
        return len(text)

    def flush(self) -> None:
        with self._lock:
            self._sync_request()
            self._flush_pending()

    def _sync_request(self) -> None:
        request_id = self._channel.current_id
        if self._request_id is _UNSET:
            self._request_id = request_id
        elif self._request_id != request_id:
            self._flush_pending()
            self._request_id = request_id
            self._captured_chars = 0
            self._truncated = False

    def _truncate(self) -> None:
        self._flush_pending()
        self._send(
            f"\n[output truncated after {OUTPUT_CAPTURE_CHARS} characters; "
            "further characters from this request were discarded]\n"
        )
        self._truncated = True

    def _flush_pending(self) -> None:
        if not self._pending:
            return
        text = "".join(self._pending)
        self._pending.clear()
        self._pending_chars = 0
        self._send(text)

    def _send(self, text: str) -> None:
        try:
            self._channel.send(
                {
                    "type": "output",
                    "id": self._request_id,
                    "stream": self._name,
                    "text": text,
                }
            )
        except (OSError, ValueError):
            # The GUI side is gone (typically during interpreter shutdown).
            pass


def redirect_std_streams(channel: Channel) -> None:
    """Route Python-level standard streams through the protocol."""

    sys.stdout = OutputStream(channel, "stdout")
    sys.stderr = OutputStream(channel, "stderr")
    # stdin belongs to the protocol: a script calling input() must not eat
    # the next request, so it gets an empty stream (input() raises EOFError).
    sys.stdin = io.StringIO()
