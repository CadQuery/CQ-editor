"""Explicit engineering checks for prismatic beams and tool access.

Units are millimetres, newtons and megapascals.  These are transparent 1-D
linear-elastic calculations, not a finite-element claim about arbitrary CAD.
"""

from __future__ import annotations

import math
from dataclasses import dataclass
from typing import Any

import cadquery as cq

from .motion import trace_path

GRAVITY_M_S2 = 9.80665


def _positive(name: str, value: float, *, zero: bool = False) -> float:
    number = float(value)
    if not math.isfinite(number) or (number < 0 if zero else number <= 0):
        raise ValueError(
            f"{name} must be finite and {'non-negative' if zero else 'positive'}"
        )
    return number


@dataclass(frozen=True)
class EngineeringMaterial:
    name: str
    density_g_cm3: float
    youngs_modulus_mpa: float
    yield_strength_mpa: float
    poisson_ratio: float = 0.3
    expansion_per_c: float = 0.0

    def __post_init__(self) -> None:
        if not self.name.strip():
            raise ValueError("Material needs a name")
        for name in ("density_g_cm3", "youngs_modulus_mpa", "yield_strength_mpa"):
            _positive(name, getattr(self, name))
        ratio = _positive("poisson_ratio", self.poisson_ratio, zero=True)
        if ratio >= 0.5:
            raise ValueError("poisson_ratio must be below 0.5")
        _positive("expansion_per_c", self.expansion_per_c, zero=True)


@dataclass(frozen=True)
class BeamSection:
    area_mm2: float
    inertia_mm4: float
    outer_radius_mm: float
    minimum_wall_mm: float | None = None

    def __post_init__(self) -> None:
        for name in ("area_mm2", "inertia_mm4", "outer_radius_mm"):
            _positive(name, getattr(self, name))
        if self.minimum_wall_mm is not None:
            _positive("minimum_wall_mm", self.minimum_wall_mm)

    @classmethod
    def rectangle(cls, width_mm: float, height_mm: float) -> BeamSection:
        width = _positive("width_mm", width_mm)
        height = _positive("height_mm", height_mm)
        return cls(width * height, width * height**3 / 12, height / 2)

    @classmethod
    def hollow_rectangle(
        cls, width_mm: float, height_mm: float, wall_mm: float
    ) -> BeamSection:
        width = _positive("width_mm", width_mm)
        height = _positive("height_mm", height_mm)
        wall = _positive("wall_mm", wall_mm)
        inside_width, inside_height = width - 2 * wall, height - 2 * wall
        if inside_width <= 0 or inside_height <= 0:
            raise ValueError("Wall must leave a positive hollow opening")
        return cls(
            width * height - inside_width * inside_height,
            (width * height**3 - inside_width * inside_height**3) / 12,
            height / 2,
            wall,
        )

    @classmethod
    def round_tube(cls, outside_mm: float, wall_mm: float) -> BeamSection:
        outside = _positive("outside_mm", outside_mm)
        wall = _positive("wall_mm", wall_mm)
        inside = outside - 2 * wall
        if inside <= 0:
            raise ValueError("Wall must leave a positive bore")
        return cls(
            math.pi * (outside**2 - inside**2) / 4,
            math.pi * (outside**4 - inside**4) / 64,
            outside / 2,
            wall,
        )


class EngineeringError(ValueError):
    """A declared structural or tool-access requirement is not satisfied."""


