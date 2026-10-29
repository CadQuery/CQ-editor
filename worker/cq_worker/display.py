"""Rich results in the console: Jupyter-style MIME bundles (FEAT-E6).

An object's representations are gathered the way Jupyter does
(``_repr_mimebundle_``, then ``_repr_png_``, ``_repr_jpeg_``…), plus
matplotlib figures. Only what the GUI shows leaves the worker, checked and
bounded here: one PNG or JPEG image (recognized by its signature, at most
:data:`MAX_IMAGE_BYTES`) and the plain text. HTML, Markdown, SVG, LaTeX and
JSON are never sent, so no markup or script reaches the GUI. A broken or
slow ``_repr_*_`` only costs the worker (Stop interrupts it); the GUI decodes
images on its reader thread, never on the one that draws.
"""

from __future__ import annotations

import base64
import binascii
import io
import sys
import threading
from collections.abc import Callable
from typing import Any

#: Plain text sent per result.
MAX_TEXT_CHARS = 64 * 1024
#: Image bytes sent per result (before base64).
MAX_IMAGE_BYTES = 4 * 1024 * 1024
#: Images sent per console command; later ones are only described.
MAX_IMAGES_PER_REQUEST = 20
#: Rich results sent per console command. This also bounds text-only loops.
MAX_RESULTS_PER_REQUEST = 100

_SIGNATURES = {
    "png": b"\x89PNG\r\n\x1a\n",
    "jpeg": b"\xff\xd8\xff",
}


def mime_bundle(value: Any) -> dict[str, Any]:
    """The representations a Jupyter front end would get for ``value``;
    ``text/plain`` is always there."""

    data: dict[str, Any] = {}
    method = _bound(value, "_repr_mimebundle_")
    if method is not None:
        result = _call(method, include=None, exclude=None)
        result = _representation(result)
        if isinstance(result, dict):
            data.update(result)
    for mime, name in [("image/png", "_repr_png_"), ("image/jpeg", "_repr_jpeg_")]:
        if mime not in data:
            method = _bound(value, name)
            result = _call(method) if method is not None else None
            result = _representation(result)
            if result is not None and result is not _FAILED:
                data[mime] = result
    if "image/png" not in data and _is_matplotlib_figure(value):
        buffer = io.BytesIO()
        if _call(value.savefig, buffer, format="png", dpi=100) is not _FAILED:
            data["image/png"] = buffer.getvalue()
    # A malformed bundle must not suppress the ordinary representation.
    if not isinstance(data.get("text/plain"), str):
        plain = _call(repr, value)
        data["text/plain"] = object.__repr__(value) if plain is _FAILED else plain
    return data


def message(data: dict[str, Any], images_left: int) -> dict[str, Any]:
    """The ``display`` message fields for a bundle: ``text`` and, when
    ``images_left`` allows, an ``image`` (``format``, base64 ``data``)."""

    text = data.get("text/plain")
    text = text if isinstance(text, str) else ""
    image = None
    for mime, kind in [("image/png", "png"), ("image/jpeg", "jpeg")]:
        raw = _image_bytes(data.get(mime))
        if raw is _TOO_LARGE:
            text += f"\n[{kind.upper()} image left out: over the limit]"
            continue
        if not isinstance(raw, bytes) or not raw.startswith(_SIGNATURES[kind]):
            continue
        if len(raw) > MAX_IMAGE_BYTES:
            text += f"\n[{kind.upper()} image of {len(raw) // 1024} KB left out: over the limit]"
        elif images_left <= 0:
            text += f"\n[{kind.upper()} image left out: too many images in one command]"
        else:
            image = {"format": kind, "data": base64.b64encode(raw).decode("ascii")}
        break
    if len(text) > MAX_TEXT_CHARS:
        text = text[:MAX_TEXT_CHARS] + f"\n[text cut after {MAX_TEXT_CHARS} characters]"
    return {"text": text, "image": image}


