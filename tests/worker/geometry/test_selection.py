"""Selectors generated for picked faces, edges and vertices."""

import cadquery as cq
import pytest
from OCP.TopAbs import TopAbs_EDGE, TopAbs_FACE, TopAbs_VERTEX
from OCP.TopExp import TopExp

from cq_worker import selection
from cq_worker.measurement import MeasurementError
from cq_worker.selection import (
    Features,
    PickedSelection,
    Receiver,
    SelectorError,
    atoms,
    selector_candidates,
)
from cq_worker.scene import Registry, SceneBuilder, ShownObject, to_compound
from cq_worker.tessellation import ShapeMap, Tolerance

_KINDS = {"face": TopAbs_FACE, "edge": TopAbs_EDGE, "vertex": TopAbs_VERTEX}
_LISTS = {"face": "Faces", "edge": "Edges", "vertex": "Vertices"}


def show(objects, merge_assemblies=False):
    registry = Registry()
    builder = SceneBuilder(registry, Tolerance(), merge_assemblies=merge_assemblies)
    nodes = builder.build({name: ShownObject(obj) for name, obj in objects.items()})
    return registry, nodes


def topology_ids(obj, kind, shapes):
    """The 1-based ids the GUI picks for ``shapes`` of ``obj``."""

    # The worker's map type: OCP 8 dropped the TopTools_IndexedMapOfShape alias.
    index = ShapeMap()
    TopExp.MapShapes_s(to_compound(obj).wrapped, _KINDS[kind], index)
    return [index.FindIndex(shape.wrapped) for shape in shapes]


def references(key, kind, ids, part=0):
    return [
        {"key": key, "part": part, "kind": kind, "identifier": identifier}
        for identifier in ids
    ]


def apply(obj, expression):
    """Runs the expression the way a script does: appended to the object."""

    result = eval("obj" + expression, {"obj": obj, "cq": cq})
    return result.vals() if isinstance(result, cq.Workplane) else [result]


def entities(obj, kind):
    shape = to_compound(obj)
    return getattr(shape, _LISTS[kind])()


def selected(obj, kind, expression):
    found = []
    for shape in apply(obj, expression):
        found.extend(getattr(shape, _LISTS[kind])())
    return set(found)


MODELS = {
    "box": lambda: cq.Workplane().box(10, 20, 30),
    "cylinder": lambda: cq.Workplane().cylinder(10, 5),
    "plate with holes": lambda: cq.Workplane()
    .box(40, 40, 5)
    .faces(">Z")
    .workplane()
    .rarray(10, 10, 3, 3)
    .hole(3)
    .edges("|Z")
    .fillet(0.5),
    "chamfered box": lambda: cq.Workplane().box(10, 10, 10).edges(">Z").chamfer(2),
    "bracket": lambda: cq.Workplane()
    .box(30, 20, 4)
    .faces(">Y")
    .workplane()
    .rect(30, 4)
    .extrude(15)
    .edges("|X")
    .fillet(1),
}


@pytest.mark.parametrize("model", MODELS)
@pytest.mark.parametrize("kind", ["face", "edge", "vertex"])
def test_every_entity_gets_a_selector_that_selects_exactly_it(model, kind):
    obj = MODELS[model]()
    registry, (node,) = show({"part": obj})
    for shape in entities(obj, kind):
        (identifier,) = topology_ids(obj, kind, [shape])
        result = selector_candidates(
            registry, references(node["key"], kind, [identifier])
        )
        assert result["method"] == {"face": "faces", "edge": "edges"}.get(
            kind, "vertices"
        )
        for candidate in result["candidates"]:
            assert selected(obj, kind, candidate["expression"]) == {shape}, candidate
        # These models never need the index fallback.
        assert not result["candidates"][0]["index_based"]


def test_simple_selectors_are_preferred():
    box = cq.Workplane().box(10, 20, 30)
    registry, (node,) = show({"box": box})

    def best(kind, selector):
        picked = getattr(box, {"face": "faces", "edge": "edges"}[kind])(selector)
        ids = topology_ids(box, kind, picked.vals())
        result = selector_candidates(registry, references(node["key"], kind, ids))
        return result["candidates"][0]["expression"]

    assert best("face", ">Z") == '.faces(">Z")'
    assert best("face", "<X") == '.faces("<X")'
    assert best("edge", "|Z") == '.edges("|Z")'
    assert best("edge", ">Z") == '.edges(">Z")'
    assert best("edge", "|Z and >Y and >X") == '.edges(">X and >Y")'
    assert best("face", ">X or <Y") == '.faces(">X or <Y")'


