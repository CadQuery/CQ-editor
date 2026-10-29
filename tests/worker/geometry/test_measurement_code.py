"""Copy as code: Python that measures the selection again, verified."""

import textwrap

import cadquery as cq
import pytest

from cq_worker import measurement_code as module
from cq_worker.execution import Execution, RunSettings
from cq_worker.measurement import MeasurementError
from cq_worker.measurement_code import measurement_code
from cq_worker.scene import Registry, SceneBuilder
from cq_worker.tessellation import Tolerance

SCRIPT = """\
plate = cq.Workplane().box(20, 10, 4).faces(">Z").workplane().hole(3)
assy = cq.Assembly().add(
    plate, name="plate", loc=cq.Location((0, 0, 50), (0, 0, 1), 30)
)
assy.add(cq.Workplane().cylinder(6, 2), name="pin", loc=cq.Location((30, 0, 0)))
show_object(plate)
show_object(assy)
"""


@pytest.fixture
def shown():
    """The script run, shown and meshed as the GUI shows it."""

    execution = Execution(SCRIPT, None, RunSettings())
    assert execution.run(), execution.error
    registry = Registry()
    nodes = SceneBuilder(registry, Tolerance()).build(execution.shown)
    return registry, dict(execution.namespace), nodes


def entity(key, kind, identifier):
    return {"key": key, "part": 0, "kind": kind, "identifier": identifier}


def body(code):
    """The measuring lines, without the imports and the helper."""

    return [
        line
        for line in code.splitlines()
        if line and not line.startswith(("from ", "import ", "def ", "    ", '"""'))
    ]


def test_two_entities_become_selectors_measurements_and_their_relationship(shown):
    registry, namespace, (plate, _) = shown
    top = entity(plate["key"], "face", 6)
    edge = entity(plate["key"], "edge", 1)

    result = measurement_code(registry, namespace, [top, edge])

    assert result["verified"] and result["problem"] is None
    lines = body(result["code"])
    assert lines[0] == 'face_1 = plate.faces(">X").val()'
    assert "area_1 = face_1.Area()  # 40" in lines
    assert "bounds_1 = exact_bounds(face_1)  # (10, -5, -2, 10, 5, 2)" in lines
    assert any(line.startswith("edge_2 = plate.edges(") for line in lines)
    assert "distance = face_1.distance(edge_2)  # 20" in lines
    assert result["code"].startswith("import math\n")


def test_the_code_runs_in_the_console_and_gives_the_measured_values(shown):
    registry, namespace, (plate, _) = shown
    result = measurement_code(registry, namespace, [entity(plate["key"], "face", 6)])
    scope = dict(namespace)
    exec(result["code"], scope)
    assert scope["area_1"] == pytest.approx(40)
    assert scope["bounds_1"] == pytest.approx((10, -5, -2, 10, 5, 2))


def test_assembly_parts_are_placed_where_the_assembly_puts_them(shown):
    registry, namespace, (_, assembly) = shown
    plate_part, pin_part = assembly["children"]

    rotated = measurement_code(registry, namespace, [], [plate_part["key"]])
    assert rotated["verified"], rotated["problem"]
    assert body(rotated["code"])[0] == (
        "shape_1 = plate.val().moved(cq.Location((0, 0, 50), (0, 0, 30)))"
    )

    pin = measurement_code(registry, namespace, [entity(pin_part["key"], "face", 1)])
    assert pin["verified"], pin["problem"]
    # Added without a variable: reached through the assembly.
    assert body(pin["code"])[0].startswith("face_1 = assy.objects['pin'].obj.faces(")
    assert body(pin["code"])[0].endswith(".val().moved(cq.Location((30, 0, 0)))")


def test_named_assembly_face_is_referenced_by_its_script_name():
    script = """\
block = cq.Workplane().box(10, 10, 10)
model = cq.Assembly(block, name='block')
model.addSubshape(block.faces('>Z').val(), name='lid', color=cq.Color('red'), layer='finish')
show_object(model)
"""
    execution = Execution(script, None, RunSettings())
    assert execution.run(), execution.error
    registry = Registry()
    (node,) = SceneBuilder(registry, Tolerance()).build(execution.shown)
    (attribute,) = node["parts"][0]["face_attributes"]

    result = measurement_code(
        registry,
        dict(execution.namespace),
        [entity(node["key"], "face", attribute["id"])],
    )

    assert result["verified"], result["problem"]
    assert "name == 'lid'" in result["code"]
    scope = dict(execution.namespace)
    exec(result["code"], scope)
    assert scope["area_1"] == pytest.approx(100)


