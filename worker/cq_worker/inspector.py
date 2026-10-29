"""CadQuery object stack inspection.

A ``Workplane`` remembers the chain of workplanes that produced it through
its ``parent`` attribute. The inspector lists that chain newest first, with
the objects each step holds, and can tessellate a step or a single object so
the GUI can overlay it on the model.
"""

from __future__ import annotations

from typing import Any

import cadquery as cq

from .provenance import creation_line
from .tessellation import Tolerance, tessellate

MAX_VALUE_LENGTH = 200


def stack_frames(workplane: cq.Workplane) -> list[cq.Workplane]:
    frames = []
    current: cq.Workplane | None = workplane
    while current is not None and getattr(current, "parent", None) is not None:
        frames.append(current)
        current = current.parent
    return frames


def describe_stack(frames: list[cq.Workplane]) -> list[dict]:
    return [
        {
            "type": type(frame).__name__,
            "value": f"origin {_point(frame.plane.origin)}",
            "line": creation_line(frame),
            "items": [
                {
                    "type": type(obj).__name__,
                    "value": describe(obj),
                    "displayable": is_displayable(obj),
                    "line": creation_line(obj),
                }
                for obj in frame.objects
            ],
        }
        for frame in frames
    ]


def is_displayable(obj: Any) -> bool:
    return isinstance(obj, cq.Shape) or isinstance(obj, cq.Sketch)


def describe(obj: Any) -> str:
    """A short, human readable summary of a stack object."""

    try:
        if isinstance(obj, cq.Vector):
            return _point(obj)
        if isinstance(obj, cq.Location):
            return f"at {_point(cq.Vector(*obj.toTuple()[0]))}"
        if isinstance(obj, cq.Vertex):
            return _point(cq.Vector(obj.toTuple()))
        if isinstance(obj, cq.Edge):
            return f"{obj.geomType().lower()}, length {_number(obj.Length())}"
        if isinstance(obj, cq.Wire):
            return f"{len(obj.Edges())} edges, length {_number(obj.Length())}"
        if isinstance(obj, cq.Face):
            return f"{obj.geomType().lower()}, area {_number(obj.Area())}"
        if isinstance(obj, cq.Shape):
            solids = obj.Solids()
            if solids:
                return f"{len(solids)} solid(s), volume {_number(obj.Volume())}"
            return f"{len(obj.Faces())} faces, {len(obj.Edges())} edges"
        if isinstance(obj, cq.Sketch):
            return f"{len(list(obj))} faces"
    except Exception:  # fall back to the plain representation
        pass
    return _text(obj)


def frame_geometry(frame: cq.Workplane, item: int | None, tolerance: Tolerance) -> dict:
    """Meshes for one stack frame (all its objects) or for one of its objects.

    Selecting a whole frame also reports its workplane, so the GUI can draw a
    grid on it, sized after the frame's largest dimension.
    """

    objects = frame.objects if item is None else [frame.objects[item]]
    meshes = []
    for obj in objects:
        if not is_displayable(obj):
            continue
        shapes = list(obj) if isinstance(obj, cq.Sketch) else [obj]
        for shape in shapes:
            mesh = tessellate(shape.wrapped, tolerance)
            if mesh is not None:
                meshes.append(mesh.to_message())

    plane = None
    if item is None:
        try:
            size = float(frame.largestDimension())
        except Exception:  # e.g. a workplane holding only points
            size = 10.0
        p = frame.plane
        plane = {
            "origin": list(p.origin.toTuple()),
            "x_dir": list(p.xDir.toTuple()),
            "z_dir": list(p.zDir.toTuple()),
            "size": size if size > 0 and size != float("inf") else 10.0,
        }
    return {"meshes": meshes, "plane": plane}


def _number(value: float) -> str:
    return f"{value:.4g}"


def _point(vector: cq.Vector) -> str:
    return "(" + ", ".join(_number(c) for c in vector.toTuple()) + ")"


def _text(value: Any) -> str:
    text = str(value)
    if len(text) <= MAX_VALUE_LENGTH:
        return text
    return text[: MAX_VALUE_LENGTH - 1] + "…"