def test_curved_faces_are_named_by_their_type():
    cylinder = cq.Workplane().cylinder(10, 5)
    registry, (node,) = show({"cylinder": cylinder})
    side = cylinder.faces("%CYLINDER").vals()
    result = selector_candidates(
        registry,
        references(node["key"], "face", topology_ids(cylinder, "face", side)),
    )
    assert result["candidates"][0]["expression"] == '.faces("%CYLINDER")'
    assert result["candidates"][0]["description"] == "cylinder"


@pytest.mark.parametrize(
    "distances, direction", [((3,), "(1,0,1)"), ((3, 1), "(3,0,1)")]
)
def test_inclined_faces_get_a_direction_selector(distances, direction):
    wedge = cq.Workplane().box(10, 10, 10).edges(">Z and >X").chamfer(*distances)
    registry, (node,) = show({"wedge": wedge})
    chamfer = wedge.faces("not (|X or |Y or |Z)").vals()
    assert len(chamfer) == 1
    result = selector_candidates(
        registry,
        references(node["key"], "face", topology_ids(wedge, "face", chamfer)),
    )
    for candidate in result["candidates"]:
        assert selected(wedge, "face", candidate["expression"]) == set(chamfer)
    # What the face is, not where it ranks among its neighbors.
    assert result["candidates"][0]["expression"] == f'.faces("+{direction}")'


def test_selectors_of_an_assembly_part_use_its_own_coordinates():
    plate = cq.Workplane().box(10, 10, 2)
    assembly = cq.Assembly().add(
        plate, name="plate", loc=cq.Location((0, 0, 50), (1, 0, 0), 90)
    )
    for merge in (False, True):
        registry, (node,) = show({"assembly": assembly}, merge_assemblies=merge)
        key = node["key"] if merge else node["children"][0]["key"]
        top = topology_ids(plate, "face", plate.faces(">Z").vals())
        result = selector_candidates(registry, references(key, "face", top))
        assert result["candidates"][0]["expression"] == '.faces(">Z")'


def test_shapes_are_receivers_too():
    solid = cq.Workplane().box(4, 6, 8).val()
    registry, (node,) = show({"solid": solid})
    top = solid.faces(">Z")
    result = selector_candidates(
        registry, references(node["key"], "face", topology_ids(solid, "face", [top]))
    )
    assert result["candidates"][0]["expression"] == '.faces(">Z")'
    for candidate in result["candidates"]:
        assert selected(solid, "face", candidate["expression"]) == {top}


@pytest.mark.parametrize(
    "obj",
    [cq.Workplane().box(4, 6, 8), cq.Workplane().box(4, 6, 8).val()],
    ids=["workplane", "shape"],
)
def test_the_index_fallback_selects_exactly_the_picked_entity(obj, monkeypatch):
    monkeypatch.setattr(selection, "search", lambda features, targets: [])
    registry, (node,) = show({"part": obj})
    face = entities(obj, "face")[3]
    result = selector_candidates(
        registry, references(node["key"], "face", topology_ids(obj, "face", [face]))
    )

    (candidate,) = result["candidates"]
    assert candidate["index_based"]
    assert "may change with the model" in candidate["description"]
    assert selected(obj, "face", candidate["expression"]) == {face}


def test_several_entities_of_a_workplane_have_an_index_fallback(monkeypatch):
    monkeypatch.setattr(selection, "search", lambda features, targets: [])
    box = cq.Workplane().box(4, 6, 8)
    registry, (node,) = show({"box": box})
    picked = entities(box, "edge")[2:4]
    result = selector_candidates(
        registry, references(node["key"], "edge", topology_ids(box, "edge", picked))
    )
    (candidate,) = result["candidates"]
    assert candidate["expression"] == ".edges()[[2, 3]]"
    assert selected(box, "edge", candidate["expression"]) == set(picked)


@pytest.mark.parametrize("model", ["box", "plate with holes", "chamfered box"])
@pytest.mark.parametrize("kind", ["face", "edge", "vertex"])
def test_simple_selectors_are_evaluated_exactly_as_cadquery_does(model, kind):
    obj = MODELS[model]()
    receiver = Receiver(obj, kind)
    features = Features.of(receiver.entities())
    for atom in atoms(features):
        expected = set(receiver.select(atom.expression))
        found = {
            features.entities[i]
            for i in range(len(features.entities))
            if atom.mask >> i & 1
        }
        assert found == expected, atom.expression


