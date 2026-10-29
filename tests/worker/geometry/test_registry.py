"""Scene key lifecycle and export selection."""

import cadquery as cq
import pytest

from cq_worker.scene import Registry, RegistryEntry
from cq_worker.tessellation import TopologyIndex


def test_subtree_shapes_deduplicates_overlapping_selection():
    registry = Registry()
    first = cq.Workplane().box(1, 1, 1).val()
    second = cq.Workplane().sphere(1).val()
    child = registry.add(RegistryEntry(second, second))
    parent = registry.add(RegistryEntry(first, first, [child]))

    assert registry.subtree_shapes([parent, child, parent]) == [first, second]


def test_release_removes_children_but_keeps_other_roots():
    registry = Registry()
    child = registry.add(RegistryEntry("child", None))
    parent = registry.add(RegistryEntry("parent", None, [child]))
    other = registry.add(RegistryEntry("other", None))
    registry.release([parent, "unknown"])

    assert registry.get(other).obj == "other"
    for key in (parent, child):
        with pytest.raises(KeyError, match="may have been cleared"):
            registry.get(key)


def test_clear_drops_entries_without_reusing_old_keys():
    registry = Registry()
    old = registry.add(RegistryEntry("old", None))
    registry.clear()
    new = registry.add(RegistryEntry("new", None))
    assert old != new
    with pytest.raises(KeyError):
        registry.get(old)


def test_prefixed_registries_never_share_keys():
    old = Registry("s1-n")
    new = Registry("s2-n")
    stale = old.add(RegistryEntry("old", None))
    fresh = new.add(RegistryEntry("new", None))

    assert (stale, fresh) == ("s1-n1", "s2-n1")
    # The GUI releases the stale key after a restart; the new object survives.
    new.release([stale])
    assert new.get(fresh).obj == "new"


def test_subtree_shapes_skips_nodes_without_geometry():
    registry = Registry()
    shape = cq.Workplane().box(1, 1, 1).val()
    child = registry.add(RegistryEntry(shape, shape))
    parent = registry.add(RegistryEntry("group", None, [child]))
    assert registry.subtree_shapes([parent]) == [shape]


def test_topology_ids_resolve_to_the_live_occt_subshapes():
    shape = cq.Workplane().box(1, 2, 3).val()
    topology = TopologyIndex.build(shape.wrapped)
    entry = RegistryEntry(shape, shape, topologies=[topology])

    assert entry.subshape(0, "face", 1).ShapeType() == "Face"
    assert entry.subshape(0, "edge", 12).ShapeType() == "Edge"
    assert entry.subshape(0, "vertex", 8).ShapeType() == "Vertex"
    with pytest.raises(IndexError, match="Unknown face id"):
        entry.subshape(0, "face", 7)
