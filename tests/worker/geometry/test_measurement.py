import math

import cadquery as cq
import pytest

from cq_worker.measurement import MeasurementError, measure
from cq_worker.scene import Registry, RegistryEntry, SceneBuilder, ShownObject
from cq_worker.tessellation import Tolerance, TopologyIndex


def add_shape(registry, shape):
    topology = TopologyIndex.build(shape.wrapped)
    return registry.add(RegistryEntry(shape, shape, topologies=[topology]))


def reference(key, kind, identifier=1):
    return {"key": key, "part": 0, "kind": kind, "identifier": identifier}


def test_two_vertices_have_an_exact_five_unit_distance():
    registry = Registry()
    first = add_shape(registry, cq.Vertex.makeVertex(0, 0, 0))
    second = add_shape(registry, cq.Vertex.makeVertex(3, 4, 0))

    result = measure(
        registry, [reference(first, "vertex"), reference(second, "vertex")]
    )

    assert result["pair"]["distance"] == pytest.approx(5.0, abs=1e-12)
    assert result["pair"]["first_point"] == pytest.approx([0.0, 0.0, 0.0])
    assert result["pair"]["second_point"] == pytest.approx([3.0, 4.0, 0.0])
    assert result["identity"] == {"scope": "scene_entry", "persistent": False}


def test_material_density_produces_mass_for_mixed_assembly():
    registry = Registry()
    box = cq.Workplane().box(10, 10, 10).val()
    steel = registry.add(
        RegistryEntry(box, box, material=cq.Material("Steel", density=7.8))
    )
    aluminium = registry.add(
        RegistryEntry(box, box, material=cq.Material("Al", density=2.7))
    )
    root = registry.add(RegistryEntry(None, None, children=[steel, aluminium]))

    (result,) = measure(registry, [], [root])["objects"]

    assert result["mass_g"] == pytest.approx(10.5)
    assert result["materials"] == ["Al", "Steel"]


def test_merged_assembly_keeps_each_part_material_for_mass():
    assembly = cq.Assembly(name="mixed")
    box = cq.Workplane().box(10, 10, 10)
    assembly.add(box, name="steel", material=cq.Material("Steel", density=7.8))
    assembly.add(box, name="al", material=cq.Material("Al", density=2.7))
    registry = Registry()
    (node,) = SceneBuilder(registry, Tolerance(), merge_assemblies=True).build(
        {"mixed": ShownObject(assembly)}
    )

    (result,) = measure(registry, [], [node["key"]])["objects"]

    assert result["mass_g"] == pytest.approx(10.5)
    assert result["materials"] == ["Al", "Steel"]


def test_curved_edge_reports_occt_length_radius_and_diameter():
    registry = Registry()
    key = add_shape(registry, cq.Edge.makeCircle(2.5))

    (entity,) = measure(registry, [reference(key, "edge")])["entities"]

    assert entity["geometry_type"] == "circle"
    assert entity["length"] == pytest.approx(5.0 * math.pi)
    assert entity["radius"] == pytest.approx(2.5)
    assert entity["diameter"] == pytest.approx(5.0)


def test_circular_planar_face_reports_area_and_boundary_radius():
    wire = cq.Wire.makeCircle(3, cq.Vector(), cq.Vector(0, 0, 1))
    face = cq.Face.makeFromWires(wire)
    registry = Registry()
    key = add_shape(registry, face)

    (entity,) = measure(registry, [reference(key, "face")])["entities"]

    assert entity["geometry_type"] == "plane"
    assert entity["area"] == pytest.approx(9.0 * math.pi)
    assert entity["radius"] == pytest.approx(3.0)
    assert entity["diameter"] == pytest.approx(6.0)


def test_cylindrical_face_reports_surface_type_and_radius():
    face = cq.Workplane("XY").cylinder(5, 2).faces("%CYLINDER").val()
    registry = Registry()
    key = add_shape(registry, face)

    (entity,) = measure(registry, [reference(key, "face")])["entities"]

    assert entity["geometry_type"] == "cylinder"
    assert entity["radius"] == pytest.approx(2.0)
    assert entity["diameter"] == pytest.approx(4.0)


def test_angles_are_measured_for_straight_edges_and_planar_faces():
    registry = Registry()
    horizontal = add_shape(registry, cq.Edge.makeLine(cq.Vector(), cq.Vector(4, 0, 0)))
    vertical = add_shape(registry, cq.Edge.makeLine(cq.Vector(), cq.Vector(0, 4, 0)))
    edge_angle = measure(
        registry,
        [reference(horizontal, "edge"), reference(vertical, "edge")],
    )["pair"]

    assert edge_angle["angle_kind"] == "edge_edge"
    assert edge_angle["angle_degrees"] == pytest.approx(90.0)


