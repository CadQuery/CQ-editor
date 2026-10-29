"""*Copy as code* for the Measurements panel.

The generated Python measures the same entities or objects again from the
script's own variables, with a selector for each picked entity, so it can
be pasted into the console or kept in the script. Every measured value is
written as a comment on its line.

The code is executed on a copy of the console's namespace (which holds the
variables of the last run) and its results are compared with the exact
measurements before it is called verified.
"""

from __future__ import annotations

import math
from typing import Any

import cadquery as cq

from . import measurement, selection
from .measurement import TopologyReference
from .scene import Registry

#: Relative and absolute agreement required between code and measurement.
TOLERANCE = 1e-9

_HELPER = '''\
from OCP.Bnd import Bnd_Box
from OCP.BRepBndLib import BRepBndLib


def exact_bounds(shape):
    """(xmin, ymin, zmin, xmax, ymax, zmax) of the exact geometry; shapes on
    screen are meshed, and BoundingBox() would include the mesh."""
    box = Bnd_Box()
    BRepBndLib.AddOptimal_s(shape.wrapped, box, False, False)
    low, high = box.CornerMin(), box.CornerMax()
    return (low.X(), low.Y(), low.Z(), high.X(), high.Y(), high.Z())
'''


class _Code:
    """Lines of code, with the value each assignment must produce."""

    def __init__(self) -> None:
        self.lines: list[str] = []
        self.expected: dict[str, Any] = {}
        self.uses_math = False
        self.uses_bounds = False

    def assign(self, name: str, expression: str, value: Any = None) -> None:
        if value is None:
            self.lines.append(f"{name} = {expression}")
            return
        self.lines.append(f"{name} = {expression}  # {_text(value)}")
        self.expected[name] = value

    def blank(self) -> None:
        if self.lines and self.lines[-1]:
            self.lines.append("")

    def text(self) -> str:
        header = ["import math"] if self.uses_math else []
        if self.uses_bounds:
            header += ([""] if header else []) + _HELPER.splitlines()
        body = self.lines[:-1] if self.lines and not self.lines[-1] else self.lines
        return "\n".join(header + ([""] if header else []) + body) + "\n"


def measurement_code(
    registry: Registry,
    namespace: dict[str, Any],
    raw_selections: Any,
    raw_objects: Any = None,
) -> dict[str, Any]:
    """Code measuring the selection again, and whether it was verified."""

    result = measurement.measure(registry, raw_selections, raw_objects)
    references, _ = measurement.resolve(registry, raw_selections, raw_objects)
    code = _Code()
    problems: list[str] = []

    if references:
        names = []
        for number, (reference, entity) in enumerate(
            zip(references, result["entities"], strict=True), start=1
        ):
            names.append(
                _entity(code, registry, namespace, reference, entity, number, problems)
            )
            code.blank()
        if result["pair"] is not None:
            _pair(code, references, names, result["pair"])
    else:
        for number, described in enumerate(result["objects"], start=1):
            _object(code, registry, namespace, described, number, problems)
            code.blank()

    text = code.text()
    if problems:
        return {"code": text, "verified": False, "problem": problems[0]}
    problem = _verify(text, namespace, code.expected)
    return {"code": text, "verified": problem is None, "problem": problem}


# -- entities ---------------------------------------------------------------------


