"""The geometry report is the same precondition used by exports and CI."""

import cadquery as cq
import pytest
from cq_worker import checking, exporting
from cq_worker.scene import Registry, RegistryEntry


def registry_of(name, shape):
    registry = Registry()
    key = registry.add(RegistryEntry(shape, shape, name=name))
    return registry, key


def test_closed_solid_has_no_geometry_issues():
    registry, key = registry_of("box", cq.Workplane().box(1, 1, 1).val())
    report = checking.check_registry(registry, [key])[0]
    assert report["valid"]
    assert report["solids"] == 1
    assert report["open_shells"] == report["non_manifold_edges"] == 0


def test_open_shell_blocks_export_until_the_user_decides(tmp_path):
    box = cq.Workplane().box(1, 1, 1).val()
    shell = cq.Shell.makeShell(box.Faces()[:-1])
    registry, key = registry_of("open case", shell)
    path = tmp_path / "part.step"
    report = checking.check_registry(registry, [key])[0]
    assert not report["valid"]
    assert report["open_shells"] == 1

    with pytest.raises(checking.GeometryInvalid) as error:
        exporting.export(registry, [key], "step", str(path))
    assert error.value.reports[0]["name"] == "open case"
    assert not path.exists(), "the check runs before the writer opens the file"

    with pytest.raises(checking.GeometryInvalid):
        exporting.export(registry, [key], "step", str(path), on_invalid="fix")
    assert not path.exists(), "a repair that did not close the shell is refused"

    exporting.export(registry, [key], "step", str(path), on_invalid="ignore")
    assert path.stat().st_size > 100


def test_interfering_parts_pause_export_before_the_writer_opens_file(tmp_path):
    plate = cq.Workplane().box(20, 20, 5).faces(">Z").workplane().hole(5).val()
    pin = cq.Workplane().circle(3).extrude(10).translate((0, 0, -5)).val()
    registry = Registry()
    plate_key = registry.add(RegistryEntry(plate, plate, name="plate"))
    pin_key = registry.add(RegistryEntry(pin, pin, name="pin"))
    path = tmp_path / "joint.step"
    with pytest.raises(checking.GeometryInvalid) as error:
        exporting.export(registry, [plate_key, pin_key], "step", str(path))
    assert error.value.reports == []
    assert error.value.interferences[0]["kind"] == "interference"
    assert not path.exists()

    exporting.export(
        registry,
        [plate_key, pin_key],
        "step",
        str(path),
        on_interference="ignore",
    )
    assert path.stat().st_size > 100
