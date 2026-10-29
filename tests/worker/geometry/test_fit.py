import math

import pytest

from cq_worker.execution import Execution, RunSettings
from cq_worker.fit import FitError, cylindrical_fit, require_fit


def test_worst_case_fit_uses_both_tolerance_limits_and_axis_offset():
    report = cylindrical_fit(
        hole_diameter=6.5,
        pin_diameter=6,
        hole_lower=-0.1,
        hole_upper=0.1,
        pin_lower=-0.05,
        pin_upper=0.05,
        center_offset=0.1,
        required_clearance=0.1,
    )
    assert report["minimum_clearance"] == pytest.approx(0.15)
    assert report["maximum_clearance"] == pytest.approx(0.45)
    assert report["ok"]


def test_oversize_pin_and_shallow_counterbore_are_rejected():
    report = cylindrical_fit(
        hole_diameter=5,
        pin_diameter=6,
        insertion_depth=6,
        hole_depth=5,
        head_diameter=10,
        counterbore_diameter=9,
        head_height=3,
        counterbore_depth=2,
    )
    assert len(report["issues"]) == 4
    with pytest.raises(FitError, match="Worst-case"):
        require_fit(hole_diameter=5, pin_diameter=6)


@pytest.mark.parametrize(
    "options",
    [
        {"hole_diameter": math.nan, "pin_diameter": 6},
        {"hole_diameter": 6, "pin_diameter": 0},
        {"hole_diameter": 6, "pin_diameter": 5, "hole_lower": 0.1, "hole_upper": -0.1},
        {"hole_diameter": 6, "pin_diameter": 5, "insertion_depth": 3},
    ],
)
def test_invalid_fit_declarations_fail_explicitly(options):
    with pytest.raises(ValueError):
        cylindrical_fit(**options)


def test_fit_requirement_fails_the_script_and_headless_build():
    execution = Execution(
        "require_fit(hole_diameter=5, pin_diameter=6)\n"
        "show_object(cq.Workplane().box(1,1,1))\n",
        None,
        RunSettings(),
    )
    assert not execution.run()
    assert execution.error["type_name"] == "FitError"
