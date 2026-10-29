"""Exact, world-space interference checks between displayed solid instances.

The display mesh is deliberately never used as evidence of clearance.  A
compound can be topologically valid while two of its solids occupy the same
space, so this analysis compares the solids individually.
"""

from __future__ import annotations

import math
from collections.abc import Callable
from dataclasses import dataclass
from typing import Any

import cadquery as cq
from OCP.BRepExtrema import BRepExtrema_DistShapeShape

from .measurement import exact_bounds
from .scene import Registry, RegistryEntry


@dataclass(frozen=True)
class SolidInstance:
    key: str
    name: str
    part: int
    solid: int
    shape: cq.Solid
    bounds: tuple[float, float, float, float, float, float]
    source: int
    placement: tuple[float, ...]
    root: str

    def reference(self) -> dict:
        return {
            "key": self.key,
            "name": self.name,
            "part": self.part,
            "solid": self.solid,
        }


def _merged_names(assembly: cq.Assembly) -> list[str]:
    names: list[str] = []

    def visit(node: cq.Assembly, parents: tuple[str, ...]) -> None:
        path = (*parents, node.name or "part")
        if node.obj is not None:
            names.append("/".join(path))
        for child in node.children:
            visit(child, path)

    visit(assembly, ())
    return names


def collect_solids(
    registry: Registry,
    keys: list[str],
    flat_parts: bool = False,
    replacements: dict[str, cq.Shape] | None = None,
) -> list[SolidInstance]:
    """Flatten actual solids, retaining each assembly occurrence and placement.

    The same source displayed independently and inside an assembly at the same
    placement is a display alias, not an extra manufactured part.  Two children
    of one assembly remain distinct even if they use one source and location.
    """

    result: list[SolidInstance] = []
    visited: set[str] = set()
    aliases: dict[tuple[int, tuple[float, ...]], str] = {}

    def visit(key: str, root: str) -> None:
        if key in visited:
            return
        visited.add(key)
        entry: RegistryEntry = registry.get(key)
        replacement = (replacements or {}).get(key)
        shapes = (
            [replacement]
            if replacement is not None
            else entry.part_shapes or ([entry.shape] if entry.shape is not None else [])
        )
        names = (
            _merged_names(entry.obj)
            if replacement is None
            and entry.part_shapes
            and isinstance(entry.obj, cq.Assembly)
            else [entry.name or key] * len(shapes)
        )
        for part, shape in enumerate(shapes):
            if shape is None:
                continue
            source = entry.sources[part] if part < len(entry.sources) else None
            identity = id(source.obj if source is not None else entry.obj)
            location = source.location if source is not None else cq.Location()
            placement = tuple(round(v, 12) for v in location_matrix(location))
            alias = (identity, placement)
            if alias in aliases and aliases[alias] != root:
                continue
            aliases.setdefault(alias, root)
            for solid_index, solid in enumerate(shape.Solids()):
                bounds = exact_bounds(solid)
                if not all(math.isfinite(value) for value in bounds):
                    raise ValueError(f"Non-finite bounds for {entry.name or key}")
                result.append(
                    SolidInstance(
                        key,
                        names[part] if part < len(names) else entry.name or key,
                        part,
                        solid_index,
                        solid,
                        bounds,
                        identity,
                        placement,
                        root,
                    )
                )
        if not flat_parts:
            for child in entry.children:
                visit(child, root)

    for key in dict.fromkeys(keys):
        visit(key, key)
    return result


def location_matrix(location: cq.Location) -> list[float]:
    """Rigid placement identity; the same convention as scene messages."""

    transform = location.wrapped.Transformation()
    return [
        transform.Value(row, column) for column in range(1, 5) for row in range(1, 4)
    ]


def _candidate(first: SolidInstance, second: SolidInstance, margin: float) -> bool:
    a, b = first.bounds, second.bounds
    return all(
        a[axis] <= b[axis + 3] + margin and b[axis] <= a[axis + 3] + margin
        for axis in range(3)
    )


def _distance(first: cq.Shape, second: cq.Shape) -> float:
    measure = BRepExtrema_DistShapeShape(first.wrapped, second.wrapped)
    if not measure.IsDone() or measure.NbSolution() == 0:
        raise ValueError("Open Cascade could not measure the pair")
    return float(measure.Value())


def inspect_pair(first: SolidInstance, second: SolidInstance, tolerance: float) -> dict:
    """Classify a candidate using exact B-rep intersection and distance."""

    report: dict[str, Any] = {
        "first": first.reference(),
        "second": second.reference(),
        "kind": "inconclusive",
        "overlap_volume": None,
        "distance": None,
        "point": None,
        "reason": None,
    }
    try:
        if not first.shape.isValid() or not second.shape.isValid():
            raise ValueError("A solid has invalid topology")
        common = first.shape.intersect(second.shape)
        if not common.isValid():
            raise ValueError("Open Cascade returned an invalid intersection")
        volume = float(common.Volume())
        if not math.isfinite(volume) or volume < 0:
            raise ValueError("Open Cascade returned an invalid overlap volume")
        report["overlap_volume"] = volume
        threshold = max(
            tolerance**3, min(first.shape.Volume(), second.shape.Volume()) * 1e-12
        )
        if volume > threshold:
            report["kind"] = "interference"
            report["distance"] = 0.0
            center = common.Center()
            point = [center.x, center.y, center.z]
            if not all(math.isfinite(value) for value in point):
                raise ValueError("Open Cascade returned a non-finite overlap center")
            report["point"] = point
        elif volume > 0:
            report["reason"] = "Overlap is below the numerical tolerance"
        else:
            distance = _distance(first.shape, second.shape)
            if not math.isfinite(distance):
                raise ValueError("Open Cascade returned a non-finite distance")
            report["distance"] = distance
            report["kind"] = "contact" if distance <= tolerance else "clearance"
    except (RuntimeError, ValueError, OverflowError) as error:
        report["reason"] = str(error)
    return report