def _entity(
    code: _Code,
    registry: Registry,
    namespace: dict[str, Any],
    reference: TopologyReference,
    described: dict[str, Any],
    number: int,
    problems: list[str],
) -> str:
    entry = registry.get(reference.key)
    source = entry.sources[reference.part]
    named = None
    if reference.kind == "face" and isinstance(entry.obj, cq.Assembly):
        attribute = next(
            (
                item
                for item in entry.face_attributes
                if item["id"] == reference.identifier
            ),
            None,
        )
        if attribute is not None and attribute.get("name"):
            label = attribute["name"]
            if list(entry.obj._subshape_names.values()).count(label) == 1:
                assembly = _variable(namespace, entry.obj) or _assembly_path(
                    namespace, entry.obj
                )
                if assembly is not None:
                    named = (
                        f"next(face for face, name in {assembly}._subshape_names.items() "
                        f"if name == {label!r})"
                    )
    if named is not None:
        expression = named
    else:
        receiver = _receiver(namespace, source.obj, problems)
        candidate = selection.selector_candidates(registry, [_message(reference)])[
            "candidates"
        ][0]
        expression = receiver + candidate["expression"]
        if isinstance(source.obj, cq.Workplane):
            expression += ".val()"
    expression += _placement(source.location)
    name = f"{reference.kind}_{number}"
    if named is None and candidate["index_based"]:
        code.lines.append(
            "# By index: this may name another entity once the model changes"
        )
    code.assign(name, expression)

    if reference.kind == "face":
        code.assign(f"area_{number}", f"{name}.Area()", described["area"])
    elif reference.kind == "edge":
        code.assign(f"length_{number}", f"{name}.Length()", described["length"])
        if described["geometry_type"] == "circle":
            code.assign(f"radius_{number}", f"{name}.radius()", described["radius"])
    if reference.kind == "vertex":
        code.assign(f"point_{number}", f"{name}.toTuple()", described["position"])
    else:
        code.assign(f"center_{number}", f"{name}.Center()", described["center"])
    box = described["bounding_box"]
    code.uses_bounds = True
    code.assign(f"bounds_{number}", f"exact_bounds({name})", [*box["min"], *box["max"]])
    return name


def _pair(
    code: _Code,
    references: list[TopologyReference],
    names: list[str],
    pair: dict[str, Any],
) -> None:
    first, second = names
    code.assign("distance", f"{first}.distance({second})", pair["distance"])
    kind = pair.get("angle_kind")
    if kind is None:
        return
    code.uses_math = True
    direction = {"edge": "tangentAt()", "face": "normalAt()"}
    a = f"{first}.{direction[references[0].kind]}"
    b = f"{second}.{direction[references[1].kind]}"
    between = f"math.degrees({a}.getAngle({b}))"
    if kind == "edge_face":
        # 0 degrees for an edge lying in the plane, 90 for one normal to it.
        code.assign("angle", f"abs(90 - {between})", pair["angle_degrees"])
    else:
        # Lines and planes have no direction: the smaller angle.
        code.assign("between", between)
        code.assign("angle", "min(between, 180 - between)", pair["angle_degrees"])


# -- objects ----------------------------------------------------------------------


def _object(
    code: _Code,
    registry: Registry,
    namespace: dict[str, Any],
    described: dict[str, Any],
    number: int,
    problems: list[str],
) -> None:
    entry = registry.get(described["key"])
    expression = _object_expression(entry, namespace, problems)
    name = f"shape_{number}"
    code.assign(name, expression)
    if described["volume"] is not None:
        code.assign(f"volume_{number}", f"{name}.Volume()", described["volume"])
    if described["area"] is not None:
        code.assign(f"area_{number}", f"{name}.Area()", described["area"])
    code.assign(f"center_{number}", f"{name}.Center()", described["center"])
    box = described["bounding_box"]
    code.uses_bounds = True
    code.assign(f"bounds_{number}", f"exact_bounds({name})", [*box["min"], *box["max"]])


def _object_expression(entry, namespace: dict[str, Any], problems: list[str]) -> str:
    obj = entry.obj
    if isinstance(obj, cq.Assembly):
        if entry.children or len(entry.sources) != 1:
            name = _variable(namespace, obj) or _assembly_path(namespace, obj)
            if name is None:
                problems.append("The assembly is not a variable of the script")
                return "assembly.toCompound()"
            # toCompound() applies the node's own location and its children's,
            # not those of the assemblies above it.
            above = (
                entry.sources[0].location * obj.loc.inverse if entry.sources else None
            )
            return f"{name}.toCompound(){_placement(above) if above else ''}"
        source = entry.sources[0]
        return _shape_of(_receiver(namespace, source.obj, problems), source.obj) + (
            _placement(source.location)
        )
    return _shape_of(_receiver(namespace, obj, problems), obj)


