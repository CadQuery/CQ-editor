import os
import time

import cadquery as cq
import numpy as np
import OCP
import pytest

from conftest import decode
from cq_worker import tessellation
from cq_worker.tessellation import GeometryCache, Tolerance, TopologyIndex, tessellate


def test_box_mesh():
    mesh = tessellate(cq.Workplane().box(2, 4, 6).val().wrapped, Tolerance())

    assert len(mesh.indices) == 12 * 3  # two triangles per face
    assert mesh.indices.max() < len(mesh.positions)
    assert len(mesh.edges) == 12  # straight edges need one segment each
    assert len(mesh.points) == 0
    assert sorted(set(mesh.triangle_faces)) == [1, 2, 3, 4, 5, 6]
    assert sorted(set(mesh.segment_edges)) == list(range(1, 13))
    assert len(mesh.topology_vertices) == 8
    # The display triangles are the pick triangles: no second copy.
    assert len(mesh.triangle_faces) == len(mesh.indices) // 3
    assert len(mesh.face_positions) == len(mesh.face_indices) == 0
    assert len(mesh.segment_edges) == len(mesh.edges)
    lo, hi = mesh.bounds()
    assert lo == pytest.approx([-1, -2, -3])
    assert hi == pytest.approx([1, 2, 3])


def test_normals_are_unit_and_point_outwards():
    mesh = tessellate(cq.Workplane().sphere(5).val().wrapped, Tolerance())

    lengths = np.linalg.norm(mesh.normals, axis=1)
    assert lengths == pytest.approx(np.ones_like(lengths), abs=1e-4)
    # On a sphere centered at the origin the outward normal is the position.
    radial = mesh.positions / np.linalg.norm(mesh.positions, axis=1, keepdims=True)
    assert np.einsum("ij,ij->i", radial, mesh.normals).min() > 0.99


def test_transformed_shapes_are_meshed_in_world_space():
    shape = cq.Workplane().box(10, 20, 30).val()
    moved = shape.moved(cq.Location(cq.Vector(100, 0, 0), cq.Vector(1, 0, 0), 90))
    mesh = tessellate(moved.wrapped, Tolerance())

    lo, hi = mesh.bounds()
    assert lo == pytest.approx([95, -15, -10], abs=1e-4)
    assert hi == pytest.approx([105, 15, 10], abs=1e-4)
    assert sorted(set(mesh.triangle_faces)) == [1, 2, 3, 4, 5, 6]
    assert mesh.topology_vertices.min(axis=0) == pytest.approx(lo, abs=1e-4)
    assert mesh.topology_vertices.max(axis=0) == pytest.approx(hi, abs=1e-4)


def test_wires_and_vertices():
    wire = tessellate(cq.Workplane().circle(5).val().wrapped, Tolerance())
    assert len(wire.indices) == 0 and len(wire.edges) > 8
    assert set(wire.segment_edges) == {1}

    vertex = tessellate(cq.Vertex.makeVertex(1, 2, 3).wrapped, Tolerance())
    assert vertex.points.tolist() == [[1, 2, 3]]
    assert vertex.topology_vertices.tolist() == [[1, 2, 3]]


def test_empty_shapes_have_no_mesh():
    assert tessellate(cq.Compound.makeCompound([]).wrapped, Tolerance()) is None


def test_message_round_trip():
    mesh = tessellate(cq.Workplane().box(1, 1, 1).val().wrapped, Tolerance())
    message = mesh.to_message()

    assert decode(message["positions"], "<f4", 3).shape == mesh.positions.shape
    assert decode(message["indices"], "<u4", 3).shape == (12, 3)
    assert decode(message["edges"], "<f4", 6).shape == (12, 6)
    topology = message["topology"]
    assert topology["face_positions"] == topology["face_indices"] == ""
    assert decode(topology["triangle_faces"], "<u4", 1).shape == (12, 1)
    assert decode(topology["segment_edges"], "<u4", 1).shape == (12, 1)
    assert decode(topology["vertices"], "<f4", 3).shape == (8, 3)
    assert message["bbox"] == [[-0.5] * 3, [0.5] * 3]
    angles = decode(message["segment_angles"], "<f4", 1)
    assert angles.shape == (12, 1)
    assert angles == pytest.approx(np.full((12, 1), 90.0))


