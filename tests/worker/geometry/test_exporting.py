"""Exports put shapes where the view shows them."""

import json
from xml.etree import ElementTree as ET
from zipfile import ZipFile

import cadquery as cq
import pytest
from cq_worker import exporting
from cq_worker.scene import Registry, SceneBuilder, ShownObject
from cq_worker.tessellation import Tolerance


def nested_assembly():
    """A box 50 mm along x in ``sub``, itself at x = 1000 in a red root at
    y = 500: in the world the box spans x 1045…1055, y 495…505."""

    child = cq.Assembly(
        cq.Workplane().box(10, 10, 10), name="child", loc=cq.Location((50, 0, 0))
    )
    sub = cq.Assembly(name="sub", loc=cq.Location((1000, 0, 0)))
    sub.add(child)
    root = cq.Assembly(
        name="root", loc=cq.Location((0, 500, 0)), color=cq.Color(1, 0, 0)
    )
    root.add(sub)
    return root


def build(obj):
    registry = Registry()
    nodes = SceneBuilder(registry, Tolerance()).build({"assy": ShownObject(obj)})
    return registry, nodes


def bounds(shape):
    box = shape.BoundingBox()
    return [round(value, 3) for value in (box.xmin, box.xmax, box.ymin, box.ymax)]


@pytest.mark.parametrize("level", ["root", "sub", "child"])
def test_any_assembly_node_is_exported_where_the_view_shows_it(tmp_path, level):
    registry, nodes = build(nested_assembly())
    node = nodes[0]
    for _ in range(["root", "sub", "child"].index(level)):
        node = node["children"][0]
    key = node["key"]

    step, brep = tmp_path / "part.step", tmp_path / "part.brep"
    exporting.export(registry, [key], "step", str(step))
    exporting.export(registry, [key], "brep", str(brep))

    world = [1045.0, 1055.0, 495.0, 505.0]
    assert bounds(cq.Shape.importBrep(str(brep))) == world
    assert bounds(cq.importers.importStep(str(step)).val()) == world


def step_part_colors(path):
    """The colors of the parts (nodes with a shape) of a STEP file."""

    return [
        node.color.toTuple()[:3] if node.color else None
        for _, node in cq.Assembly.importStep(str(path)).traverse()
        if node.obj is not None
    ]


@pytest.mark.parametrize("level", ["root", "sub"])
def test_parts_are_exported_in_the_color_they_inherit(tmp_path, level):
    # The view draws the box red, as its root; so must the file.
    root = nested_assembly()
    registry, nodes = build(root)
    node = nodes[0] if level == "root" else nodes[0]["children"][0]
    step = tmp_path / "part.step"
    exporting.export(registry, [node["key"]], "step", str(step))

    colors = step_part_colors(step)
    assert colors and all(
        color == pytest.approx((1, 0, 0), abs=1e-3) for color in colors
    ), colors
    # The script's own assembly is not touched.
    assert root.children[0].color is None
    assert root.children[0].children[0].color is None


def test_several_shown_objects_keep_names_colors_and_positions_in_step(tmp_path):
    registry = Registry()
    nodes = SceneBuilder(registry, Tolerance()).build(
        {
            "base": ShownObject(cq.Workplane().box(10, 10, 2), {"color": "steelblue"}),
            "pin": ShownObject(
                cq.Workplane().center(50, 0).cylinder(10, 2),
                {"color": "red"},
            ),
        }
    )
    path = tmp_path / "parts.step"
    exporting.export(registry, [node["key"] for node in nodes], "step", str(path))
    imported = cq.Assembly.importStep(str(path))
    parts = {node.name: node for _, node in imported.traverse() if node.obj is not None}
    assert set(parts) == {"base", "pin"}
    assert bounds(parts["base"].obj) == [-5, 5, -5, 5]
    assert bounds(parts["pin"].obj.moved(parts["pin"].loc)) == [48, 52, -2, 2]
    assert parts["pin"].color.toTuple()[:3] == pytest.approx((1, 0, 0))