def _shape_of(receiver: str, obj: Any) -> str:
    """One shape for what a shown object holds."""

    if isinstance(obj, cq.Workplane):
        shapes = [value for value in obj.vals() if isinstance(value, cq.Shape)]
        if len(shapes) == 1 and len(obj.vals()) == 1:
            return f"{receiver}.val()"
        return (
            f"cq.Compound.makeCompound([v for v in {receiver}.vals() "
            "if isinstance(v, cq.Shape)])"
        )
    return receiver


# -- names and placements -----------------------------------------------------------


def _receiver(namespace: dict[str, Any], obj: Any, problems: list[str]) -> str:
    name = _variable(namespace, obj) or _assembly_path(namespace, obj)
    if name is None:
        problems.append(
            f"The {type(obj).__name__} is not a variable of the script: "
            "replace obj with an expression for it"
        )
        return "obj"
    return name


def _variable(namespace: dict[str, Any], obj: Any) -> str | None:
    for name, value in namespace.items():
        if value is obj and name.isidentifier() and not name.startswith("_"):
            return name
    return None


def _assembly_path(namespace: dict[str, Any], obj: Any) -> str | None:
    """``assembly.objects["name"].obj`` for an object added to an assembly
    of the script without a variable of its own."""

    for name, value in namespace.items():
        if not isinstance(value, cq.Assembly) or name.startswith("_"):
            continue
        for key, node in value.objects.items():
            if node is obj:
                return f"{name}.objects[{key!r}]"
            if node.obj is obj:
                return f"{name}.objects[{key!r}].obj"
    return None


def _placement(location: cq.Location) -> str:
    translation, angles = location.toTuple()
    if all(abs(value) < 1e-12 for value in angles):
        if all(abs(value) < 1e-12 for value in translation):
            return ""
        return f".moved(cq.Location({_vector(translation)}))"
    return f".moved(cq.Location({_vector(translation)}, {_vector(angles)}))"


def _vector(values) -> str:
    return "(" + ", ".join(_number(value) for value in values) + ")"


def _number(value: float) -> str:
    value = float(value)
    # 29.999999999999993 degrees is 30 within rounding noise.
    short = round(value, 6)
    if math.isclose(short, value, rel_tol=1e-12, abs_tol=1e-12):
        value = short
    if value == 0:
        return "0"
    if value == int(value) and abs(value) < 1e15:
        return str(int(value))
    return repr(value)


def _message(reference: TopologyReference) -> dict:
    return {
        "key": reference.key,
        "part": reference.part,
        "kind": reference.kind,
        "identifier": reference.identifier,
    }


# -- verification -----------------------------------------------------------------


def _verify(
    text: str, namespace: dict[str, Any], expected: dict[str, Any]
) -> str | None:
    """``None`` when the code runs and reproduces every expected value."""

    scope = dict(namespace)
    try:
        exec(compile(text, "<measurement code>", "exec"), scope)
    except Exception as error:
        return f"The code failed: {type(error).__name__}: {error}"
    for name, value in expected.items():
        if not _close(scope.get(name), value):
            return f"{name} came out as {_text(scope.get(name))}, not {_text(value)}"
    return None


def _close(actual: Any, expected: Any) -> bool:
    if isinstance(actual, cq.Vector):
        actual = actual.toTuple()
    if isinstance(expected, (list, tuple)):
        if not isinstance(actual, (list, tuple)) or len(actual) != len(expected):
            return False
        return all(_close(a, e) for a, e in zip(actual, expected))
    try:
        return math.isclose(
            float(actual), float(expected), rel_tol=TOLERANCE, abs_tol=TOLERANCE
        )
    except (TypeError, ValueError):
        return False


def _text(value: Any) -> str:
    if isinstance(value, cq.Vector):
        value = value.toTuple()
    if isinstance(value, (list, tuple)):
        return "(" + ", ".join(_text(v) for v in value) + ")"
    if isinstance(value, float):
        # Exact geometry carries rounding noise such as 2.8e-17.
        text = "0" if abs(value) < 1e-12 else f"{value:.10g}"
        return text
    return repr(value)