def test_objects_are_measured_whole(shown):
    registry, namespace, (plate, assembly) = shown
    result = measurement_code(registry, namespace, [], [plate["key"], assembly["key"]])
    assert result["verified"], result["problem"]
    lines = body(result["code"])
    assert "shape_1 = plate.val()" in lines
    assert "shape_2 = assy.toCompound()" in lines
    assert any(line.startswith("volume_2 = shape_2.Volume()  # ") for line in lines)


def test_index_fallbacks_are_marked_in_the_code(shown, monkeypatch):
    from cq_worker import selection

    monkeypatch.setattr(selection, "search", lambda features, targets: [])
    registry, namespace, (plate, _) = shown
    result = measurement_code(registry, namespace, [entity(plate["key"], "face", 6)])
    assert result["verified"], result["problem"]
    assert "# By index: this may name another entity" in result["code"]
    assert "face_1 = plate.faces()[5].val()" in result["code"]


def test_objects_without_a_variable_are_left_to_the_user():
    execution = Execution(
        "show_object(cq.Workplane().box(1, 2, 3))\n", None, RunSettings()
    )
    assert execution.run(), execution.error
    registry = Registry()
    (node,) = SceneBuilder(registry, Tolerance()).build(execution.shown)
    result = measurement_code(registry, dict(execution.namespace), [], [node["key"]])
    assert not result["verified"]
    assert "replace obj" in result["problem"]
    assert "shape_1 = obj.val()" in result["code"]


def test_a_variable_rebound_in_the_console_is_not_used(shown):
    registry, namespace, (plate, _) = shown
    namespace["plate"] = cq.Workplane().box(1, 1, 1)
    result = measurement_code(registry, namespace, [entity(plate["key"], "face", 6)])
    # The shown plate is still reachable, through the assembly holding it.
    assert result["verified"], result["problem"]
    assert "face_1 = assy.objects['plate'].obj.faces(" in result["code"]

    del namespace["assy"]
    result = measurement_code(registry, namespace, [entity(plate["key"], "face", 6)])
    assert not result["verified"]
    assert "face_1 = obj.faces(" in result["code"]


def test_verification_reports_code_that_disagrees():
    assert module._verify("x = 1.0\n", {}, {"x": 1.0}) is None
    assert "x came out as 2" in module._verify("x = 2.0\n", {}, {"x": 1.0})
    assert "ZeroDivisionError" in module._verify("x = 1 / 0\n", {}, {"x": 1.0})
    assert "(1, 2, 3)" in module._verify(
        "v = cq.Vector(1, 2, 4)\n", {"cq": cq}, {"v": [1.0, 2.0, 3.0]}
    )


def test_stale_selections_are_errors(shown):
    registry, namespace, (plate, _) = shown
    registry.release([plate["key"]])
    with pytest.raises(MeasurementError, match="previous Python session"):
        measurement_code(registry, namespace, [entity(plate["key"], "face", 1)])
    with pytest.raises(MeasurementError, match="or objects"):
        measurement_code(registry, namespace, [], [])


def test_numbers_are_written_short_when_rounding_noise_allows():
    assert module._number(29.999999999999993) == "30"
    assert module._number(-0.0) == "0"
    assert module._number(2.5) == "2.5"
    assert module._number(1 / 3) == repr(1 / 3)
    assert module._text(2.7755575615628914e-17) == "0"


def test_placements_drop_what_is_zero():
    assert module._placement(cq.Location()) == ""
    assert module._placement(cq.Location((1, 2, 3))) == ".moved(cq.Location((1, 2, 3)))"
    rotated = module._placement(cq.Location((0, 0, 0), (0, 0, 1), 90))
    assert rotated == ".moved(cq.Location((0, 0, 0), (0, 0, 90)))"


def test_code_text_is_valid_python_with_and_without_helpers():
    code = module._Code()
    code.assign("x", "1", 1.0)
    compile(code.text(), "<test>", "exec")
    code.uses_math = True
    code.uses_bounds = True
    text = code.text()
    compile(text, "<test>", "exec")
    assert textwrap.dedent(text).count("def exact_bounds") == 1
