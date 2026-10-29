"""Conservative collision certification along declared rigid-body motion.

Each segment linearly interpolates translation and XYZ Euler angles.  A
Lipschitz displacement bound certifies empty intervals; near contact, the
segment is subdivided.  Exhausting the depth is *inconclusive*, never clear.
"""

from __future__ import annotations

import math
from collections.abc import Iterable
from typing import Any

import cadquery as cq
from OCP.BRepExtrema import BRepExtrema_DistShapeShape

from .measurement import exact_bounds
from .scene import to_compound

Pose = tuple[float, float, float, float, float, float]


class PathError(ValueError):
    """A declared assembly path is blocked or cannot be certified."""


def _shape(value: cq.Shape | cq.Workplane | cq.Assembly) -> cq.Shape:
    shape = value.toCompound() if isinstance(value, cq.Assembly) else to_compound(value)
    if not shape.Solids() or not shape.isValid():
        raise ValueError("Path objects must contain valid closed solids")
    return shape


def _pose(value: Iterable[float]) -> Pose:
    numbers = tuple(float(component) for component in value)
    if len(numbers) != 6 or not all(math.isfinite(number) for number in numbers):
        raise ValueError("Each pose must contain finite x, y, z, rx, ry, rz")
    return numbers  # type: ignore[return-value]


def _placed(shape: cq.Shape, pose: Pose) -> cq.Shape:
    return shape.moved(cq.Location(pose[:3], pose[3:]))


def _distance(first: cq.Shape, second: cq.Shape) -> float:
    operation = BRepExtrema_DistShapeShape(first.wrapped, second.wrapped)
    if not operation.IsDone() or operation.NbSolution() == 0:
        raise ValueError("Open Cascade could not measure a path position")
    result = float(operation.Value())
    if not math.isfinite(result):
        raise ValueError("Open Cascade returned a non-finite path distance")
    return result


def _radius(shape: cq.Shape) -> float:
    x0, y0, z0, x1, y1, z1 = exact_bounds(shape)
    return math.sqrt(
        max(x0 * x0, x1 * x1) + max(y0 * y0, y1 * y1) + max(z0 * z0, z1 * z1)
    )


def _bounds_overlap(
    first: tuple[float, float, float, float, float, float],
    second: tuple[float, float, float, float, float, float],
    margin: float,
) -> bool:
    return all(
        first[axis] <= second[axis + 3] + margin
        and second[axis] <= first[axis + 3] + margin
        for axis in range(3)
    )


def _midpoint(first: Pose, second: Pose) -> Pose:
    return tuple((a + b) / 2 for a, b in zip(first, second, strict=True))  # type: ignore[return-value]


def _motion_bound(first: Pose, second: Pose, radius: float) -> float:
    translation = math.dist(first[:3], second[:3])
    angles = sum(abs(b - a) for a, b in zip(first[3:], second[3:], strict=True))
    return (translation + radius * math.radians(angles)) / 2


def trace_path(
    moving: cq.Shape | cq.Workplane | cq.Assembly,
    obstacles: Iterable[cq.Shape | cq.Workplane | cq.Assembly],
    poses: Iterable[Iterable[float]],
    *,
    required_clearance: float = 0.0,
    tolerance: float = 1e-6,
    max_depth: int = 18,
    max_poses: int = 4096,
) -> dict[str, Any]:
    """Certify a path or find an intersection, with no unsafe success guess.

    Positions use model units and Euler angles use degrees.  The source solid
    is local to the path origin; obstacles are fixed in world coordinates.
    """

    body = _shape(moving)
    fixed = [_shape(obstacle) for obstacle in obstacles]
    fixed_bounds = [exact_bounds(obstacle) for obstacle in fixed]
    route = [_pose(pose) for pose in poses]
    if len(route) < 2:
        raise ValueError("A path needs at least two poses")
    if not math.isfinite(required_clearance) or required_clearance < 0:
        raise ValueError("Required clearance must be finite and non-negative")
    if not math.isfinite(tolerance) or tolerance <= 0:
        raise ValueError("Path tolerance must be finite and positive")
    if (
        isinstance(max_depth, bool)
        or not isinstance(max_depth, int)
        or not 0 <= max_depth <= 24
    ):
        raise ValueError("max_depth must be an integer between 0 and 24")
    if isinstance(max_poses, bool) or not isinstance(max_poses, int) or max_poses < 3:
        raise ValueError("max_poses must be an integer of at least 3")

    radius = _radius(body)
    checked = 0
    cache: dict[Pose, tuple[float, dict[str, Any] | None]] = {}

    def probe(pose: Pose) -> tuple[float, dict[str, Any] | None]:
        nonlocal checked
        if pose in cache:
            return cache[pose]
        if checked >= max_poses:
            raise ValueError("Path pose budget exhausted")
        placed = _placed(body, pose)
        placed_bounds = exact_bounds(placed)
        closest = math.inf
        collision = None
        for index, (obstacle, bounds) in enumerate(
            zip(fixed, fixed_bounds, strict=True)
        ):
            if _bounds_overlap(placed_bounds, bounds, tolerance):
                common = placed.intersect(obstacle)
                if not common.isValid():
                    raise ValueError(
                        "Open Cascade returned an invalid path intersection"
                    )
                volume = float(common.Volume())
                if not math.isfinite(volume):
                    raise ValueError(
                        "Open Cascade returned a non-finite overlap volume"
                    )
                if volume > tolerance**3:
                    collision = {"obstacle": index, "overlap_volume": volume}
                    break
                if volume > 0:
                    raise ValueError("Path overlap is below numerical tolerance")
            distance = _distance(placed, obstacle)
            closest = min(closest, distance)
        checked += 1
        cache[pose] = closest, collision
        return cache[pose]

    for segment, (start, end) in enumerate(zip(route, route[1:])):
        try:
            for endpoint in (start, end):
                _, collision = probe(endpoint)
                if collision is not None:
                    return {
                        "status": "blocked",
                        "segment": segment,
                        "pose": endpoint,
                        "checked_poses": checked,
                        **collision,
                    }

            def certify(a: Pose, b: Pose, depth: int) -> dict[str, Any] | None:
                middle = _midpoint(a, b)
                distance, collision = probe(middle)
                if collision is not None:
                    return {"status": "blocked", "pose": middle, **collision}
                # Distance can decrease by no more than the maximum motion of
                # a point on the body from the midpoint to any intermediate pose.
                if distance > required_clearance + tolerance + _motion_bound(
                    a, b, radius
                ):
                    return None
                if depth >= max_depth:
                    return {
                        "status": "inconclusive",
                        "pose": middle,
                        "minimum_distance": distance,
                    }
                return certify(a, middle, depth + 1) or certify(middle, b, depth + 1)

            finding = certify(start, end, 0)
            if finding is not None:
                return {"segment": segment, "checked_poses": checked, **finding}
        except (RuntimeError, ValueError, OverflowError) as error:
            return {
                "status": "inconclusive",
                "segment": segment,
                "checked_poses": checked,
                "reason": str(error),
            }
    return {"status": "clear", "segments": len(route) - 1, "checked_poses": checked}


def require_clear_path(*args: Any, **kwargs: Any) -> dict[str, Any]:
    """Make a CQ-editor script and headless build fail on an unsafe path."""

    report = trace_path(*args, **kwargs)
    if report["status"] != "clear":
        raise PathError(f"Assembly path {report['status']}: {report}")
    return report