def test_entities_of_two_objects_or_two_kinds_have_no_selector():
    box = cq.Workplane().box(1, 1, 1)
    registry, nodes = show({"a": box, "b": cq.Workplane().sphere(1)})
    with pytest.raises(SelectorError, match="one object"):
        selector_candidates(
            registry,
            references(nodes[0]["key"], "face", [1])
            + references(nodes[1]["key"], "face", [1]),
        )
    with pytest.raises(SelectorError, match="one kind"):
        selector_candidates(
            registry,
            references(nodes[0]["key"], "face", [1])
            + references(nodes[0]["key"], "edge", [1]),
        )
    with pytest.raises(SelectorError, match="Select faces"):
        selector_candidates(registry, [])


def test_stale_or_unsupported_objects_report_why():
    registry, (node,) = show({"sketch": cq.Sketch().rect(2, 2)})
    with pytest.raises(SelectorError, match="not for a Sketch"):
        selector_candidates(registry, references(node["key"], "face", [1]))
    registry.release([node["key"]])
    with pytest.raises(SelectorError, match="Unknown object"):
        selector_candidates(registry, references(node["key"], "face", [1]))

    registry, (node,) = show({"box": cq.Workplane().box(1, 1, 1)})
    with pytest.raises(SelectorError, match="Unknown face id 7"):
        selector_candidates(registry, references(node["key"], "face", [7]))


# -- the console's selected() ---------------------------------------------------


def test_picked_entities_resolve_to_live_shapes_placed_as_shown():
    plate = cq.Workplane().box(10, 10, 2)
    assembly = cq.Assembly().add(plate, name="plate", loc=cq.Location((0, 0, 50)))
    registry, (node,) = show({"assembly": assembly})
    key = node["children"][0]["key"]
    top = topology_ids(plate, "face", plate.faces(">Z").vals())
    edge = topology_ids(plate, "edge", plate.edges("|Z and >X and >Y").vals())

    picked = PickedSelection.from_message(
        {
            "entities": references(key, "face", top) + references(key, "edge", edge),
            "objects": [node["key"]],
        }
    )
    face, line = picked.resolve(registry)

    assert isinstance(face, cq.Face) and isinstance(line, cq.Edge)
    assert face.Area() == pytest.approx(100)
    # In the assembly's coordinates, as the 3D view shows it.
    assert face.Center().z == pytest.approx(51)
    assert line.Length() == pytest.approx(2)


def test_without_picked_entities_the_selected_objects_are_returned():
    box = cq.Workplane().box(1, 2, 3)
    assembly = (
        cq.Assembly()
        .add(cq.Workplane().box(1, 1, 1), name="a")
        .add(cq.Workplane().sphere(1), name="b", loc=cq.Location((5, 0, 0)))
    )
    registry, (box_node, assembly_node) = show({"box": box, "assembly": assembly})

    solid, both = PickedSelection.from_message(
        {"objects": [box_node["key"], assembly_node["key"]]}
    ).resolve(registry)

    # A shown workplane holding one solid is that solid, not a compound.
    assert isinstance(solid, cq.Solid) and solid.Volume() == pytest.approx(6)
    assert isinstance(both, cq.Compound) and len(both.Solids()) == 2
    assert PickedSelection.from_message(None).resolve(registry) == []


def test_a_stale_selection_explains_itself():
    box = cq.Workplane().box(1, 1, 1)
    registry, (node,) = show({"box": box})
    gone = PickedSelection.from_message(
        {"entities": references(node["key"], "face", [9])}
    )
    with pytest.raises(LookupError, match="pick it again"):
        gone.resolve(registry)
    registry.release([node["key"]])
    stale = PickedSelection.from_message({"objects": [node["key"]]})
    with pytest.raises(LookupError, match="previous Python session"):
        stale.resolve(registry)


@pytest.mark.parametrize(
    "message",
    [[], {"entities": {}}, {"objects": [3]}, {"entities": [{"key": "n1"}]}],
)
def test_malformed_selections_are_rejected(message):
    with pytest.raises(MeasurementError):
        PickedSelection.from_message(message)
