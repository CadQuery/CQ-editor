"""Conservative dimensional checks for cylindrical assembly fits.

These checks use the tolerances declared by the designer.  They supplement,
but cannot replace, exact interference checks of the modeled solids.
"""

from __future__ import annotations

import math


class FitError(ValueError):
    """A declared mechanical fit fails its worst-case requirements."""


def _finite(name: str, value: float, *, minimum: float | None = None) -> float:
    result = float(value)
    if not math.isfinite(result) or (minimum is not None and result < minimum):
        raise ValueError(f"{name} must be finite and at least {minimum}")
    return result


def cylindrical_fit(
    *,
    hole_diameter: float,
    pin_diameter: float,
    hole_lower: float = 0.0,
    hole_upper: float = 0.0,
    pin_lower: float = 0.0,
    pin_upper: float = 0.0,
    required_clearance: float = 0.0,
    center_offset: float = 0.0,
    insertion_depth: float | None = None,
    hole_depth: float | None = None,
    head_diameter: float | None = None,
    counterbore_diameter: float | None = None,
    head_height: float | None = None,
    counterbore_depth: float | None = None,
) -> dict:
    """Evaluate the worst-case envelope of a rigid pin and circular hole.

    All dimensions are in the same model unit.  Deviations are signed and
    ``center_offset`` is the maximum radial axis offset, not a diameter.
    Equal nominal diameters with zero deviations have zero clearance, which
    is not proof that manufactured parts will assemble.
    """

    hole = _finite("hole_diameter", hole_diameter, minimum=0.0)
    pin = _finite("pin_diameter", pin_diameter, minimum=0.0)
    if hole == 0 or pin == 0:
        raise ValueError("Diameters must be positive")
    hole_low = _finite("hole_lower", hole_lower)
    hole_high = _finite("hole_upper", hole_upper)
    pin_low = _finite("pin_lower", pin_lower)
    pin_high = _finite("pin_upper", pin_upper)
    if hole_low > hole_high or pin_low > pin_high:
        raise ValueError("Lower deviation cannot exceed upper deviation")
    hole_min, hole_max = hole + hole_low, hole + hole_high
    pin_min, pin_max = pin + pin_low, pin + pin_high
    if hole_min <= 0 or pin_min <= 0:
        raise ValueError("Tolerance range must keep diameters positive")
    required = _finite("required_clearance", required_clearance, minimum=0.0)
    offset = _finite("center_offset", center_offset, minimum=0.0)
    # A radial offset consumes twice that amount of diametral clearance.
    clearance_min = hole_min - pin_max - 2 * offset
    clearance_max = hole_max - pin_min - 2 * offset
    issues = []
    if clearance_min < required:
        issues.append("Worst-case diametral clearance is below the required clearance")

    if (insertion_depth is None) != (hole_depth is None):
        raise ValueError("Provide both insertion_depth and hole_depth")
    if insertion_depth is not None:
        assert hole_depth is not None
        insertion = _finite("insertion_depth", insertion_depth, minimum=0.0)
        depth = _finite("hole_depth", hole_depth, minimum=0.0)
        if insertion > depth:
            issues.append("Insertion is deeper than the available hole")

    if (head_diameter is None) != (counterbore_diameter is None):
        raise ValueError("Provide both head_diameter and counterbore_diameter")
    if head_diameter is not None:
        assert counterbore_diameter is not None
        head = _finite("head_diameter", head_diameter, minimum=0.0)
        counterbore = _finite("counterbore_diameter", counterbore_diameter, minimum=0.0)
        if head <= 0 or counterbore <= 0:
            raise ValueError("Head and counterbore diameters must be positive")
        if head > counterbore:
            issues.append("Head is wider than its counterbore")
    if (head_height is None) != (counterbore_depth is None):
        raise ValueError("Provide both head_height and counterbore_depth")
    if head_height is not None:
        assert counterbore_depth is not None
        height = _finite("head_height", head_height, minimum=0.0)
        depth = _finite("counterbore_depth", counterbore_depth, minimum=0.0)
        if height > depth:
            issues.append("Head is taller than the counterbore depth")

    return {
        "ok": not issues,
        "minimum_clearance": clearance_min,
        "maximum_clearance": clearance_max,
        "required_clearance": required,
        "issues": issues,
    }


def require_fit(**dimensions: float) -> dict:
    """Fail a CQ-editor script, headless check or build on an unsafe fit."""

    report = cylindrical_fit(**dimensions)
    if not report["ok"]:
        raise FitError("; ".join(report["issues"]))
    return report
