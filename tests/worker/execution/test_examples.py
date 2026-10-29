"""Smoke tests for every bundled example.

The examples 01 to 08 shipped before the Rust rewrite and are compatibility
fixtures; later examples are additions held to the same standard.
"""

from pathlib import Path

import pytest

from cq_worker.execution import Execution, RunSettings

EXAMPLES = Path(__file__).parents[3] / "examples"
LEGACY_EXAMPLES = sorted(EXAMPLES.glob("*.py"))


@pytest.mark.parametrize("path", LEGACY_EXAMPLES, ids=lambda path: path.name)
def test_legacy_example_executes_and_produces_a_model(path):
    execution = Execution(path.read_text(encoding="utf-8"), str(path), RunSettings())
    execution.run()

    assert execution.error is None, execution.error
    assert execution.shown, f"{path.name} did not produce a visible model"
