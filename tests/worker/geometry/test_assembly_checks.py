import cadquery as cq
import pytest

from cq_worker.assembly_checks import (
    AssemblyError,
    assembly_diagnostics,
    require_constrained_assembly,
)


def test_unconstrained_assembly_reports_free_dof_and_overlap():
    box = cq.Workplane().box(2, 2, 2)
    assembly = cq.Assembly().add(box, name="first").add(box, name="second")
    report = assembly_diagnostics(assembly)
    assert report["minimum_free_dof"] == 6
    assert report["unconstrained_components"] == ["first", "second"]
    assert "underconstrained" in report["issues"]
    assert "interference" in report["issues"]
    with pytest.raises(AssemblyError):
        require_constrained_assembly(assembly)


def test_solved_fixed_assembly_reports_no_free_dof_or_interference():
    box = cq.Workplane().box(1, 1, 1)
    assembly = (
        cq.Assembly()
        .add(box, name="first")
        .add(box, name="second", loc=cq.Location((3, 0, 0)))
        .constrain("first", "Fixed")
        .constrain("second", "Fixed")
        .solve()
    )
    report = require_constrained_assembly(assembly)
    assert report["solver_success"] is True
    assert report["minimum_free_dof"] == 0
    assert report["interferences"] == []


def test_plane_mate_alone_leaves_one_rotational_degree_of_freedom():
    box = cq.Workplane().box(1, 1, 1)
    assembly = (
        cq.Assembly()
        .add(box, name="base")
        .add(box, name="top", loc=cq.Location((0, 0, 3)))
        .constrain("base", "Fixed")
        .constrain("base@faces@>Z", "top@faces@<Z", "Plane")
        .solve()
    )
    report = assembly_diagnostics(assembly)
    assert report["minimum_free_dof"] == 1
    assert "underconstrained" in report["issues"]


def test_unsolved_constraints_are_not_reported_as_verified():
    box = cq.Workplane().box(1, 1, 1)
    assembly = (
        cq.Assembly()
        .add(box, name="first")
        .add(box, name="second", loc=cq.Location((3, 0, 0)))
        .constrain("first", "Fixed")
        .constrain("second", "Fixed")
    )
    report = assembly_diagnostics(assembly)
    assert "constraints_not_solved" in report["issues"]