def edge_angles_of(shape) -> list[float]:
    wrapped = shape.val().wrapped if isinstance(shape, cq.Workplane) else shape.wrapped
    return [
        round(float(a), 1)
        for a in tessellation.edge_angles(TopologyIndex.build(wrapped))
    ]


def test_edges_know_the_angle_their_faces_meet_at():
    # A box's edges are right angles.
    assert set(edge_angles_of(cq.Workplane().box(10, 10, 10))) == {90.0}
    # A fillet meets its faces smoothly; its far ends are still corners.
    filleted = cq.Workplane().box(10, 10, 10).edges("|Z").fillet(2)
    assert set(edge_angles_of(filleted)) == {0.0, 90.0}
    # A cylinder: two circles at right angles and a smooth seam.
    assert sorted(edge_angles_of(cq.Workplane().cylinder(10, 5))) == [0.0, 90.0, 90.0]
    # Faces split in one plane meet flat.
    split = (
        cq.Workplane()
        .box(10, 10, 1)
        .union(cq.Workplane().box(10, 10, 1).translate((10, 0, 0)), clean=False)
    )
    assert 0.0 in edge_angles_of(split)


def test_open_edges_and_wires_are_outlines():
    outline = tessellation.OPEN_EDGE_ANGLE
    assert set(edge_angles_of(cq.Workplane().rect(4, 4).val())) == {outline}
    wire = cq.Workplane().polyline([(0, 0), (1, 0), (1, 1)]).val()
    assert edge_angles_of(wire) == [outline]


def test_every_segment_carries_the_angle_of_its_edge():
    mesh = tessellate(
        cq.Workplane().box(10, 10, 10).edges("|Z").fillet(2).val().wrapped, Tolerance()
    )
    assert len(mesh.segment_angles) == len(mesh.edges)
    angles = tessellation.edge_angles(
        TopologyIndex.build(
            cq.Workplane().box(10, 10, 10).edges("|Z").fillet(2).val().wrapped
        )
    )
    assert mesh.segment_angles.tolist() == angles[mesh.segment_edges - 1].tolist()
    assert mesh.segment_angles.dtype == np.float32


def test_finer_tolerance_gives_more_triangles():
    shape = cq.Workplane().sphere(5).val().wrapped
    coarse = tessellate(shape, Tolerance(linear=1e-2, angular=0.5))
    fine = tessellate(
        cq.Workplane().sphere(5).val().wrapped, Tolerance(linear=1e-4, angular=0.1)
    )
    assert len(fine.indices) > len(coarse.indices)


def test_geometry_cache_survives_occt_meshing_side_effects():
    shape = cq.Workplane().sphere(5).val().wrapped
    tolerance = Tolerance()
    cache = GeometryCache()
    first_id, first = cache.tessellate(shape, tolerance, TopologyIndex.build(shape))

    # Meshing changes internal OCCT flags and therefore the raw BREP digest.
    # The cache aliases that post-mesh representation back to the original.
    second_id, second = cache.tessellate(shape, tolerance, TopologyIndex.build(shape))
    assert second_id == first_id
    assert second is first

    rebuilt = cq.Workplane().sphere(5).val().wrapped
    rebuilt_id, rebuilt_mesh = cache.tessellate(
        rebuilt, tolerance, TopologyIndex.build(rebuilt)
    )
    assert rebuilt_id == first_id
    assert rebuilt_mesh is first