def analyze_interferences(
    registry: Registry,
    keys: list[str],
    *,
    flat_parts: bool = False,
    tolerance: float = 1e-6,
    clearance: float = 0.0,
    replacements: dict[str, cq.Shape] | None = None,
    cancelled: Callable[[], bool] | None = None,
) -> list[dict]:
    """Report nearby pairs; never claim a failed exact calculation is safe."""

    if not math.isfinite(tolerance) or tolerance <= 0:
        raise ValueError("Interference tolerance must be finite and positive")
    if not math.isfinite(clearance) or clearance < 0:
        raise ValueError("Clearance limit must be finite and non-negative")
    parts = sorted(
        collect_solids(registry, keys, flat_parts, replacements),
        key=lambda part: part.bounds[0],
    )
    active: list[SolidInstance] = []
    reports: list[dict] = []
    margin = max(tolerance, clearance)
    for part in parts:
        if cancelled is not None and cancelled():
            raise InterruptedError("Interference check cancelled")
        active = [
            other for other in active if other.bounds[3] + margin >= part.bounds[0]
        ]
        for other in active:
            if not _candidate(other, part, margin):
                continue
            report = inspect_pair(other, part, tolerance)
            if report["kind"] != "clearance" or report["distance"] <= clearance:
                reports.append(report)
        active.append(part)
    return reports


def overlap_shape(registry: Registry, first: dict, second: dict) -> cq.Shape:
    """Rebuild the exact common volume for a reported pair of live solids."""

    keys = list(dict.fromkeys((first["key"], second["key"])))
    instances = collect_solids(registry, keys)

    def resolve(reference: dict) -> SolidInstance:
        matches = [
            instance
            for instance in instances
            if instance.key == reference["key"]
            and instance.part == reference["part"]
            and instance.solid == reference["solid"]
            and instance.name == reference["name"]
        ]
        if len(matches) != 1:
            raise ValueError("Interference pair is no longer in the live scene")
        return matches[0]

    a, b = resolve(first), resolve(second)
    if a is b:
        raise ValueError("An interference pair must contain two distinct solids")
    common = a.shape.intersect(b.shape)
    volume = float(common.Volume())
    if not common.isValid() or not math.isfinite(volume) or volume <= 0:
        raise ValueError("The pair has no valid positive shared volume")
    return common


def apply_allowances(
    registry: Registry,
    keys: list[str],
    reports: list[dict],
    allowances: list[tuple[str, str, str]],
    *,
    flat_parts: bool = False,
    replacements: dict[str, cq.Shape] | None = None,
) -> None:
    """Mark only explicitly named, unambiguous interference pairs as accepted.

    A basename is accepted when it identifies exactly one solid. Full paths
    from merged assemblies can disambiguate repeated names. Numerical failures
    are never accepted, even if the pair was declared.
    """

    if not allowances:
        return
    instances = collect_solids(registry, keys, flat_parts, replacements)

    def resolve(name: str) -> SolidInstance | None:
        matches = [instance for instance in instances if instance.name == name]
        if not matches and "/" not in name:
            matches = [
                instance
                for instance in instances
                if instance.name.rsplit("/", 1)[-1] == name
            ]
        if not matches:
            return None
        if len(matches) != 1:
            raise ValueError(
                f"Interference allowance {name!r} identifies {len(matches)} solids; "
                "use unique part names or a full assembly path"
            )
        return matches[0]

    accepted: dict[frozenset[tuple[str, int, int]], str] = {}
    for first, second, reason in allowances:
        a, b = resolve(first), resolve(second)
        if a is None or b is None:
            continue  # A geometry check may cover only selected objects.
        if a is b:
            raise ValueError("An interference allowance must name two distinct solids")
        pair = frozenset(((a.key, a.part, a.solid), (b.key, b.part, b.solid)))
        if pair in accepted:
            raise ValueError("An interference pair was declared more than once")
        accepted[pair] = reason
    for report in reports:
        if report["kind"] != "interference":
            continue
        pair = frozenset(
            (
                (side["key"], side["part"], side["solid"])
                for side in (report["first"], report["second"])
            )
        )
        if pair in accepted:
            report["allowance_reason"] = accepted[pair]


def summarize(reports: list[dict]) -> dict:
    """A tri-state verdict: numerical uncertainty must never count as safe."""

    counts = {
        kind: 0 for kind in ("interference", "inconclusive", "contact", "clearance")
    }
    for report in reports:
        counts[report["kind"]] += 1
    accepted = sum(
        report["kind"] == "interference" and bool(report.get("allowance_reason"))
        for report in reports
    )
    status = (
        "interference"
        if counts["interference"] > accepted
        else "inconclusive" if counts["inconclusive"] else "clear"
    )
    return {"status": status, "counts": counts, "accepted": accepted}
