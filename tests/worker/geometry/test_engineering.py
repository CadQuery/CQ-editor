import cadquery as cq
import pytest

from cq_worker.engineering import (
    BeamSection,
    EngineeringError,
    EngineeringMaterial,
    beam_check,
    require_beam,
    tool_access,
)
from cq_worker.execution import Execution, RunSettings

STEEL = EngineeringMaterial("Steel", 7.85, 210_000, 250, expansion_per_c=12e-6)


def test_hollow_tube_section_and_self_weight_have_explicit_units():
    section = BeamSection.hollow_rectangle(20, 20, 1.5)
    assert section.area_mm2 == pytest.approx(111)
    assert section.inertia_mm4 == pytest.approx(6373.25)
    assert section.minimum_wall_mm == 1.5
    report = beam_check(section=section, material=STEEL, length_mm=700)
    assert report["mass_kg"] == pytest.approx(0.609945)
    assert report["maximum_stress_mpa"] > 0


def test_load_deflection_wall_and_yield_requirements_are_checked():
    section = BeamSection.hollow_rectangle(20, 20, 1.5)
    safe = beam_check(
        section=section,
        material=STEEL,
        length_mm=700,
        transverse_load_n=100,
        maximum_deflection_mm=10,
        required_wall_mm=1.2,
        required_safety_factor=2,
    )
    assert safe["ok"]
    assert safe["deflection_mm"] > 8
    failed = beam_check(
        section=section,
        material=STEEL,
        length_mm=700,
        transverse_load_n=100,
        maximum_deflection_mm=5,
        required_wall_mm=2,
    )
    assert not failed["ok"]
    assert len(failed["issues"]) == 2
    with pytest.raises(EngineeringError):
        require_beam(
            section=section,
            material=STEEL,
            length_mm=700,
            transverse_load_n=1000,
        )


def test_compression_and_restrained_temperature_add_conservative_stress():
    section = BeamSection.round_tube(20, 2)
    report = beam_check(
        section=section,
        material=STEEL,
        length_mm=1500,
        axial_compression_n=3000,
        temperature_change_c=60,
        thermally_restrained=True,
    )
    assert report["thermal_expansion_mm"] == pytest.approx(1.08)
    assert report["maximum_stress_mpa"] > 150


def test_tool_access_finds_an_obstruction_on_the_approach():
    wall = cq.Workplane().box(2, 10, 10)
    report = tool_access(1, 2, [wall], [(-4, 0, 0, 0, 90, 0), (4, 0, 0, 0, 90, 0)])
    assert report["status"] == "blocked"


def test_beam_requirement_fails_the_script_on_unacceptable_deflection():
    execution = Execution(
        "steel=EngineeringMaterial('steel',7.85,210000,250)\n"
        "tube=BeamSection.hollow_rectangle(20,20,1.5)\n"
        "require_beam(section=tube,material=steel,length_mm=700,"
        "transverse_load_n=100,maximum_deflection_mm=1)\n",
        None,
        RunSettings(),
    )
    assert not execution.run()
    assert execution.error["type_name"] == "EngineeringError"