def test_geometry_ids_name_the_mesher_that_made_them():
    # The GUI keeps meshes across worker restarts, also when Python moves to
    # another OCP build; a mesh of that other build must never be reused.
    shape = cq.Workplane().box(1, 2, 3).val().wrapped
    tolerance = Tolerance()
    current = GeometryCache().identifier(shape, tolerance)

    assert GeometryCache().identifier(shape, tolerance) == current
    assert GeometryCache(generator="another OCP").identifier(shape, tolerance) != (
        current
    )
    generator = tessellation.mesh_generator()
    assert f"cq-editor-mesh {tessellation.MESH_FORMAT};" in generator
    assert OCP.__version__ in generator
    # The installed binary, which tells environments with equal versions apart.
    assert os.path.normcase(os.path.abspath(OCP.OCP.__file__)) in generator


def test_large_structural_dimensions_keep_submillimeter_cross_sections():
    mesh = tessellate(
        cq.Workplane().box(1_000_000, 0.5, 0.25).val().wrapped, Tolerance()
    )
    lo, hi = mesh.bounds()

    assert hi[0] - lo[0] == pytest.approx(1_000_000, abs=0.05)
    assert hi[1] - lo[1] == pytest.approx(0.5, abs=1e-6)
    assert hi[2] - lo[2] == pytest.approx(0.25, abs=1e-6)


def _face_centroids(positions, indices, faces) -> set:
    corners = positions[np.asarray(indices).reshape(-1, 3)]
    centers = np.round(corners.mean(axis=1), 3)
    return {(int(face), *map(float, center)) for face, center in zip(faces, centers)}


def _sketch_faces():
    sketch = cq.Sketch().circle(4).rect(3, 3, mode="s")
    return cq.Compound.makeCompound(list(sketch.moved(cq.Location(cq.Vector(0, 0, 2)))))


_BOX = cq.Solid.makeBox(1, 1, 1)
FACE_ID_SHAPES = {
    "box": lambda: cq.Workplane().box(2, 4, 6).val(),
    "sphere": lambda: cq.Workplane().sphere(5).val(),
    "torus": lambda: cq.Solid.makeTorus(10, 2),
    "filleted holes": lambda: cq.Workplane()
    .box(40, 40, 5)
    .faces(">Z")
    .workplane()
    .rarray(10, 10, 3, 3)
    .hole(3)
    .edges("|Z")
    .fillet(0.5)
    .val(),
    "reversed faces of a cut": lambda: cq.Workplane()
    .box(10, 10, 10)
    .cut(cq.Workplane().sphere(6))
    .val(),
    "two solids": lambda: cq.Compound.makeCompound(
        [_BOX, cq.Solid.makeBox(1, 1, 1, cq.Vector(1, 0, 0))]
    ),
    "a solid listed twice": lambda: cq.Compound.makeCompound([_BOX, _BOX]),
    "sketch faces": _sketch_faces,
    "moved and rotated": lambda: cq.Workplane()
    .box(3, 3, 3)
    .val()
    .moved(cq.Location(cq.Vector(5, 0, 0), cq.Vector(0, 1, 0), 30)),
    "open shell": lambda: cq.Shell.makeShell(
        cq.Workplane().box(1, 2, 3).faces().vals()[:3]
    ),
}


@pytest.mark.parametrize("name", FACE_ID_SHAPES)
def test_display_triangles_carry_the_face_ids_of_separate_pick_triangles(name):
    shape = FACE_ID_SHAPES[name]().wrapped
    mesh = tessellate(shape, Tolerance())
    assert len(mesh.face_indices) == 0, "display triangles must be matched"
    assert len(mesh.triangle_faces) == len(mesh.indices) // 3

    # The former extraction, walking every face's triangulation, is the
    # oracle: each face must own exactly the same triangles.
    topology = TopologyIndex.build(shape)
    positions, indices, faces = tessellation._topology_faces(topology.faces)
    assert _face_centroids(
        mesh.positions, mesh.indices, mesh.triangle_faces
    ) == _face_centroids(positions, indices, faces)


def test_unmatched_display_triangles_fall_back_to_separate_pick_triangles(
    monkeypatch,
):
    monkeypatch.setattr(tessellation, "_display_triangle_faces", lambda *_: None)
    mesh = tessellate(cq.Workplane().box(1, 1, 1).val().wrapped, Tolerance())

    assert len(mesh.face_indices) == 12 * 3
    assert len(mesh.triangle_faces) == len(mesh.face_indices) // 3
    assert sorted(set(mesh.triangle_faces)) == [1, 2, 3, 4, 5, 6]


