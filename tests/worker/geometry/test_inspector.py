"""CadQuery construction-stack descriptions and inspection overlays."""

import cadquery as cq

from cq_worker.inspector import (
    MAX_VALUE_LENGTH,
    describe_stack,
    frame_geometry,
    is_displayable,
    stack_frames,
    _text,
)
from cq_worker.tessellation import Tolerance


def test_stack_frames_are_newest_first_and_stop_at_root():
    box = cq.Workplane().box(1, 2, 3)
    face = box.faces(">Z")
    frames = stack_frames(face)
    assert frames[0] is face
    assert any(frame is box for frame in frames)
    assert stack_frames(cq.Workplane()) == []


def test_described_stack_marks_shapes_as_displayable():
    frame = cq.Workplane().box(1, 1, 1)
    (description,) = describe_stack([frame])
    assert description["type"] == "Workplane"
    assert description["items"][0]["type"] == "Solid"
    assert description["items"][0]["displayable"] is True
    assert "origin" in description["value"]


def test_vector_is_not_displayable_but_shape_is():
    assert not is_displayable(cq.Vector(1, 2, 3))
    assert is_displayable(cq.Workplane().box(1, 1, 1).val())


def test_point_only_frame_provides_grid_without_mesh():
    frame = cq.Workplane().pushPoints([(1, 2)])
    whole = frame_geometry(frame, None, Tolerance())
    single = frame_geometry(frame, 0, Tolerance())
    assert whole["meshes"] == []
    assert whole["plane"]["size"] > 0
    assert single == {"meshes": [], "plane": None}


def test_long_fallback_description_is_bounded():
    assert len(_text("x" * (MAX_VALUE_LENGTH + 20))) == MAX_VALUE_LENGTH
