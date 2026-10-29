"""CAD advice supplements, but never replaces, the Python traceback."""

import json

from cq_worker.cli import main
from cq_worker.diagnostics import diagnose
from cq_worker.execution import Execution, RunSettings


def test_failed_fillet_keeps_the_traceback_and_links_the_cad_call():
    source = (
        "part = cq.Workplane().box(1, 1, 1)\n" "result = part.edges().fillet(100)\n"
    )
    execution = Execution(source, None, RunSettings())
    assert not execution.run()
    error = execution.error
    assert error is not None
    assert "BRep_API: command not done" in error["text"]
    assert error["line"] == 2
    advice = error["diagnostic"]
    assert advice["operation"] == "fillet"
    assert advice["line"] == 2
    assert any("radius" in hint for hint in advice["hints"])


def test_selector_advice_needs_a_related_error():
    source = 'selected = part.faces(">Z")'
    advice = diagnose(ValueError("Nothing selected"), source, 1)
    assert advice is not None
    assert advice["operation"] == "faces"
    assert advice["receiver"] == "part"
    assert diagnose(RuntimeError("unrelated failure"), source, 1) is None
    assert diagnose(ValueError("Nothing selected"), source, None) is None


def test_syntax_errors_and_unrelated_python_lines_get_no_cad_advice():
    assert diagnose(SyntaxError("invalid syntax"), 'part.faces(">Z")', 1) is None
    assert diagnose(RuntimeError("fillet example"), "part.fillet(1)", 1) is None
    assert (
        diagnose(RuntimeError("BRep_API: command not done"), "raise RuntimeError()", 1)
        is None
    )


def test_headless_json_includes_advice_with_the_original_error(tmp_path, capsys):
    script = tmp_path / "failed_fillet.py"
    script.write_text(
        "part = cq.Workplane().box(1, 1, 1)\n" "part.edges().fillet(100)\n",
        encoding="utf-8",
    )
    assert main(["check", str(script), "--json"]) == 1
    payload = json.loads(capsys.readouterr().out)
    assert payload["diagnostic"]["operation"] == "fillet"
    assert "StdFail_NotDone" in payload["traceback"]