def test_planar_face_angle_and_edge_to_plane_angle():
    registry = Registry()
    xy_face = cq.Workplane("XY").rect(2, 2).extrude(0.1).faces(">Z").val()
    yz_face = cq.Workplane("YZ").rect(2, 2).extrude(0.1).faces(">X").val()
    edge = cq.Edge.makeLine(cq.Vector(), cq.Vector(0, 0, 4))
    xy_key = add_shape(registry, xy_face)
    yz_key = add_shape(registry, yz_face)
    edge_key = add_shape(registry, edge)

    faces = measure(registry, [reference(xy_key, "face"), reference(yz_key, "face")])[
        "pair"
    ]
    edge_face = measure(
        registry, [reference(edge_key, "edge"), reference(xy_key, "face")]
    )["pair"]

    assert faces["angle_kind"] == "face_face"
    assert faces["angle_degrees"] == pytest.approx(90.0)
    assert edge_face["angle_kind"] == "edge_face"
    assert edge_face["angle_degrees"] == pytest.approx(90.0)


def test_rejects_empty_or_oversized_selections_and_stale_references():
    registry = Registry()
    key = add_shape(registry, cq.Vertex.makeVertex(0, 0, 0))
    ref = reference(key, "vertex")

    with pytest.raises(MeasurementError, match="between one and 64"):
        measure(registry, [])
    with pytest.raises(MeasurementError, match="between one and 64"):
        measure(registry, [ref] * 65)
    registry.release([key])
    with pytest.raises(MeasurementError, match="previous Python session"):
        measure(registry, [ref])


def test_more_than_two_entities_are_described_without_a_pair():
    registry = Registry()
    box = cq.Workplane().box(1, 2, 3).val()
    key = add_shape(registry, box)

    result = measure(registry, [reference(key, "face", i) for i in (1, 2, 3)])

    assert [entity["identifier"] for entity in result["entities"]] == [1, 2, 3]
    assert all(entity["area"] > 0 for entity in result["entities"])
    assert result["pair"] is None


def _shown(registry, obj):
    """Shows ``obj`` as the GUI does, which meshes it."""

    from cq_worker.scene import SceneBuilder, ShownObject
    from cq_worker.tessellation import Tolerance

    (node,) = SceneBuilder(registry, Tolerance()).build({"obj": ShownObject(obj)})
    return node["key"]


def test_bounding_boxes_are_exact_even_on_meshed_shapes():
    registry = Registry()
    sphere = cq.Workplane().sphere(5)
    key = _shown(registry, sphere)
    # CadQuery's own BoundingBox() now includes the display mesh.
    assert sphere.val().BoundingBox().xmax > 5.01

    (face,) = measure(registry, [reference(key, "face")])["entities"]
    box = face["bounding_box"]
    assert box["min"] == pytest.approx([-5, -5, -5], abs=1e-6)
    assert box["max"] == pytest.approx([5, 5, 5], abs=1e-6)
    assert box["size"] == pytest.approx([10, 10, 10], abs=1e-6)
    assert box["center"] == pytest.approx([0, 0, 0], abs=1e-6)


def test_the_selection_has_one_bounding_box_around_all_entities():
    registry = Registry()
    key = _shown(registry, cq.Workplane().box(2, 4, 6))
    result = measure(registry, [reference(key, "face", 1), reference(key, "face", 2)])
    # The two faces normal to X span the whole box.
    assert result["bounding_box"]["size"] == pytest.approx([2, 4, 6])
    assert result["objects"] == []


def test_selected_objects_are_measured_when_no_entity_is_picked():
    registry = Registry()
    key = _shown(registry, cq.Workplane().box(1, 2, 3))
    wire = _shown(registry, cq.Workplane().rect(2, 2))

    result = measure(registry, [], [key, wire])

    assert result["entities"] == [] and result["pair"] is None
    box, square = result["objects"]
    assert box["key"] == key and box["shape_type"] == "solid"
    assert box["volume"] == pytest.approx(6)
    assert box["area"] == pytest.approx(22)
    assert box["center"] == pytest.approx([0, 0, 0])
    assert (box["solids"], box["faces"], box["edges"], box["vertices"]) == (1, 6, 12, 8)
    assert box["bounding_box"]["size"] == pytest.approx([1, 2, 3])
    assert square["volume"] is None and square["area"] is None
    assert square["length"] == pytest.approx(8)
    assert result["bounding_box"]["size"] == pytest.approx([2, 2, 3])


def test_picked_entities_take_precedence_over_selected_objects():
    registry = Registry()
    key = _shown(registry, cq.Workplane().box(1, 2, 3))
    result = measure(registry, [reference(key, "face", 1)], [key])
    assert len(result["entities"]) == 1 and result["objects"] == []


@pytest.mark.parametrize("objects", [[""], "n1", [1], ["n1"] * 65])
def test_invalid_object_lists_are_rejected(objects):
    with pytest.raises(MeasurementError, match="scene keys"):
        measure(Registry(), [], objects)
