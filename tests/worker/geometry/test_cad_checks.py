"""Script requirements are visible in the GUI and gate headless builds."""

import json

import cadquery as cq
import pytest

from cq_worker.cad_checks import CheckSuite
from cq_worker.execution import Execution, RunSettings
from cq_worker.cli import main


def test_expectations_collect_all_results_and_script_lines():
    source = (
        "part = cq.Workplane().box(2, 3, 4)\n"
        "other = cq.Workplane().box(1, 1, 1).translate((10, 0, 0))\n"
        "expect(part, 'box').is_valid().volume_between(20, 30)\n"
        "expect(part, 'size').dimensions_within(x=(2, 2), y=(3, 3))\n"
        "expect(part, 'space').clearance_at_least(other, 5)\n"
        "expect(part, 'wrong volume').volume_between(1, 2)\n"
        "show_object(part)\n"
    )
    execution = Execution(source, None, RunSettings())
    assert execution.run()
    assert [row["ok"] for row in execution.checks.results] == [
        True,
        True,
        True,
        True,
        False,
    ]
    assert [row["line"] for row in execution.checks.results] == [3, 3, 4, 5, 6]
    assert not execution.checks.ok


def test_invalid_or_unmeasurable_shape_fails_without_hiding_other_results():
    suite = CheckSuite()
    face = cq.Workplane().box(1, 1, 1).val().Faces()[0]
    suite.expect(face, "face", 4).volume_between(1, 2).is_valid()
    assert suite.results[0]["ok"] is False
    assert "at least one solid" in suite.results[0]["detail"]
    assert suite.results[1]["ok"] is True


def test_clearance_rejects_touching_and_overlapping_solids():
    suite = CheckSuite()
    first = cq.Workplane().box(2, 2, 2)
    touching = cq.Workplane().box(2, 2, 2).translate((2, 0, 0))
    overlapping = cq.Workplane().box(2, 2, 2).translate((1, 0, 0))
    suite.expect(first, "touch", 1).clearance_at_least(touching, 0.1)
    suite.expect(first, "overlap", 2).clearance_at_least(overlapping, 0.1)
    assert [row["ok"] for row in suite.results] == [False, False]


def test_bad_requirement_arguments_are_script_errors():
    suite = CheckSuite()
    box = cq.Workplane().box(1, 1, 1)
    with pytest.raises(ValueError, match="minimum exceeds maximum"):
        suite.expect(box, None, 1).volume_between(3, 2)
    with pytest.raises(ValueError, match="finite"):
        suite.expect(box, None, 1).clearance_at_least(box, float("nan"))
    assert suite.results == []


def test_cli_check_reports_all_assertions_and_build_does_not_export(tmp_path, capsys):
    script = tmp_path / "part.py"
    script.write_text(
        "part = cq.Workplane().box(2, 3, 4)\n"
        "expect(part, 'volume').volume_between(1, 2)\n"
        "expect(part, 'topology').is_valid()\n"
        "show_object(part)\n",
        encoding="utf-8",
    )
    assert main(["check", str(script), "--json"]) == 1
    report = json.loads(capsys.readouterr().out)
    assert "reports" in report, report
    assert report["reports"][0]["valid"] is True
    assert [(check["name"], check["ok"]) for check in report["checks"]] == [
        ("volume", False),
        ("topology", True),
    ]
    output = tmp_path / "part.step"
    assert main(["build", str(script), "--export", str(output), "--json"]) == 1
    failure = json.loads(capsys.readouterr().out)
    assert failure["checks"][0]["line"] == 2
    assert not output.exists()
