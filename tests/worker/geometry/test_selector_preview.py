"""The caret preview reads selectors without evaluating source expressions."""

import cadquery as cq

from cq_worker.scene import Registry, SceneBuilder, ShownObject
from cq_worker.selector_preview import preview
from cq_worker.tessellation import Tolerance


def displayed(obj):
    registry = Registry()
    nodes = SceneBuilder(registry, Tolerance()).build({"part": ShownObject(obj)})
    return registry, nodes[0]["key"]


def test_literal_selector_matches_exact_displayed_topology():
    part = cq.Workplane().box(2, 3, 4)
    registry, key = displayed(part)
    source = 'part.faces(">Z")\n'
    result = preview(registry, {"part": part}, source, 1, 12)
    assert result["active"]
    assert result["count"] == 1
    assert len(result["matches"]) == 1
    match = result["matches"][0]
    assert match["key"] == key
    assert match["kind"] == "face"
    selected = registry.get(key).subshape(match["part"], "face", match["identifier"])
    assert selected.wrapped.IsSame(part.faces(">Z").val().wrapped)


def test_empty_and_invalid_selectors_report_without_stale_highlights():
    part = cq.Workplane().box(2, 3, 4)
    registry, _ = displayed(part)
    empty = preview(registry, {"part": part}, 'part.faces(">X and <X")', 1, 13)
    assert empty["active"] and empty["count"] == 0 and empty["matches"] == []
    invalid = preview(registry, {"part": part}, 'part.faces("invalid[")', 1, 13)
    assert invalid["active"] and invalid["message"].startswith("Invalid selector")
    outside = preview(registry, {"part": part}, 'part.faces(">Z")\nx = 1', 2, 2)
    assert not outside["active"]


def test_caret_column_counts_characters_before_non_ascii_text():
    part = cq.Workplane().box(2, 3, 4)
    registry, _ = displayed(part)
    source = 'label = "peça"; part.faces(">Z")'
    column = source.index("faces") + 2
    result = preview(registry, {"part": part}, source, 1, column)
    assert result["active"] and result["count"] == 1


def test_dynamic_receiver_and_argument_are_never_executed():
    part = cq.Workplane().box(2, 3, 4)
    registry, _ = displayed(part)
    calls = []

    def dangerous():
        calls.append(1)
        return part

    dynamic_receiver = preview(
        registry,
        {"part": part, "dangerous": dangerous},
        'dangerous().faces(">Z")',
        1,
        18,
    )
    dynamic_argument = preview(
        registry, {"part": part}, "part.faces(dangerous())", 1, 16
    )
    assert dynamic_receiver["message"] and dynamic_argument["message"]
    assert calls == []
