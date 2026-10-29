"""The line of the script that created each CadQuery object, for the GUI's
*Go to source*.

While a script runs, :func:`recording` wraps the constructors of
``Workplane`` (every chained operation creates one through ``newObject``),
``Shape`` and its subclasses, ``Assembly`` (``add`` creates the child nodes)
and ``Sketch``. Each new object remembers the line of the innermost frame
of the script: an object made inside a function of the script points into
that function, one made by a helper module points to the script line that
called it. The constructors are restored when the run ends, whatever
happens, so nothing else pays for it.
"""

from __future__ import annotations

import contextlib
import functools
import sys
import types
from collections.abc import Iterator
from typing import Any

import cadquery as cq

#: Attribute holding the 1-based script line on recorded objects.
LINE_ATTRIBUTE = "_cq_editor_line"


def creation_line(obj: Any) -> int | None:
    """The script line that created ``obj``, if it was recorded."""

    line = getattr(obj, "__dict__", {}).get(LINE_ATTRIBUTE)
    return line if isinstance(line, int) else None


def _recorded_classes() -> list[type]:
    """The classes whose ``__init__`` creates script objects: those that
    define one themselves, so every constructor runs through a wrapper."""

    classes: list[type] = []
    pending = [cq.Workplane, cq.Shape, cq.Assembly, cq.Sketch]
    while pending:
        cls = pending.pop()
        if cls in classes:
            continue
        if "__init__" in cls.__dict__:
            classes.append(cls)
        pending.extend(cls.__subclasses__())
    return classes


@contextlib.contextmanager
def recording(filename: str) -> Iterator[None]:
    """Records creation lines in the code of ``filename`` while active."""

    def script_line() -> int | None:
        frame: types.FrameType | None = sys._getframe(2)
        while frame is not None:
            if frame.f_code.co_filename == filename:
                return frame.f_lineno
            frame = frame.f_back
        return None

    def wrapped(original):
        @functools.wraps(original)
        def __init__(self, *args, **kwargs):
            original(self, *args, **kwargs)
            attributes = getattr(self, "__dict__", None)
            if attributes is not None and LINE_ATTRIBUTE not in attributes:
                line = script_line()
                if line is not None:
                    attributes[LINE_ATTRIBUTE] = line

        return __init__

    patched = []
    try:
        for cls in _recorded_classes():
            original = cls.__dict__["__init__"]
            cls.__init__ = wrapped(original)  # type: ignore[misc]
            patched.append((cls, original))
        yield
    finally:
        for cls, original in reversed(patched):
            cls.__init__ = original
