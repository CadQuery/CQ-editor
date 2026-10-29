"""Read-only preview of literal CadQuery selectors in the last completed run.

No source expression is evaluated. Only a top-level runtime variable and a
literal string argument are resolved, so moving the caret never executes
arbitrary Python from the editor buffer.
"""

from __future__ import annotations

import ast
from typing import Any

import cadquery as cq

from .scene import Registry
from .selection import Receiver

_KINDS = {"faces": "face", "edges": "edge", "vertices": "vertex"}
MAX_SELECTOR_LENGTH = 256
MAX_HIGHLIGHTS = 64


def preview(
    registry: Registry,
    names: dict[str, cq.Shape | cq.Workplane],
    source: str,
    line: int,
    column: int,
    preferred_key: str | None = None,
) -> dict[str, Any]:
    """Return exact topology ids for the selector at a 1-based caret position."""

    lines = source.splitlines()
    if not 1 <= line <= len(lines) or not 1 <= column <= len(lines[line - 1]) + 1:
        return {"active": False, "count": 0, "matches": [], "message": None}
    position = (line, len(lines[line - 1][: column - 1].encode("utf-8")))
    try:
        tree = ast.parse(source)
    except SyntaxError:
        return {"active": False, "count": 0, "matches": [], "message": None}
    candidates: list[ast.Call] = []
    for node in ast.walk(tree):
        if (
            isinstance(node, ast.Call)
            and isinstance(node.func, ast.Attribute)
            and node.func.attr in _KINDS
            and node.end_lineno is not None
            and node.end_col_offset is not None
            and (node.lineno, node.col_offset) <= position
            and position <= (node.end_lineno, node.end_col_offset)
        ):
            candidates.append(node)
    if not candidates:
        return {"active": False, "count": 0, "matches": [], "message": None}
    call = min(
        candidates,
        key=lambda node: (
            (node.end_lineno or node.lineno) - node.lineno,
            (node.end_col_offset or node.col_offset) - node.col_offset,
        ),
    )
    result: dict[str, Any] = {
        "active": True,
        "count": 0,
        "matches": [],
        "message": None,
    }
    attribute = call.func
    assert isinstance(attribute, ast.Attribute)
    if (
        not isinstance(attribute.value, ast.Name)
        or len(call.args) != 1
        or call.keywords
        or not isinstance(call.args[0], ast.Constant)
        or not isinstance(call.args[0].value, str)
    ):
        result["message"] = "Preview supports a variable with a literal selector string"
        return result
    selector = call.args[0].value
    if len(selector) > MAX_SELECTOR_LENGTH:
        result["message"] = "Selector is too long to preview"
        return result
    variable = attribute.value.id
    obj = names.get(variable)
    if obj is None:
        result["message"] = f"{variable} is not a CadQuery object from the last run"
        return result
    kind = _KINDS[attribute.attr]
    sources = []
    for key in registry.keys():
        entry = registry.get(key)
        for part, part_source in enumerate(entry.sources):
            if part_source.obj is obj and part < len(entry.topologies):
                sources.append((key, part, entry.topologies[part]))
    if not sources:
        result["message"] = f"{variable} is not displayed in the 3D view"
        return result
    sources.sort(key=lambda item: item[0] != preferred_key)
    key, part, topology = sources[0]
    try:
        selected = Receiver(obj, kind).select(selector)
    except Exception as error:
        result["message"] = f"Invalid selector: {error}"
        return result
    result["count"] = len(selected)
    collection = getattr(
        topology, {"face": "faces", "edge": "edges", "vertex": "vertices"}[kind]
    )
    by_hash: dict[int, list[tuple[int, cq.Shape]]] = {}
    for identifier, raw in enumerate(collection, 1):
        entity = cq.Shape.cast(raw)
        by_hash.setdefault(entity.hashCode(), []).append((identifier, entity))
    for shape in selected[:MAX_HIGHLIGHTS]:
        match_id = next(
            (
                identifier
                for identifier, entity in by_hash.get(shape.hashCode(), [])
                if entity.wrapped.IsSame(shape.wrapped)
            ),
            None,
        )
        if match_id is None:
            result["message"] = (
                "Selected entities do not belong to the displayed object"
            )
            result["matches"] = []
            return result
        result["matches"].append(
            {"key": key, "part": part, "kind": kind, "identifier": match_id}
        )
    return result
