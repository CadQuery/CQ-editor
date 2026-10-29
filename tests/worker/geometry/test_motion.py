import cadquery as cq

from cq_worker.execution import Execution, RunSettings
from cq_worker.motion import trace_path


def test_path_detects_wall_between_two_clear_end_positions():
    wall = cq.Workplane().box(2, 10, 10)
    ball = cq.Workplane().sphere(0.5)
    report = trace_path(ball, [wall], [(-3, 0, 0, 0, 0, 0), (3, 0, 0, 0, 0, 0)])
    assert report["status"] == "blocked"
    assert report["overlap_volume"] > 0


def test_pin_can_follow_a_path_through_a_larger_hole():
    plate = cq.Workplane().box(10, 10, 2).faces(">Z").workplane().hole(2)
    pin = cq.Workplane().circle(0.5).extrude(2)
    report = trace_path(pin, [plate], [(0, 0, -4, 0, 0, 0), (0, 0, 4, 0, 0, 0)])
    assert report["status"] == "clear", report


def test_rotating_arm_hits_obstacle_between_clear_end_angles():
    arm = cq.Workplane().box(2, 0.2, 0.2)
    obstacle = cq.Workplane().sphere(0.2).translate((0.7, 0.7, 0))
    report = trace_path(arm, [obstacle], [(0, 0, 0, 0, 0, 0), (0, 0, 0, 0, 0, 90)])
    assert report["status"] == "blocked"


def test_depth_limit_cannot_turn_uncertainty_into_clearance():
    wall = cq.Workplane().box(2, 10, 2)
    ball = cq.Workplane().sphere(1)
    report = trace_path(
        ball,
        [wall],
        [(-3, 0, 2, 0, 0, 0), (3, 0, 2, 0, 0, 0)],
        max_depth=0,
    )
    assert report["status"] == "inconclusive"


def test_path_requirement_stops_a_script_that_would_cross_a_wall():
    execution = Execution(
        "wall=cq.Workplane().box(2,10,10)\n"
        "ball=cq.Workplane().sphere(.5)\n"
        "require_clear_path(ball,[wall],[(-3,0,0,0,0,0),(3,0,0,0,0,0)])\n",
        None,
        RunSettings(),
    )
    assert not execution.run()
    assert execution.error["type_name"] == "PathError"
