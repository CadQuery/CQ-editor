"""Exact measurements on the live OCCT subshapes kept by the scene registry.

The mesh is used only to decide what the user clicked.  Every value returned
from this module is calculated from the corresponding OCCT shape, in world
coordinates, so tessellation tolerance and the viewer's ``f32`` coordinates
do not affect the result.
"""

from __future__ import annotations

import math
from dataclasses import dataclass
from typing import Any, Mapping

import cadquery as cq
from OCP.Bnd import Bnd_Box
from OCP.BRepAdaptor import BRepAdaptor_Surface
from OCP.BRepBndLib import BRepBndLib
from OCP.BRepExtrema import BRepExtrema_DistShapeShape
from OCP.GeomAbs import GeomAbs_Cylinder, GeomAbs_Sphere
from OCP.TopoDS import TopoDS_Iterator

from .scene import Registry

#: Entities the GUI can select at once (``MAX_TOPOLOGY_SELECTIONS``).
MAX_SELECTIONS = 64


class MeasurementError(ValueError):
    """Invalid or stale topology selection supplied by the GUI."""


@dataclass(frozen=True)
class TopologyReference:
    key: str
    part: int
    kind: str
    identifier: int

    @classmethod
    def from_message(cls, value: Any) -> TopologyReference:
        if not isinstance(value, Mapping):
            raise MeasurementError("Each measurement selection must be an object")
        key = value.get("key")
        kind = value.get("kind")
        part = value.get("part")
        identifier = value.get("identifier")
        if not isinstance(key, str) or not key:
            raise MeasurementError("A measurement selection needs an object key")
        if kind not in {"face", "edge", "vertex"}:
            raise MeasurementError(f"Unknown topology kind {kind!r}")
        if isinstance(part, bool) or not isinstance(part, int) or part < 0:
            raise MeasurementError("Topology part must be a non-negative integer")
        if (
            isinstance(identifier, bool)
            or not isinstance(identifier, int)
            or identifier < 1
        ):
            raise MeasurementError("Topology identifier must be a positive integer")
        return cls(key, part, kind, identifier)


def measure(
    registry: Registry, raw_selections: Any, raw_objects: Any = None
) -> dict[str, Any]:
    """Describe each selected entity; for exactly two, also their relationship.
    Without entities, describe the selected objects instead."""

    references, shapes = resolve(registry, raw_selections, raw_objects)

    result: dict[str, Any] = {
        # OCCT's indexed subshape order is deterministic only for this live
        # registry entry.  It is deliberately not advertised as a persistent
        # topological name after the model is rebuilt.
        "identity": {"scope": "scene_entry", "persistent": False},
        "entities": (
            [
                _describe(reference, shape)
                for reference, shape in zip(references, shapes, strict=True)
            ]
            if references
            else []
        ),
        "objects": (
            []
            if references
            else [
                _describe_object(registry, key, shape)
                for key, shape in zip(_object_keys(raw_objects), shapes, strict=True)
            ]
        ),
        "pair": None,
        # Of everything measured together.
        "bounding_box": _bounds(*shapes),
    }
    if len(references) == 2:
        result["pair"] = _describe_pair(shapes[0], shapes[1])
    return result


def resolve(
    registry: Registry, raw_selections: Any, raw_objects: Any = None
) -> tuple[list[TopologyReference], list[cq.Shape]]:
    """The picked entities, or else the selected objects, as world shapes."""

    raw_selections = [] if raw_selections is None else raw_selections
    if not isinstance(raw_selections, list) or len(raw_selections) > MAX_SELECTIONS:
        raise MeasurementError(
            f"Select between one and {MAX_SELECTIONS} topological entities to measure"
        )
    if not raw_selections:
        keys = _object_keys(raw_objects)
        if not keys:
            raise MeasurementError(
                f"Select between one and {MAX_SELECTIONS} topological entities "
                "or objects to measure"
            )
        return [], [object_shape(registry, key) for key in keys]

    references = [TopologyReference.from_message(value) for value in raw_selections]
    shapes = []
    for reference in references:
        try:
            shape = registry.get(reference.key).subshape(
                reference.part, reference.kind, reference.identifier
            )
        except (IndexError, KeyError, ValueError) as error:
            raise MeasurementError(str(error)) from None
        shapes.append(shape)
    return references, shapes


