"""Wire format and stream isolation regressions."""

import io
import json
import os
import time

import pytest

from cq_worker.protocol import (
    OUTPUT_CAPTURE_CHARS,
    OUTPUT_CHUNK_CHARS,
    Channel,
    OutputStream,
)


def messages(writer):
    return [json.loads(line) for line in writer.getvalue().splitlines()]


def test_receive_skips_empty_lines_and_returns_none_at_eof():
    channel = Channel(io.BytesIO(b'\n  \n{"cmd":"run","id":4}\n'), io.BytesIO())
    assert channel.receive() == {"cmd": "run", "id": 4}
    assert channel.receive() is None


def test_receive_rejects_malformed_json():
    channel = Channel(io.BytesIO(b"{broken}\n"), io.BytesIO())
    with pytest.raises(json.JSONDecodeError):
        channel.receive()


def test_send_writes_one_utf8_message_per_line():
    writer = io.BytesIO()
    channel = Channel(io.BytesIO(), writer)
    channel.send({"type": "output", "text": "ação\n新"})
    channel.send({"type": "ready", "id": 3})
    assert messages(writer) == [
        {"type": "output", "text": "ação\n新"},
        {"type": "ready", "id": 3},
    ]
    assert writer.getvalue().count(b"\n") == 2


def test_send_rejects_non_json_floats_without_writing():
    writer = io.BytesIO()
    channel = Channel(io.BytesIO(), writer)
    with pytest.raises(ValueError):
        channel.send({"value": float("nan")})
    assert writer.getvalue() == b""


def test_output_stream_buffers_fragments_until_newline():
    writer = io.BytesIO()
    channel = Channel(io.BytesIO(), writer)
    channel.current_id = 17
    stream = OutputStream(channel, "stdout")
    assert stream.writable() and stream.encoding == "utf-8"
    assert stream.write("first ") == 6
    assert writer.getvalue() == b""
    stream.write("line\n")
    assert messages(writer) == [
        {"type": "output", "id": 17, "stream": "stdout", "text": "first line\n"}
    ]


def test_output_stream_flushes_partial_line_and_ignores_empty_write():
    writer = io.BytesIO()
    stream = OutputStream(Channel(io.BytesIO(), writer), "stderr")
    assert stream.write("") == 0
    stream.flush()
    assert writer.getvalue() == b""
    stream.write("partial")
    stream.flush()
    stream.flush()
    assert messages(writer) == [
        {"type": "output", "id": None, "stream": "stderr", "text": "partial"}
    ]


def test_output_stream_flushes_by_size_without_waiting_for_a_newline():
    writer = io.BytesIO()
    stream = OutputStream(Channel(io.BytesIO(), writer), "stdout")
    text = "x" * (OUTPUT_CHUNK_CHARS + 1)

    stream.write(text)

    assert [message["text"] for message in messages(writer)] == [text[:-1]]
    stream.flush()
    assert "".join(message["text"] for message in messages(writer)) == text


def test_output_stream_truncates_excessive_output_per_request():
    writer = io.BytesIO()
    channel = Channel(io.BytesIO(), writer)
    channel.current_id = 4
    stream = OutputStream(channel, "stdout")
    limit = OUTPUT_CAPTURE_CHARS

    text = "x" * (limit + 10)
    assert stream.write(text) == len(text)
    stream.write("discarded too")
    stream.flush()

    first_request = [message["text"] for message in messages(writer)]
    assert sum(part.count("x") for part in first_request) == limit
    assert sum("output truncated" in part for part in first_request) == 1

    channel.current_id = 5
    stream.write("new request\n")
    assert messages(writer)[-1]["id"] == 5
    assert messages(writer)[-1]["text"] == "new request\n"


def test_output_stream_requires_text():
    stream = OutputStream(Channel(io.BytesIO(), io.BytesIO()), "stdout")
    with pytest.raises(TypeError):
        stream.write(b"bytes")


@pytest.mark.parametrize("error", [BrokenPipeError, ValueError])
def test_output_stream_survives_disconnected_gui(error):
    class BrokenWriter:
        def write(self, _data):
            raise error("gone")

    stream = OutputStream(Channel(io.BytesIO(), BrokenWriter()), "stdout")
    stream.write("done\n")
    stream.flush()


def test_cancellations_are_seen_while_a_request_is_handled():
    read_fd, write_fd = os.pipe()
    with os.fdopen(read_fd, "rb", buffering=0) as reader:
        channel = Channel(reader, io.BytesIO())
        os.write(write_fd, b'{"cmd":"run","id":1}\n')
        assert channel.receive() == {"cmd": "run", "id": 1}
        # Nothing more has arrived: asking does not wait.
        assert not channel.cancel_requested(1)

        os.write(
            write_fd,
            b'{"cmd":"inspect","id":2}\n{"cmd":"cancel","target":1}\n{"cmd":"canc',
        )
        assert channel.cancel_requested(1)
        os.write(write_fd, b'el","target":2}\n')
        assert channel.cancel_requested(2)
        channel.forget_cancel(1)
        assert not channel.cancel_requested(1)

        # What was read meanwhile is served in order, then the end.
        assert channel.receive() == {"cmd": "inspect", "id": 2}
        os.write(write_fd, b'{"cmd":"shutdown"}')
        os.close(write_fd)
        assert not channel.cancel_requested(3)
        assert channel.receive() == {"cmd": "shutdown"}
        assert channel.receive() is None
        assert channel.receive() is None


def test_malformed_requests_are_raised_in_order():
    payload = b'{"cmd":"a"}\n{broken}\n{"cmd":"b"}\n'
    channel = Channel(io.BytesIO(payload), io.BytesIO())

    assert channel.receive() == {"cmd": "a"}
    with pytest.raises(json.JSONDecodeError):
        channel.receive()
    assert channel.receive() == {"cmd": "b"}


def test_cancellations_are_recorded_between_requests_too():
    payload = b'{"cmd":"cancel","target":9}\n{"cmd":"run","id":9}\n'
    channel = Channel(io.BytesIO(payload), io.BytesIO())
    assert channel.receive() == {"cmd": "run", "id": 9}
    assert channel.cancel_requested(9)


def test_an_interrupt_stops_the_request_it_names_and_no_other():
    channel = Channel(io.BytesIO(b'{"cmd":"interrupt","target":3}\n'), io.BytesIO())
    assert channel.receive() is None, "recorded, not handed out"

    with channel.interruptible(4):
        time.sleep(0.3)  # not this request
    with pytest.raises(KeyboardInterrupt):
        with channel.interruptible(3):
            while True:
                pass
    assert not channel.interrupt_requested(3), "forgotten once handled"


def test_an_interrupt_arriving_during_a_request_raises_in_it():
    read_fd, write_fd = os.pipe()
    with os.fdopen(read_fd, "rb", buffering=0) as reader:
        channel = Channel(reader, io.BytesIO())
        with pytest.raises(KeyboardInterrupt):
            with channel.interruptible(5):
                os.write(
                    write_fd,
                    b'{"cmd":"inspect","id":6}\n{"cmd":"interrupt","target":5}\n',
                )
                while True:
                    pass
        # What arrived with it is kept for later, in order.
        os.write(write_fd, b'{"cmd":"shutdown"}\n')
        os.close(write_fd)
        assert channel.receive() == {"cmd": "inspect", "id": 6}
        assert channel.receive() == {"cmd": "shutdown"}
