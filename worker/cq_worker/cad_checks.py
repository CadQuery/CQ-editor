"""Assertions about a model, recorded during a script run for the GUI and CI.

All distances and volumes use the script's model units. A failed assertion is
recorded rather than raised, so one run can report every unmet requirement.
Invalid assertion arguments still raise immediately as programming errors.
"""

from __future__ import annotations

import math
from typing import Any

import cadquery as cq
from OCP.BRepExtrema import BRepExtrema_DistShapeShape

from .checking import check_shape
from .scene import to_compound


def _finite(value: float, label: str) -> float:
    number = float(value)
    if not math.isfinite(number):
        raise ValueError(f"{label} must be finite")
    return number


def _range(bounds: tuple[float, float], label: str) -> tuple[float, float]:
    if not isinstance(bounds, (tuple, list)) or len(bounds) != 2:
        raise ValueError(f"{label} needs a (minimum, maximum) pair")
    low, high = _finite(bounds[0], label), _finite(bounds[1], label)
    if low > high:
        raise ValueError(f"{label} minimum exceeds maximum")
    return low, high


class CheckSuite:
    def __init__(self) -> None:
        self.results: list[dict[str, Any]] = []

    def expect(
        self,
        obj: cq.Shape | cq.Workplane | cq.Sketch,
        name: str | None,
        line: int | None,
    ) -> Expectation:
        if name is not None and (not isinstance(name, str) or not name.strip()):
            raise ValueError("Check name must be nonempty text")
        if not isinstance(obj, (cq.Shape, cq.Workplane, cq.Sketch)):
            raise TypeError("expect() needs a CadQuery shape, workplane or sketch")
        return Expectation(self, obj, name or f"Check {len(self.results) + 1}", line)

    @property
    def ok(self) -> bool:
        return all(result["ok"] for result in self.results)


class Expectation:
    def __init__(
        self,
        suite: CheckSuite,
        obj: cq.Shape | cq.Workplane | cq.Sketch,
        name: str,
        line: int | None,
    ) -> None:
        self.suite = suite
        self.obj = obj
        self.name = name
        self.line = line

    def _record(self, kind: str, ok: bool, detail: str) -> Expectation:
        self.suite.results.append(
            {
                "name": self.name,
                "kind": kind,
                "ok": bool(ok),
                "detail": detail,
                "line": self.line,
            }
        )
        return self

    def _shape(self) -> cq.Compound:
        shape = to_compound(self.obj)
        if not shape.Solids() and not shape.Faces() and not shape.Edges():
            raise ValueError("The checked object has no geometry")
        return shape

    def is_valid(self) -> Expectation:
        try:
            report = check_shape(self._shape())
            detail = (
                "Valid topology" if report["valid"] else "; ".join(report["issues"])
            )
            return self._record("validity", report["valid"], detail)
        except Exception as error:
            return self._record("validity", False, f"Could not check topology: {error}")

    def volume_between(self, minimum: float, maximum: float) -> Expectation:
        low, high = _range((minimum, maximum), "Volume")
        try:
            shape = self._shape()
            if not shape.Solids():
                raise ValueError("Volume needs at least one solid")
            actual = _finite(shape.Volume(), "Measured volume")
            return self._record(
                "volume",
                low <= actual <= high,
                f"Volume {actual:g}; expected [{low:g}, {high:g}] model units³",
            )
        except Exception as error:
            return self._record("volume", False, f"Could not measure volume: {error}")

    def dimensions_within(
        self,
        *,
        x: tuple[float, float] | None = None,
        y: tuple[float, float] | None = None,
        z: tuple[float, float] | None = None,
    ) -> Expectation:
        bounds = {
            axis: _range(value, axis)
            for axis, value in (("x", x), ("y", y), ("z", z))
            if value is not None
        }
        if not bounds:
            raise ValueError("Specify at least one dimension range")
        try:
            box = self._shape().BoundingBox()
            actual = {"x": box.xlen, "y": box.ylen, "z": box.zlen}
            detail = "; ".join(
                f"{axis.upper()} {actual[axis]:g}; expected [{low:g}, {high:g}] model units"
                for axis, (low, high) in bounds.items()
            )
            return self._record(
                "dimensions",
                all(
                    low <= actual[axis] <= high for axis, (low, high) in bounds.items()
                ),
                detail,
            )
        except Exception as error:
            return self._record(
                "dimensions", False, f"Could not measure dimensions: {error}"
            )

    def clearance_at_least(
        self, other: cq.Shape | cq.Workplane | cq.Sketch, minimum: float
    ) -> Expectation:
        limit = _finite(minimum, "Clearance")
        if limit < 0:
            raise ValueError("Clearance cannot be negative")
        if not isinstance(other, (cq.Shape, cq.Workplane, cq.Sketch)):
            raise TypeError(
                "Clearance needs another CadQuery shape, workplane or sketch"
            )
        try:
            first = self._shape()
            second = to_compound(other)
            if not second.Solids() and not second.Faces() and not second.Edges():
                raise ValueError("The other object has no geometry")
            operation = BRepExtrema_DistShapeShape(first.wrapped, second.wrapped)
            if not operation.IsDone() or operation.NbSolution() < 1:
                raise ValueError("OpenCascade could not calculate the distance")
            actual = _finite(operation.Value(), "Measured clearance")
            return self._record(
                "clearance",
                actual >= limit,
                f"Clearance {actual:g}; required at least {limit:g} model units",
            )
        except Exception as error:
            return self._record(
                "clearance", False, f"Could not measure clearance: {error}"
            )