class Displayer:
    """Sends ``display`` messages for one console command at a time."""

    def __init__(self, send: Callable[[dict], None]) -> None:
        self._send = send
        self._images_left = MAX_IMAGES_PER_REQUEST
        self._results_left = MAX_RESULTS_PER_REQUEST
        self._result_limit_reported = False
        self._request_id: int | None = None
        self._owner_thread: int | None = None
        self._active = False

    def start(self, request_id: int | None) -> None:
        """Begin a command, binding all its results to ``request_id``."""

        self._images_left = MAX_IMAGES_PER_REQUEST
        self._results_left = MAX_RESULTS_PER_REQUEST
        self._result_limit_reported = False
        self._request_id = request_id
        self._owner_thread = threading.get_ident()
        self._active = True

    def finish(self) -> None:
        """End the command so retained/background ``display`` calls are inert."""

        self._active = False
        self._request_id = None
        self._owner_thread = None

    def display(self, *values: Any) -> None:
        """Shows each value in the console, an image included."""

        # User code may retain ``display`` and call it from a background
        # thread or after the command ended. Such a result cannot safely be
        # associated with a request and must not be mislabeled as the next one.
        if not self._active or threading.get_ident() != self._owner_thread:
            return
        # What was printed before comes first.
        sys.stdout.flush()
        sys.stderr.flush()
        for value in values:
            if self._results_left <= 0:
                if not self._result_limit_reported:
                    self._emit(
                        {
                            "type": "display",
                            "text": (
                                "[rich output truncated after "
                                f"{MAX_RESULTS_PER_REQUEST} results; "
                                "further results from this command were discarded]"
                            ),
                            "image": None,
                        }
                    )
                    self._result_limit_reported = True
                break
            fields = message(mime_bundle(value), self._images_left)
            self._results_left -= 1
            if fields["image"] is not None:
                self._images_left -= 1
            self._emit({"type": "display", **fields})

    def _emit(self, fields: dict[str, Any]) -> None:
        self._send({"id": self._request_id, **fields})

    def displayhook(self, value: Any) -> None:
        """``sys.displayhook`` while a console command runs: the value of
        an expression statement, shown like ``display`` and kept in ``_``."""

        if value is None:
            return
        import builtins

        setattr(builtins, "_", None)
        self.display(value)
        setattr(builtins, "_", value)


_FAILED = object()
_TOO_LARGE = object()


def _call(function: Callable, *args: Any, **kwargs: Any) -> Any:
    """``function(...)``, or :data:`_FAILED` if it raises (a broken
    representation must not hide the result)."""

    try:
        return function(*args, **kwargs)
    except Exception:
        return _FAILED


def _bound(value: Any, name: str) -> Callable | None:
    """The method ``name`` of an instance; classes have them unbound."""

    if isinstance(value, type):
        return None
    try:
        method = getattr(value, name, None)
    except Exception:
        return None
    return method if callable(method) else None


def _representation(value: Any) -> Any:
    """The data part of a Jupyter ``(data, metadata)`` representation."""

    if isinstance(value, tuple):
        return value[0] if value else _FAILED
    return value


def _is_matplotlib_figure(value: Any) -> bool:
    figure = sys.modules.get("matplotlib.figure")
    return figure is not None and isinstance(value, figure.Figure)


def _image_bytes(value: Any) -> bytes | object | None:
    """Image bytes, rejected before decoding/copying when already too large.

    Bundles hold base64 text while ``_repr_png_`` normally returns bytes.
    Checking the encoded length first prevents a hostile representation from
    making the worker allocate an arbitrarily large decoded buffer.
    """

    if isinstance(value, (bytes, bytearray)):
        if len(value) > MAX_IMAGE_BYTES:
            return _TOO_LARGE
        return bytes(value)
    if isinstance(value, str):
        encoded_limit = 4 * ((MAX_IMAGE_BYTES + 2) // 3)
        if len(value) > encoded_limit:
            return _TOO_LARGE
        try:
            raw = base64.b64decode(value, validate=True)
        except (binascii.Error, ValueError):
            return None
        return raw if len(raw) <= MAX_IMAGE_BYTES else _TOO_LARGE
    return None
