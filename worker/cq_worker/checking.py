"""Geometry checks shared by interactive inspection, export and headless CI."""

from __future__ import annotations

from typing import Any

import cadquery as cq

from .scene import Registry, RegistryEntry


class GeometryInvalid(Exception):
    def __init__(
        self,
        reports: list[dict[str, Any]],
        interferences: list[dict[str, Any]] | None = None,
    ) -> None:
        self.reports = reports
        self.interferences = interferences or []
        names = ", ".join(report["name"] for report in reports)
        super().__init__(
            f"Geometry needs review before export: {names}"
            if names
            else "Assembly interference needs review before export"
        )


def check_shape(shape: cq.Shape) -> dict[str, Any]:
    """Inspect the exact world-space shape that would leave the editor."""
    valid = shape.isValid()
    solids = shape.Solids()
    shells = shape.Shells()
    open_shells = sum(not shell.Closed() for shell in shells)
    non_manifold = 0
    # Count each edge within its solid or free shell. Counting over a whole
    # compound would incorrectly flag two separately valid touching solids.
    checked = solids or shells
    for body in checked:
        counts: dict[int, int] = {}
        for face in body.Faces():
            for edge in face.Edges():
                identifier = edge.hashCode()
                counts[identifier] = counts.get(identifier, 0) + 1
        non_manifold += sum(count > 2 for count in counts.values())
    issues: list[str] = []
    if not valid:
        issues.append("OpenCascade reports invalid topology")
    if open_shells:
        issues.append(f"{open_shells} open shell(s)")
    if non_manifold:
        issues.append(f"{non_manifold} non-manifold edge(s)")
    return {
        "valid": valid and not open_shells and not non_manifold,
        "solids": len(solids),
        "open_shells": open_shells,
        "non_manifold_edges": non_manifold,
        "issues": issues,
    }


def check_registry(
    registry: Registry, keys: list[str], flat_parts: bool = False
) -> list[dict[str, Any]]:
    """Report every exported part once, including descendants of selections."""
    reports: list[dict[str, Any]] = []
    seen: set[str] = set()

    def visit(key: str) -> None:
        if key in seen:
            return
        seen.add(key)
        entry: RegistryEntry = registry.get(key)
        if entry.shape is not None:
            reports.append(
                {
                    "key": key,
                    "name": entry.name or key,
                    **check_shape(entry.shape),
                }
            )
        if not flat_parts:
            for child in entry.children:
                visit(child)

    for key in keys:
        visit(key)
    return reports


def invalid_reports(reports: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return [report for report in reports if not report["valid"]]