def _object_keys(raw_objects: Any) -> list[str]:
    if raw_objects is None:
        return []
    if (
        not isinstance(raw_objects, list)
        or len(raw_objects) > MAX_SELECTIONS
        or not all(isinstance(key, str) and key for key in raw_objects)
    ):
        raise MeasurementError(
            f"Objects to measure must be at most {MAX_SELECTIONS} scene keys"
        )
    return raw_objects


def object_shape(registry: Registry, key: str) -> cq.Shape:
    """An object and its assembly children as shown: placed in world
    coordinates, and a compound of a single shape is that shape."""

    try:
        shapes = registry.subtree_shapes([key])
    except KeyError as error:
        raise MeasurementError(error.args[0]) from None
    if not shapes:
        raise MeasurementError(f"Object {key!r} has no geometry to measure")
    shape = shapes[0] if len(shapes) == 1 else cq.Compound.makeCompound(shapes)
    while isinstance(shape, cq.Compound):
        children = list(_children(shape))
        if len(children) != 1:
            break
        shape = children[0]
    return shape


def _children(shape: cq.Shape) -> list[cq.Shape]:
    children = []
    iterator = TopoDS_Iterator(shape.wrapped)
    while iterator.More():
        children.append(cq.Shape.cast(iterator.Value()))
        iterator.Next()
    return children


def exact_bounds(shape: cq.Shape) -> tuple[float, float, float, float, float, float]:
    """``(xmin, ymin, zmin, xmax, ymax, zmax)`` from the exact geometry.

    ``Shape.BoundingBox()`` uses the triangulation when the shape has one,
    and every displayed shape has: a sphere of radius 5 then reports about
    5.04. This ignores the triangulation and the tolerances.
    """

    box = Bnd_Box()
    BRepBndLib.AddOptimal_s(shape.wrapped, box, False, False)
    # Bnd_Box.Get() returns a struct OCP 8 cannot convert; the corners are
    # the same values in every version.
    low, high = box.CornerMin(), box.CornerMax()
    return (low.X(), low.Y(), low.Z(), high.X(), high.Y(), high.Z())


def _bounds(*shapes: cq.Shape) -> dict[str, list[float]] | None:
    if not shapes:
        return None
    lows, highs = [], []
    for shape in shapes:
        bounds = exact_bounds(shape)
        lows.append(bounds[:3])
        highs.append(bounds[3:])
    low = [min(axis) for axis in zip(*lows)]
    high = [max(axis) for axis in zip(*highs)]
    return {
        "min": low,
        "max": high,
        "size": [b - a for a, b in zip(low, high)],
        "center": [(a + b) / 2 for a, b in zip(low, high)],
    }


def _material_mass(registry: Registry, key: str) -> tuple[float | None, list[str]]:
    """Mass in grams from each displayed solid's assigned material."""
    entry = registry.get(key)
    masses: list[float] = []
    names: list[str] = []
    incomplete = False

    def visit(current) -> None:
        nonlocal incomplete
        own = (
            zip(current.part_shapes, current.part_materials, strict=True)
            if current.part_shapes
            else [(current.shape, current.material)]
        )
        for shape, material in own:
            if shape is None or not shape.Solids():
                continue
            factors = {"g/cm^3": 0.001, "kg/m^3": 0.000001, "g/mm^3": 1.0}
            factor = factors.get(material.densityUnit) if material is not None else None
            if factor is None or not math.isfinite(material.density):
                incomplete = True
            else:
                masses.append(shape.Volume() * material.density * factor)
                names.append(material.name)
        for child in current.children:
            visit(registry.get(child))

    visit(entry)
    return (sum(masses) if masses and not incomplete else None), sorted(set(names))


def _describe_object(registry: Registry, key: str, shape: cq.Shape) -> dict[str, Any]:
    solids = shape.Solids()
    faces = shape.Faces()
    edges = shape.Edges()
    mass, materials = _material_mass(registry, key)
    return {
        "key": key,
        "shape_type": shape.ShapeType().lower(),
        "volume": float(shape.Volume()) if solids else None,
        "area": float(shape.Area()) if faces else None,
        "length": (
            float(sum(edge.Length() for edge in edges)) if edges and not faces else None
        ),
        "center": _point(shape.Center()),
        "bounding_box": _bounds(shape),
        "solids": len(solids),
        "faces": len(faces),
        "edges": len(edges),
        "vertices": len(shape.Vertices()),
        "mass_g": mass,
        "materials": materials,
    }


