"""Conservative diagnostics for CadQuery assembly constraints and fit."""

from __future__ import annotations

import math
from typing import Any

import cadquery as cq

from .checking import check_registry, invalid_reports
from .interference import analyze_interferences
from .scene import Registry, SceneBuilder, ShownObject
from .tessellation import Tolerance

# Upper bounds on independent scalar equations each constraint can impose.
# Their sum gives a *lower* bound on free degrees of freedom: redundancy can
# only make the real assembly less constrained than the reported bound.
_MAX_RANK = {
    "Fixed": 6,
    "FixedPoint": 3,
    "FixedAxis": 2,
    "FixedRotation": 3,
    "FixedRotationAxis": 1,
    "Point": 3,
    "Axis": 2,
    "Plane": 5,
    "PointInPlane": 1,
    "PointOnLine": 2,
}
_UNARY = {"Fixed", "FixedPoint", "FixedAxis", "FixedRotation", "FixedRotationAxis"}


class AssemblyError(ValueError):
    """A strict constraint and interference requirement is not satisfied."""


def assembly_diagnostics(assembly: cq.Assembly) -> dict[str, Any]:
    """Describe constraints without moving a part or silently solving it.

    ``minimum_free_dof`` is conservative for a flat assembly.  A value above
    zero proves underconstraint; zero does not prove that every direction is
    constrained, because constraints may be redundant or dependent.
    """

    if not isinstance(assembly, cq.Assembly):
        raise TypeError("assembly_diagnostics expects a CadQuery Assembly")
    components = {
        name: node for name, node in assembly.objects.items() if node.obj is not None
    }
    constraints: list[dict[str, Any]] = [
        {"kind": str(spec.kind), "objects": list(spec.objects)}
        for spec in assembly.constraints
    ]
    referenced = {name for spec in assembly.constraints for name in spec.objects}
    unconstrained = sorted(name for name in components if name not in referenced)
    nested = any(node.children for node in assembly.children)
    unknown_kinds = sorted({row["kind"] for row in constraints} - _MAX_RANK.keys())
    minimum_free_dof: int | None = None
    if not nested and not unknown_kinds:
        total = 6 * len(components)
        rank_ceiling = sum(_MAX_RANK[row["kind"]] for row in constraints)
        # Without a unary ground constraint, one rigid global placement is a
        # gauge freedom and does not make the relative assembly underconstrained.
        global_gauge = 0 if any(row["kind"] in _UNARY for row in constraints) else 6
        minimum_free_dof = max(0, total - rank_ceiling - global_gauge)

    solved = getattr(assembly, "_solve_result", None)
    solver_success: bool | None = None
    residual: float | None = None
    if isinstance(solved, dict):
        solver_success = bool(solved.get("success", False))
        values = solved.get("iterations", {}).get("obj", [])
        if values:
            candidate = float(values[-1])
            if math.isfinite(candidate):
                residual = candidate

    registry = Registry()
    nodes = SceneBuilder(registry, Tolerance()).build(
        {"assembly": ShownObject(assembly)}
    )
    if nodes:
        keys = [node["key"] for node in nodes]
        topology_issues = invalid_reports(check_registry(registry, keys))
        pairs = analyze_interferences(registry, keys)
    else:
        topology_issues = [
            {"name": assembly.name, "issues": ["Could not inspect assembly"]}
        ]
        pairs = []

    issues = []
    if topology_issues:
        issues.append("invalid_geometry")
    if any(pair["kind"] == "interference" for pair in pairs):
        issues.append("interference")
    if any(pair["kind"] == "inconclusive" for pair in pairs):
        issues.append("inconclusive_geometry")
    if constraints and solver_success is not True:
        issues.append("constraints_not_solved")
    if residual is not None and residual > 1e-8:
        issues.append("constraint_residual")
    if minimum_free_dof is not None and minimum_free_dof > 0:
        issues.append("underconstrained")
    if nested or unknown_kinds:
        issues.append("dof_inconclusive")

    return {
        "ok": not issues,
        "components": sorted(components),
        "constraints": constraints,
        "unconstrained_components": unconstrained,
        "minimum_free_dof": minimum_free_dof,
        "solver_success": solver_success,
        "solver_residual": residual,
        "topology_issues": topology_issues,
        "interferences": pairs,
        "issues": issues,
    }


def require_constrained_assembly(assembly: cq.Assembly) -> dict[str, Any]:
    """Fail a script when a constrained assembly cannot be certified."""

    report = assembly_diagnostics(assembly)
    if not report["ok"]:
        raise AssemblyError(f"Assembly needs review: {', '.join(report['issues'])}")
    return report
