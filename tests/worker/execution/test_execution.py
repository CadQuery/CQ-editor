import sys
import textwrap

from cq_worker.execution import SCRIPT_FILENAME, Execution, RunSettings, rand_color


def run(source: str, path=None, **settings) -> Execution:
    execution = Execution(textwrap.dedent(source), path, RunSettings(**settings))
    execution.run()
    return execution


def test_show_object_names_from_variables():
    execution = run("""
        result = cq.Workplane().box(1, 1, 1)
        show_object(result)
        show_object(cq.Workplane().sphere(1), name="ball", options={"color": "red"})
        show_object(cq.Workplane().sphere(2), alpha=0.5)
        """)

    assert execution.error is None
    assert list(execution.shown) == ["result", "ball", "Workplane 1"]
    assert execution.shown["ball"].options == {"color": "red"}
    assert execution.shown["Workplane 1"].options == {"alpha": 0.5}


def test_injected_api_is_removed_afterwards():
    execution = run("x = 1")
    assert "show_object" not in execution.namespace
    assert "cq" in execution.namespace
    assert execution.namespace["__name__"] == "__cq_main__"


def test_debug_overlays_in_translucent_red():
    execution = run("debug(cq.Workplane().box(1, 1, 1), name='probe')")
    assert execution.shown["probe"].options == {"color": "red", "alpha": 0.2}


def test_runtime_errors_point_at_the_script_line():
    execution = run("""
        a = 1
        b = a / 0
        """)

    error = execution.error
    assert error["type_name"] == "ZeroDivisionError"
    assert error["line"] == 3
    assert error["frames"][0]["file"] == SCRIPT_FILENAME
    assert error["frames"][0]["code"] == "b = a / 0"


def test_syntax_errors_are_reported():
    execution = run("x = (1,\n")
    assert execution.error["type_name"] == "SyntaxError"
    assert execution.error["line"] is not None


def test_errors_inside_script_functions_keep_the_call_chain():
    execution = run("""
        def inner():
            raise ValueError("boom")

        inner()
        """)
    lines = [f["line"] for f in execution.error["frames"]]
    assert lines == [5, 3]
    assert execution.error["message"] == "boom"


def test_sys_exit_ends_the_script_quietly():
    execution = run("import sys\nx = 1\nsys.exit()\ny = 2")
    assert execution.error is None
    assert "x" in execution.namespace and "y" not in execution.namespace


def test_rand_color_is_reproducible():
    first = run("c = rand_color()").namespace["c"]
    second = run("c = rand_color()").namespace["c"]
    assert first == second
    assert first["alpha"] == 1.0
    assert all(10 <= v < 100 for v in first["color"])
    assert len(rand_color(cfloat=True)) == 4


def test_script_directory_and_module_reloading(tmp_path):
    helper = tmp_path / "helper_module_for_test.py"
    script = tmp_path / "main.py"
    helper.write_text("VALUE = 1\n")

    source = "import os, helper_module_for_test\nvalue = helper_module_for_test.VALUE\ncwd = os.getcwd()"
    first = run(source, path=str(script))
    assert first.error is None
    assert first.namespace["value"] == 1
    assert first.namespace["cwd"] == str(tmp_path)
    assert "helper_module_for_test" not in sys.modules
    assert str(tmp_path) not in sys.path

    # A different length, so the stale bytecode cache cannot match even when
    # both writes fall within the same second.
    helper.write_text("VALUE = 22\n")
    assert run(source, path=str(script)).namespace["value"] == 22


def test_settings_from_message_ignore_unknown_keys():
    settings = RunSettings.from_message({"reload_cq": True, "bogus": 1})
    assert settings.reload_cq is True
    assert settings.change_working_dir is True


def test_long_values_are_truncated():
    from cq_worker.execution import MAX_VALUE_LENGTH, describe_variables

    (row,) = describe_variables({"long": "x" * 5000}, hidden=set())
    assert len(row["value"]) == MAX_VALUE_LENGTH
    assert row["value"].endswith("…")


def test_duplicate_show_object_names_emit_a_warning_once_per_name(caplog):
    import logging

    with caplog.at_level(logging.WARNING, logger="script"):
        execution = run("""
            for i in range(5):
                part = cq.Workplane("XY").box(10, 10, 10)
                show_object(part, name="part")
                inferred = cq.Workplane("XY").sphere(2)
                show_object(inferred)
            """)

    assert execution.error is None
    assert list(execution.shown) == ["part", "inferred"]
    warnings = [r.getMessage() for r in caplog.records if r.name == "script"]
    assert len(warnings) == 2
    assert '"part"' in warnings[0]
    assert '"inferred"' in warnings[1]
    assert "replaced" in warnings[0]
    assert "distinct names" in warnings[0]