def test_face_ids_need_matching_triangle_counts():
    shape = cq.Workplane().box(1, 1, 1).val().wrapped
    mesh = tessellate(shape, Tolerance())  # triangulates the faces

    assert (
        len(tessellation._display_triangle_faces(shape, mesh.positions, mesh.indices))
        == len(mesh.indices) // 3
    )
    extra_triangle = np.concatenate((mesh.indices, mesh.indices[:3]))
    assert (
        tessellation._display_triangle_faces(shape, mesh.positions, extra_triangle)
        is None
    )
    invalid_indices = mesh.indices.astype(np.int64)
    invalid_indices[0] = -1
    assert (
        tessellation._display_triangle_faces(shape, mesh.positions, invalid_indices)
        is None
    )


def test_equal_triangle_counts_do_not_hide_reordered_face_groups(monkeypatch):
    """A changed glTF face order must select the independent fallback mesh."""

    original = tessellation._mesh_faces

    def reordered(*args):
        positions, normals, indices = original(*args)
        triangles = indices.reshape(-1, 3).copy()
        # Every box face has two triangles. Swap the first two complete face
        # groups, preserving both the per-face and total triangle counts.
        triangles[:4] = triangles[[2, 3, 0, 1]]
        return positions, normals, triangles.reshape(-1)

    monkeypatch.setattr(tessellation, "_mesh_faces", reordered)
    mesh = tessellate(cq.Workplane().box(1, 2, 3).val().wrapped, Tolerance())

    assert len(mesh.face_indices) == 12 * 3
    assert len(mesh.triangle_faces) == len(mesh.face_indices) // 3
    assert sorted(set(mesh.triangle_faces)) == [1, 2, 3, 4, 5, 6]


def test_triangle_order_inside_one_face_group_is_irrelevant(monkeypatch):
    original = tessellation._mesh_faces

    def reordered(*args):
        positions, normals, indices = original(*args)
        triangles = indices.reshape(-1, 3).copy()
        triangles[:2] = triangles[[1, 0]]
        return positions, normals, triangles.reshape(-1)

    monkeypatch.setattr(tessellation, "_mesh_faces", reordered)
    mesh = tessellate(cq.Workplane().box(1, 2, 3).val().wrapped, Tolerance())

    assert len(mesh.face_indices) == 0
    assert len(mesh.triangle_faces) == len(mesh.indices) // 3


def test_glb_round_trips_leave_only_an_empty_scratch_directory():
    tessellate(cq.Workplane().box(1, 1, 1).val().wrapped, Tolerance())
    first = tessellation._scratch_dir
    tessellate(cq.Workplane().sphere(1).val().wrapped, Tolerance())

    # One directory per worker process, reused and emptied after each mesh.
    assert tessellation._scratch_dir == first
    assert os.listdir(first) == []


def test_scratch_left_by_killed_workers_is_removed_once_stale(tmp_path):
    stale_dir = tmp_path / "cq-editor-mesh-killed"
    stale_dir.mkdir()
    (stale_dir / "mesh-1.glb").write_bytes(b"glTF")
    stale_file = tmp_path / "cq-editor-abc123.glb"  # older releases
    stale_file.write_bytes(b"glTF")
    fresh_dir = tmp_path / "cq-editor-mesh-running"
    fresh_dir.mkdir()
    unrelated = tmp_path / "notes.glb"
    unrelated.write_bytes(b"glTF")
    old = time.time() - 2 * tessellation.STALE_SCRATCH_SECONDS
    for path in (stale_dir, stale_file, unrelated):
        os.utime(path, (old, old))

    tessellation._remove_stale_scratch(str(tmp_path))

    assert not stale_dir.exists()
    assert not stale_file.exists()
    assert fresh_dir.exists()
    assert unrelated.exists()