@pytest.mark.parametrize(
    "schema,header",
    [
        ("ap214", "AUTOMOTIVE_DESIGN"),
        ("ap242", "AP242_MANAGED_MODEL"),
    ],
)
def test_step_schema_is_written_into_the_file(tmp_path, schema, header):
    registry, nodes = build(cq.Workplane().box(1, 1, 1))
    path = tmp_path / "part.step"
    exporting.export(registry, [nodes[0]["key"]], "step", str(path), step_schema=schema)
    assert header in path.read_text(errors="ignore")


def test_fused_step_is_an_explicit_export_option(tmp_path):
    registry, nodes = build(cq.Workplane().box(2, 2, 2))
    path = tmp_path / "fused.step"
    exporting.export(registry, [nodes[0]["key"]], "step", str(path), step_mode="fused")
    assert bounds(cq.importers.importStep(str(path)).val()) == [-1, 1, -1, 1]


@pytest.mark.parametrize(
    "file_format",
    ["stl", "brep", "3mf", "gltf", "glb", "amf", "vrml", "xbf", "svg", "dxf"],
)
def test_other_export_formats_write_a_nonempty_file(tmp_path, file_format):
    registry, nodes = build(cq.Workplane().box(1, 1, 1))
    path = tmp_path / f"part.{file_format}"
    exporting.export(registry, [nodes[0]["key"]], file_format, str(path))
    assert path.stat().st_size > 100


def test_3mf_carries_each_shown_name_and_color(tmp_path):
    registry = Registry()
    nodes = SceneBuilder(registry, Tolerance()).build(
        {
            "base": ShownObject(cq.Workplane().box(2, 2, 2), {"color": "red"}),
            "pin": ShownObject(
                cq.Workplane().center(10, 0).box(1, 1, 1), {"color": "blue"}
            ),
        }
    )
    path = tmp_path / "parts.3mf"
    exporting.export(registry, [node["key"] for node in nodes], "3mf", str(path))
    with ZipFile(path) as archive:
        model = ET.fromstring(archive.read("3D/3dmodel.model"))
    core = "{http://schemas.microsoft.com/3dmanufacturing/core/2015/02}"
    resources = model.find(f"{core}resources")
    materials = resources.find(f"{core}basematerials")
    colors = {base.get("name"): base.get("displaycolor") for base in materials}
    assert colors == {"base": "#FF0000", "pin": "#0000FF"}
    objects = [
        node
        for node in resources.findall(f"{core}object")
        if node.find(f"{core}mesh") is not None
    ]
    assert {node.get("name") for node in objects} == {"base", "pin"}


@pytest.mark.parametrize(
    "unit,millimeters,meters",
    [
        ("millimeter", 1.0, 0.001),
        ("centimeter", 10.0, 0.01),
        ("meter", 1000.0, 1.0),
        ("inch", 25.4, 0.0254),
    ],
)
def test_units_reach_step_3mf_and_gltf(tmp_path, unit, millimeters, meters):
    registry, nodes = build(cq.Workplane().box(1, 1, 1))
    key = nodes[0]["key"]
    step = tmp_path / "part.step"
    three_mf = tmp_path / "part.3mf"
    gltf = tmp_path / "part.gltf"
    exporting.export(registry, [key], "step", str(step), unit=unit)
    exporting.export(registry, [key], "3mf", str(three_mf), unit=unit)
    exporting.export(registry, [key], "gltf", str(gltf), unit=unit)

    assert cq.importers.importStep(str(step)).val().BoundingBox().xlen == pytest.approx(
        millimeters
    )
    with ZipFile(three_mf) as archive:
        model = ET.fromstring(archive.read("3D/3dmodel.model"))
    assert model.get("unit") == unit
    accessors = json.loads(gltf.read_text())["accessors"]
    x_coordinates = [
        bound[0]
        for accessor in accessors
        for bound in (accessor.get("min", []), accessor.get("max", []))
        if len(bound) == 3
    ]
    assert max(x_coordinates) - min(x_coordinates) == pytest.approx(meters)


def test_export_requires_a_known_model_unit(tmp_path):
    registry, nodes = build(cq.Workplane().box(1, 1, 1))
    with pytest.raises(ValueError, match="Choose the model unit"):
        exporting.export(
            registry,
            [nodes[0]["key"]],
            "step",
            str(tmp_path / "part.step"),
            unit="none",
        )