def _describe(reference: TopologyReference, shape: cq.Shape) -> dict[str, Any]:
    geometry_type = shape.geomType().lower()
    center = _point(shape.Center())
    result: dict[str, Any] = {
        "key": reference.key,
        "part": reference.part,
        "kind": reference.kind,
        "identifier": reference.identifier,
        "geometry_type": geometry_type,
        "center": center,
        "position": center if reference.kind == "vertex" else None,
        "length": (
            shape.Length()
            if reference.kind == "edge" and isinstance(shape, cq.Edge)
            else None
        ),
        "area": shape.Area() if reference.kind == "face" else None,
        "radius": None,
        "diameter": None,
        "bounding_box": _bounds(shape),
    }
    radius = _radius(shape, reference.kind, geometry_type)
    if radius is not None:
        result["radius"] = radius
        result["diameter"] = radius * 2.0
    return result


def _radius(shape: cq.Shape, kind: str, geometry_type: str) -> float | None:
    if kind == "edge" and geometry_type == "circle" and isinstance(shape, cq.Edge):
        return float(shape.radius())
    if kind != "face":
        return None

    surface = BRepAdaptor_Surface(shape.wrapped)
    surface_type = surface.GetType()
    if surface_type == GeomAbs_Cylinder:
        return float(surface.Cylinder().Radius())
    if surface_type == GeomAbs_Sphere:
        return float(surface.Sphere().Radius())

    # A circular planar face (disc) has a planar surface, so its radius lives
    # on the boundary rather than on the surface adaptor.  Return a radius
    # only when all circular boundaries agree; an annulus is ambiguous.
    radii = {
        round(float(edge.radius()), 12)
        for edge in shape.Edges()
        if edge.geomType() == "CIRCLE"
    }
    return next(iter(radii)) if len(radii) == 1 else None


def _describe_pair(first: cq.Shape, second: cq.Shape) -> dict[str, Any]:
    distance, first_point, second_point = _distance(first, second)
    angle = _angle(first, second)
    return {
        "distance": distance,
        "first_point": first_point,
        "second_point": second_point,
        "angle_degrees": angle[0] if angle else None,
        "angle_kind": angle[1] if angle else None,
    }


def _distance(
    first: cq.Shape, second: cq.Shape
) -> tuple[float, list[float], list[float]]:
    operation = BRepExtrema_DistShapeShape(first.wrapped, second.wrapped)
    if not operation.IsDone() or operation.NbSolution() < 1:
        raise MeasurementError("OCCT could not calculate the distance")
    return (
        float(operation.Value()),
        _point(operation.PointOnShape1(1)),
        _point(operation.PointOnShape2(1)),
    )


def _angle(first: cq.Shape, second: cq.Shape) -> tuple[float, str] | None:
    if isinstance(first, cq.Edge) and isinstance(second, cq.Edge):
        if first.geomType() != "LINE" or second.geomType() != "LINE":
            return None
        return _vector_angle(first.tangentAt(), second.tangentAt()), "edge_edge"
    if isinstance(first, cq.Face) and isinstance(second, cq.Face):
        if first.geomType() != "PLANE" or second.geomType() != "PLANE":
            return None
        return _vector_angle(first.normalAt(), second.normalAt()), "face_face"
    if isinstance(first, cq.Edge) and isinstance(second, cq.Face):
        return _edge_face_angle(first, second)
    if isinstance(first, cq.Face) and isinstance(second, cq.Edge):
        return _edge_face_angle(second, first)
    return None


def _edge_face_angle(edge: cq.Edge, face: cq.Face) -> tuple[float, str] | None:
    if edge.geomType() != "LINE" or face.geomType() != "PLANE":
        return None
    tangent = edge.tangentAt().normalized()
    normal = face.normalAt().normalized()
    # An edge lying in the plane is 0 degrees; one normal to it is 90.
    degrees = math.degrees(math.asin(abs(_clamped_dot(tangent, normal))))
    return degrees, "edge_face"


def _vector_angle(first: cq.Vector, second: cq.Vector) -> float:
    # Lines and planes are unoriented geometric entities, so report the
    # smaller angle regardless of edge direction or face orientation.
    dot = abs(_clamped_dot(first.normalized(), second.normalized()))
    return math.degrees(math.acos(dot))


def _clamped_dot(first: cq.Vector, second: cq.Vector) -> float:
    return max(-1.0, min(1.0, first.dot(second)))


def _point(value: Any) -> list[float]:
    if hasattr(value, "toTuple"):
        value = value.toTuple()
    else:
        value = (value.X(), value.Y(), value.Z())
    return [float(component) for component in value]
