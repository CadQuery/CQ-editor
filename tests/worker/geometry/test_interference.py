"""Interference checks use the placed solids, never their display meshes."""

import cadquery as cq
import pytest
from cq_worker.interference import (
    analyze_interferences,
    apply_allowances,
    overlap_shape,
    summarize,
)
from cq_worker.scene import Registry, SceneBuilder, ShownObject
from cq_worker.tessellation import Tolerance


def assembly_report(hole_diameter, pin_diameter, *, merged=False, pin_x=0):
    plate = cq.Workplane().box(20, 20, 5).faces(">Z").workplane().hole(hole_diameter)
    pin = cq.Workplane().circle(pin_diameter / 2).extrude(10).translate((0, 0, -5))
    assembly = cq.Assembly(name="joint")
    assembly.add(plate, name="plate")
    assembly.add(pin, name="pin", loc=cq.Location((pin_x, 0, 0)))
    registry = Registry()
    (node,) = SceneBuilder(registry, Tolerance(), merge_assemblies=merged).build(
        {"joint": ShownObject(assembly)}
    )
    return analyze_interferences(registry, [node["key"]])


@pytest.mark.parametrize("merged", [False, True])
def test_oversize_pin_interferes_with_hole_in_both_display_modes(merged):
    (report,) = assembly_report(5, 6, merged=merged)
    assert report["kind"] == "interference"
    assert report["overlap_volume"] == pytest.approx(43.1968989869)
    assert {
        report["first"]["name"].split("/")[-1],
        report["second"]["name"].split("/")[-1],
    } == {
        "plate",
        "pin",
    }


def test_smaller_pin_passes_through_exact_hole_without_interference():
    assert assembly_report(6.5, 6) == []


def test_misaligned_pin_interferes_even_when_its_diameter_is_smaller():
    reports = assembly_report(6.5, 6, pin_x=0.5)
    assert len(reports) == 1 and reports[0]["kind"] == "interference"


def test_overlap_shape_matches_report_and_rejects_stale_pair():
    first = cq.Workplane().box(2, 2, 2)
    second = cq.Workplane().box(2, 2, 2).translate((1, 0, 0))
    registry = Registry()
    nodes = SceneBuilder(registry, Tolerance()).build(
        {"first": ShownObject(first), "second": ShownObject(second)}
    )
    (report,) = analyze_interferences(registry, [node["key"] for node in nodes])
    common = overlap_shape(registry, report["first"], report["second"])
    assert common.Volume() == pytest.approx(report["overlap_volume"])
    stale = {**report["first"], "solid": 99}
    with pytest.raises(ValueError, match="no longer in the live scene"):
        overlap_shape(registry, stale, report["second"])


def test_face_contact_is_separate_from_penetration():
    first = cq.Workplane().box(2, 2, 2)
    second = cq.Workplane().box(2, 2, 2).translate((2, 0, 0))
    registry = Registry()
    nodes = SceneBuilder(registry, Tolerance()).build(
        {"first": ShownObject(first), "second": ShownObject(second)}
    )
    (report,) = analyze_interferences(registry, [node["key"] for node in nodes])
    assert report["kind"] == "contact"
    assert report["overlap_volume"] == pytest.approx(0)


def test_two_instances_of_same_shape_are_not_deduplicated_inside_assembly():
    cube = cq.Workplane().box(2, 2, 2)
    assembly = cq.Assembly().add(cube, name="one").add(cube, name="two")
    registry = Registry()
    (node,) = SceneBuilder(registry, Tolerance()).build(
        {"assembly": ShownObject(assembly)}
    )
    (report,) = analyze_interferences(registry, [node["key"]])
    assert report["kind"] == "interference"
    assert report["overlap_volume"] == pytest.approx(8)


def test_equal_diameters_are_contact_not_assured_manufacturing_clearance():
    reports = assembly_report(6, 6)
    assert len(reports) == 1
    assert reports[0]["kind"] == "contact"
    assert summarize(reports)["status"] == "clear"


def test_failed_exact_operation_is_inconclusive_not_safe(monkeypatch):
    def fail(*_args):
        raise RuntimeError("boolean operation failed")

    monkeypatch.setattr(cq.Shape, "intersect", fail)
    (report,) = assembly_report(5, 6)
    assert report["kind"] == "inconclusive"
    assert "boolean operation failed" in report["reason"]
    assert summarize([report])["status"] == "inconclusive"


def test_summary_prioritizes_interference_over_uncertainty():
    reports = [{"kind": "inconclusive"}, {"kind": "interference"}]
    assert summarize(reports) == {
        "status": "interference",
        "counts": {
            "interference": 1,
            "inconclusive": 1,
            "contact": 0,
            "clearance": 0,
        },
        "accepted": 0,
    }


def test_explicit_allowance_accepts_only_one_unambiguous_pair():
    registry = Registry()
    nodes = SceneBuilder(registry, Tolerance()).build(
        {
            "plate": ShownObject(cq.Workplane().box(4, 4, 4)),
            "pin": ShownObject(cq.Workplane().box(2, 2, 2)),
            "other": ShownObject(cq.Workplane().box(1, 1, 1)),
        }
    )
    keys = [node["key"] for node in nodes]
    reports = analyze_interferences(registry, keys)
    assert len(reports) == 3
    apply_allowances(registry, keys, reports, [("plate", "pin", "press fit")])
    assert summarize(reports)["status"] == "interference"
    assert summarize(reports)["accepted"] == 1
    assert sum(bool(report.get("allowance_reason")) for report in reports) == 1


def test_allowance_does_not_accept_inconclusive_boolean(monkeypatch):
    registry = Registry()
    nodes = SceneBuilder(registry, Tolerance()).build(
        {
            "plate": ShownObject(cq.Workplane().box(4, 4, 4)),
            "pin": ShownObject(cq.Workplane().box(2, 2, 2)),
        }
    )
    keys = [node["key"] for node in nodes]

    def fail(*_args):
        raise RuntimeError("boolean failed")

    monkeypatch.setattr(cq.Shape, "intersect", fail)
    reports = analyze_interferences(registry, keys)
    apply_allowances(registry, keys, reports, [("plate", "pin", "press fit")])
    assert summarize(reports)["status"] == "inconclusive"
    assert reports[0].get("allowance_reason") is None
