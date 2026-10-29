"""Conservative explanations for common CAD kernel failures.

The traceback remains authoritative. These are troubleshooting hints, never
claims that one geometric cause has been proven from an exception string.
"""

from __future__ import annotations

import ast
from typing import Any

_HINTS = {
    "fillet": (
        "Try a smaller radius; adjacent faces may leave too little room.",
        "Check that the selected edges belong to the intended solid and do not meet at a problematic corner.",
        "Inspect the shape before this operation for invalid or very short edges.",
    ),
    "chamfer": (
        "Try smaller chamfer distances relative to nearby faces and edges.",
        "Check the selected edges and the validity of the solid before chamfering.",
    ),
    "loft": (
        "Check that each section is a usable closed wire or face and that section order is correct.",
        "Look for crossing, coincident or sharply different profiles.",
    ),
    "sweep": (
        "Check that the path and profile are valid and that the profile follows the intended plane.",
        "Look for tight turns or self-intersection along the path.",
    ),
    "cut": (
        "Check that both operands contain valid solids and overlap where the cut is expected.",
        "Look for coincident or nearly tangent faces that may make the boolean ambiguous.",
    ),
    "union": (
        "Check both solids for valid topology and inspect where they touch or overlap.",
        "Nearly coincident faces and very thin intersections can make the boolean unstable.",
    ),
    "intersect": (
        "Check that the solids actually share volume rather than only touching.",
        "Inspect both operands for invalid topology or nearly coincident faces.",
    ),
    "shell": (
        "Try a smaller wall thickness relative to the smallest local feature.",
        "Inspect highly curved areas and the faces chosen for removal.",
    ),
    "faces": (
        "Preview the selector on the last successful model and check its match count.",
        "Check the selector direction and the workplane or shape on which it runs.",
    ),
    "edges": (
        "Preview the selector on the last successful model and check its match count.",
        "Check edge direction, geometry type and the workplane that receives the selector.",
    ),
    "vertices": (
        "Preview the selector on the last successful model and check its match count.",
        "Check the selector direction and the workplane that receives it.",
    ),
}

_SELECTORS = {"faces", "edges", "vertices"}
_KERNEL_MARKERS = (
    "stdfail",
    "brep_",
    "brepbuilder",
    "brepalgo",
    "opencascade",
    "could not construct",
    "could not build",
    "failed to create",
    "not a valid shape",
    "self-intersect",
)
_SELECTOR_MARKERS = ("selector", "selected", "parseexception", "syntax", "no objects")


def diagnose(
    error: BaseException, source: str, line: int | None
) -> dict[str, Any] | None:
    """Attach a hint only when the failing script line names a known CAD call."""

    if line is None or isinstance(error, SyntaxError):
        return None
    try:
        tree = ast.parse(source)
    except SyntaxError:
        return None
    calls: list[ast.Call] = []
    for node in ast.walk(tree):
        if (
            isinstance(node, ast.Call)
            and isinstance(node.func, ast.Attribute)
            and node.func.attr in _HINTS
            and node.end_lineno is not None
            and node.lineno <= line <= node.end_lineno
        ):
            calls.append(node)
    if not calls:
        return None
    traceback_frame = error.__traceback__
    from_cadquery = False
    stack_operations = []
    while traceback_frame is not None:
        frame = traceback_frame.tb_frame
        filename = frame.f_code.co_filename.replace("\\", "/").lower()
        if "/cadquery/" in filename:
            from_cadquery = True
            if frame.f_code.co_name in _HINTS:
                stack_operations.append(frame.f_code.co_name)
        traceback_frame = traceback_frame.tb_next
    for stack_operation in stack_operations:
        matching = [
            call
            for call in calls
            if isinstance(call.func, ast.Attribute)
            and call.func.attr == stack_operation
        ]
        if matching:
            calls = matching
            break
    # For a chain on one line, the outermost call is the operation being
    # applied. For a multiline chain, prefer the call named on this line.
    calls.sort(
        key=lambda node: (
            node.func.end_lineno == line,
            node.end_lineno == line,
            node.end_col_offset or 0,
        ),
        reverse=True,
    )
    attribute = calls[0].func
    assert isinstance(attribute, ast.Attribute)
    operation = attribute.attr
    reason = f"{type(error).__name__}: {error}".lower()
    markers = _SELECTOR_MARKERS if operation in _SELECTORS else _KERNEL_MARKERS
    if not any(marker in reason for marker in markers) and not (
        from_cadquery and not isinstance(error, TypeError)
    ):
        return None
    receiver = attribute.value.id if isinstance(attribute.value, ast.Name) else None
    return {
        "operation": operation,
        "receiver": receiver,
        "line": line,
        "hints": list(_HINTS[operation]),
    }