def beam_check(
    *,
    section: BeamSection,
    material: EngineeringMaterial,
    length_mm: float,
    transverse_load_n: float = 0.0,
    axial_compression_n: float = 0.0,
    support: str = "cantilever",
    required_safety_factor: float = 1.5,
    maximum_deflection_mm: float | None = None,
    required_wall_mm: float | None = None,
    temperature_change_c: float = 0.0,
    thermally_restrained: bool = False,
) -> dict[str, Any]:
    """Check a prismatic Euler-Bernoulli beam under stated simple loads.

    A cantilever has an end point load; a simply supported beam has a center
    point load.  Both include self-weight as a uniform load.  The reported
    stress conservatively sums bending, compression and restrained thermal
    stress.  Local buckling, joint flexibility and 3-D stress are not modeled.
    """

    if not isinstance(section, BeamSection) or not isinstance(
        material, EngineeringMaterial
    ):
        raise TypeError("Provide a BeamSection and EngineeringMaterial")
    length = _positive("length_mm", length_mm)
    transverse = _positive("transverse_load_n", transverse_load_n, zero=True)
    axial = _positive("axial_compression_n", axial_compression_n, zero=True)
    safety = _positive("required_safety_factor", required_safety_factor)
    if safety < 1:
        raise ValueError("required_safety_factor must be at least 1")
    if support not in ("cantilever", "simply_supported_center_load"):
        raise ValueError("Unknown support case")
    limit = (
        _positive("maximum_deflection_mm", maximum_deflection_mm, zero=True)
        if maximum_deflection_mm is not None
        else None
    )
    wall_requirement = (
        _positive("required_wall_mm", required_wall_mm)
        if required_wall_mm is not None
        else None
    )
    temperature = float(temperature_change_c)
    if not math.isfinite(temperature):
        raise ValueError("temperature_change_c must be finite")

    e = material.youngs_modulus_mpa
    inertia = section.inertia_mm4
    # density (g/cm^3) * area (mm^2) / 1e6 -> kg per millimetre.
    self_weight_n_per_mm = (
        material.density_g_cm3 * section.area_mm2 * GRAVITY_M_S2 / 1e6
    )
    if support == "cantilever":
        moment = transverse * length + self_weight_n_per_mm * length**2 / 2
        deflection = transverse * length**3 / (
            3 * e * inertia
        ) + self_weight_n_per_mm * length**4 / (8 * e * inertia)
        effective_length = 2 * length
    else:
        moment = transverse * length / 4 + self_weight_n_per_mm * length**2 / 8
        deflection = transverse * length**3 / (
            48 * e * inertia
        ) + 5 * self_weight_n_per_mm * length**4 / (384 * e * inertia)
        effective_length = length

    thermal_expansion = material.expansion_per_c * temperature * length
    thermal_stress = (
        e * material.expansion_per_c * abs(temperature) if thermally_restrained else 0.0
    )
    stress = (
        moment * section.outer_radius_mm / inertia
        + axial / section.area_mm2
        + thermal_stress
    )
    buckling = math.pi**2 * e * inertia / effective_length**2
    mass_kg = material.density_g_cm3 * section.area_mm2 * length / 1e6
    if not all(
        math.isfinite(value) for value in (stress, deflection, buckling, mass_kg)
    ):
        raise ValueError("Beam result is outside numeric range")

    issues = []
    if stress * safety > material.yield_strength_mpa:
        issues.append("Yield strength is below the required safety factor")
    if axial > 0 and axial * safety > buckling:
        issues.append("Euler column buckling capacity is insufficient")
    if limit is not None and deflection > limit:
        issues.append("Deflection exceeds the stated limit")
    if wall_requirement is not None:
        if section.minimum_wall_mm is None:
            issues.append("Minimum wall is unknown for this section")
        elif section.minimum_wall_mm < wall_requirement:
            issues.append("Section wall is below the stated minimum")

    return {
        "ok": not issues,
        "material": material.name,
        "support": support,
        "mass_kg": mass_kg,
        "maximum_stress_mpa": stress,
        "yield_allowable_mpa": material.yield_strength_mpa / safety,
        "deflection_mm": deflection,
        "buckling_capacity_n": buckling,
        "thermal_expansion_mm": thermal_expansion,
        "minimum_wall_mm": section.minimum_wall_mm,
        "issues": issues,
        "model": "1-D linear-elastic Euler-Bernoulli beam; ideal supports and section",
    }


def require_beam(**conditions: Any) -> dict[str, Any]:
    report = beam_check(**conditions)
    if not report["ok"]:
        raise EngineeringError("; ".join(report["issues"]))
    return report


def tool_access(
    tool_diameter_mm: float,
    tool_length_mm: float,
    obstacles: list[cq.Shape | cq.Workplane | cq.Assembly],
    poses: list[tuple[float, float, float, float, float, float]],
    *,
    required_clearance: float = 0.0,
) -> dict[str, Any]:
    """Check a cylindrical tool envelope along a declared approach path."""

    diameter = _positive("tool_diameter_mm", tool_diameter_mm)
    length = _positive("tool_length_mm", tool_length_mm)
    tool = cq.Workplane().circle(diameter / 2).extrude(length)
    return trace_path(tool, obstacles, poses, required_clearance=required_clearance)


def require_tool_access(*args: Any, **kwargs: Any) -> dict[str, Any]:
    report = tool_access(*args, **kwargs)
    if report["status"] != "clear":
        raise EngineeringError(f"Tool access {report['status']}: {report}")
    return report
