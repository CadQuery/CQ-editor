"""The script lines that created CadQuery objects (Go to source)."""

import textwrap

import cadquery as cq

from cq_worker import inspector
from cq_worker.execution import Execution, RunSettings
from cq_worker.provenance import creation_line
from cq_worker.scene import Registry, SceneBuilder
from cq_worker.tessellation import Tolerance


def run(source, path=None):
    execution = Execution(textwrap.dedent(source), path, RunSettings())
    assert execution.run(), execution.error
    return execution


def test_each_object_remembers_the_line_that_created_it():
    execution = run("""\
        base = cq.Workplane("XY")
        plate = (
            base.box(10, 10, 2)
            .faces(">Z")
            .workplane()
            .hole(3)
        )
        solid = cq.Solid.makeBox(1, 2, 3)
        sketch = cq.Sketch().rect(2, 2)
        """)
    names = execution.namespace
    assert creation_line(names["base"]) == 1
    # Each step of a chain points to its own line.
    assert creation_line(names["plate"]) == 6
    assert creation_line(names["plate"].parent) == 5
    assert creation_line(names["plate"].parent.parent.parent) == 3
    assert creation_line(names["plate"].val()) == 6
    assert creation_line(names["solid"]) == 8
    assert creation_line(names["sketch"]) == 9


def test_objects_made_in_functions_and_modules_point_into_the_script(tmp_path):
    (tmp_path / "helpers.py").write_text(
        "import cadquery as cq\n\n\ndef peg():\n    return cq.Workplane().cylinder(4, 1)\n"
    )
    script = tmp_path / "part.py"
    source = """\
        import helpers

        def tab(width):
            return cq.Workplane().box(width, 1, 1)

        made_here = tab(3)
        made_there = helpers.peg()
        """
    script.write_text(textwrap.dedent(source))
    names = run(source, str(script)).namespace
    # Inside the script's own function...
    assert creation_line(names["made_here"]) == 4
    # ...and, for a helper module, the script line that called it.
    assert creation_line(names["made_there"]) == 7


def test_assembly_children_point_to_their_add_call():
    names = run("""\
        box = cq.Workplane().box(1, 1, 1)
        assembly = cq.Assembly()
        assembly.add(box, name="first")
        assembly.add(
            cq.Workplane().sphere(1),
            name="second",
        )
        """).namespace
    assembly = names["assembly"]
    assert creation_line(assembly) == 2
    assert [creation_line(child) for child in assembly.children] == [3, 4]


def test_constructors_are_restored_after_a_run_even_a_failing_one():
    originals = (cq.Workplane.__init__, cq.Shape.__init__, cq.Assembly.__init__)
    run("x = cq.Workplane().box(1, 1, 1)")
    failing = Execution("cq.Workplane().box(1, 1, 1)\n1 / 0\n", None, RunSettings())
    assert not failing.run()
    assert (cq.Workplane.__init__, cq.Shape.__init__, cq.Assembly.__init__) == originals
    # Outside a run nothing is recorded.
    assert creation_line(cq.Workplane().box(1, 1, 1)) is None


def test_scene_nodes_and_stack_frames_carry_their_lines():
    execution = run("""\
        plate = cq.Workplane().box(4, 4, 1).faces(">Z").workplane().hole(1)
        assembly = cq.Assembly().add(cq.Workplane().sphere(1), name="ball")
        show_object(plate)
        show_object(assembly)
        """)
    builder = SceneBuilder(Registry(), Tolerance())
    plate, assembly = builder.build(execution.shown)
    assert plate["line"] == 1
    assert assembly["line"] == 2
    assert assembly["children"][0]["line"] == 2

    frames = inspector.describe_stack(
        inspector.stack_frames(execution.namespace["plate"])
    )
    assert {frame["line"] for frame in frames} == {1}
    assert frames[0]["items"][0]["line"] == 1


def test_a_node_without_a_creation_line_points_to_its_show_object_call():
    made_before = cq.Workplane().box(1, 1, 1)  # created outside any run
    execution = Execution("\nshow_object(prebuilt)\n", None, RunSettings())
    execution.namespace["prebuilt"] = made_before
    assert execution.run(), execution.error
    (node,) = SceneBuilder(Registry(), Tolerance()).build(execution.shown)
    assert node["line"] == 2
