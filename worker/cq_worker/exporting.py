"""Write displayed objects to CAD and mesh files."""

from __future__ import annotations

import inspect
import os
import tempfile
from xml.etree import ElementTree as ET
from zipfile import ZipFile

import cadquery as cq
from OCP.gp import gp_Vec
from OCP.Interface import Interface_Static
from OCP.STEPControl import STEPControl_Controller

from .checking import GeometryInvalid, check_registry, check_shape, invalid_reports
from .interference import analyze_interferences, apply_allowances
from .scene import Registry, RegistryEntry, to_compound

FORMATS = (
    "step",
    "stl",
    "brep",
    "3mf",
    "gltf",
    "glb",
    "amf",
    "vrml",
    "vtkjs",
    "xbf",
    "svg",
    "dxf",
)
UNITS = {
    "millimeter": ("MM", 0.001),
    "centimeter": ("CM", 0.01),
    "meter": ("M", 1.0),
    "inch": ("INCH", 0.0254),
}


def export(
    registry: Registry,
    keys: list[str],
    file_format: str,
    path: str,
    tolerance: float = 0.1,
    angular_tolerance: float = 0.1,
    step_mode: str = "default",
    step_schema: str = "ap242",
    stl_ascii: bool = False,
    flat_parts: bool = False,
    unit: str = "millimeter",
    on_invalid: str = "check",
    on_interference: str = "check",
    interference_allowances: list[tuple[str, str, str]] | None = None,
) -> None:
    """Export the objects behind ``keys`` (with their descendants).

    STEP always uses a named assembly, including several shown objects. The
    original CadQuery objects are never changed by export.
    """

    file_format = file_format.lower()
    if file_format not in FORMATS:
        raise ValueError(f"Unsupported export format {file_format!r}")
    if not keys:
        raise ValueError("Nothing to export")

    if step_mode not in ("default", "fused"):
        raise ValueError(f"Unsupported STEP mode {step_mode!r}")
    if step_schema not in ("ap214", "ap242"):
        raise ValueError(f"Unsupported STEP schema {step_schema!r}")
    if unit not in UNITS:
        raise ValueError("Choose the model unit before exporting")
    if on_invalid not in ("check", "ignore", "fix"):
        raise ValueError(f"Unsupported invalid-geometry action {on_invalid!r}")
    if on_interference not in ("check", "ignore"):
        raise ValueError(f"Unsupported interference action {on_interference!r}")
    bad = invalid_reports(check_registry(registry, keys, flat_parts))
    if bad and on_invalid == "check":
        raise GeometryInvalid(bad)
    repaired: dict[str, cq.Shape] = {}
    if bad and on_invalid == "fix":
        still_bad = []
        for report in bad:
            key = report["key"]
            shape = registry.get(key).shape
            if shape is None:
                raise ValueError(f"Cannot repair empty geometry {key!r}")
            fixed = shape.fix()
            checked = check_shape(fixed)
            if checked["valid"]:
                repaired[key] = fixed
            else:
                still_bad.append(
                    {
                        **report,
                        **checked,
                        "issues": [
                            *checked["issues"],
                            "ShapeFix did not resolve this geometry",
                        ],
                    }
                )
        if still_bad:
            raise GeometryInvalid(still_bad)

    physical = analyze_interferences(
        registry, keys, flat_parts=flat_parts, replacements=repaired
    )
    apply_allowances(
        registry,
        keys,
        physical,
        interference_allowances or [],
        flat_parts=flat_parts,
        replacements=repaired,
    )
    unsafe = [
        pair
        for pair in physical
        if pair["kind"] == "inconclusive"
        or (pair["kind"] == "interference" and not pair.get("allowance_reason"))
    ]
    if unsafe and on_interference == "check":
        raise GeometryInvalid([], unsafe)

    if file_format in ("step", "gltf", "glb", "vrml", "vtkjs", "xbf"):
        assembly = export_assembly(registry, keys, flat_parts, repaired)
        if file_format == "step":
            STEPControl_Controller.Init_s()
            previous = Interface_Static.CVal_s("write.step.schema")
            previous_unit = Interface_Static.CVal_s("write.step.unit")
            previous_source = Interface_Static.CVal_s("xstep.cascade.unit")
            schema = "AP242DIS" if step_schema == "ap242" else "AP214IS"
            if not Interface_Static.SetCVal_s("write.step.schema", schema):
                raise RuntimeError(f"Cannot select STEP schema {schema}")
            try:
                if not Interface_Static.SetCVal_s("xstep.cascade.unit", "MM"):
                    raise RuntimeError("Cannot set the STEP source unit")
                if not Interface_Static.SetCVal_s("write.step.unit", UNITS[unit][0]):
                    raise RuntimeError(f"Cannot set the STEP output unit to {unit}")
                if "outputUnit" in inspect.signature(cq.Assembly.export).parameters:
                    # CadQuery 2.8 sets both interface units inside export().
                    # Its defaults would overwrite the caller's model unit.
                    assembly.export(
                        path,
                        "STEP",
                        mode=step_mode,  # type: ignore[arg-type]
                        unit=UNITS[unit][0],  # type: ignore[arg-type]
                        outputUnit=UNITS[unit][0],  # type: ignore[arg-type]
                    )
                else:
                    assembly.export(path, "STEP", mode=step_mode)  # type: ignore[arg-type]
            finally:
                Interface_Static.SetCVal_s("write.step.schema", previous)
                Interface_Static.SetCVal_s("write.step.unit", previous_unit)
                Interface_Static.SetCVal_s("xstep.cascade.unit", previous_source)
        else:
            if file_format in ("gltf", "glb"):
                _scale_assembly(assembly, UNITS[unit][1])
            assembly.export(
                path,
                "GLTF" if file_format == "gltf" else file_format.upper(),  # type: ignore[arg-type]
                tolerance=tolerance,
                angularTolerance=angular_tolerance,
            )
            if file_format == "vtkjs":
                # CadQuery appends .zip itself. Reject a broken/empty scene
                # instead of reporting an export that cannot be opened.
                archive = path + ".zip"
                try:
                    with ZipFile(archive) as package:
                        if not package.namelist():
                            raise RuntimeError(
                                "VTK.js exporter produced an empty scene"
                            )
                    os.replace(archive, path)
                except Exception:
                    if os.path.exists(archive):
                        os.unlink(archive)
                    raise
        return

    parts = _export_parts(registry, keys, flat_parts)
    shapes = [repaired.get(key, entry.shape) for key, entry in parts]
    compound = to_compound(shapes)
    if file_format == "stl":
        compound.exportStl(
            path,
            tolerance=tolerance,
            angularTolerance=angular_tolerance,
            ascii=stl_ascii,
        )
    elif file_format == "brep":
        compound.exportBrep(path)
    elif file_format == "3mf":
        cq.exporters.export(
            compound,
            path,
            "3MF",
            tolerance,
            angular_tolerance,
            opt={"unit": unit},
        )
        _decorate_3mf(path, [entry for _, entry in parts])
    else:
        cq.exporters.export(
            compound,
            path,
            file_format.upper(),  # type: ignore[arg-type]
            tolerance,
            angular_tolerance,
        )


def export_assembly(
    registry: Registry,
    keys: list[str],
    flat_parts: bool = False,
    repaired: dict[str, cq.Shape] | None = None,
) -> cq.Assembly:
    """The requested nodes, with world placement, names and display colors.

    A selected ancestor owns its descendants. Ignoring separately selected
    descendants prevents a duplicated solid in the resulting STEP file.
    """

    repaired = repaired or {}
    descendants: set[str] = set()

    def mark_children(key: str) -> None:
        for child in registry.get(key).children:
            descendants.add(child)
            mark_children(child)

    if not flat_parts:
        for key in keys:
            mark_children(key)

    result = cq.Assembly(name="CQ-editor export")
    used_names: set[str] = set()
    for key in dict.fromkeys(keys):
        if key in descendants:
            continue
        entry = registry.get(key)
        name = entry.name or key
        original = name
        index = 2
        while name in used_names:
            name = f"{original} ({index})"
            index += 1
        used_names.add(name)
        if repaired and not flat_parts:
            _add_repaired_node(result, registry, key, name, repaired)
        elif flat_parts and entry.shape is not None:
            color = (
                entry.display_color
                or (entry.obj.color if isinstance(entry.obj, cq.Assembly) else None)
                or entry.inherited_color
            )
            result.add(repaired.get(key, entry.shape), name=name, color=color)
        elif isinstance(entry.obj, cq.Assembly):
            result.add(placed_assembly(entry), name=name)
        elif entry.shape is not None:
            result.add(entry.shape, name=name, color=entry.display_color)
    if not result.children:
        raise ValueError("No exportable geometry in the chosen objects")
    return result


def _add_repaired_node(
    parent: cq.Assembly,
    registry: Registry,
    key: str,
    name: str,
    repaired: dict[str, cq.Shape],
) -> None:
    """Rebuild just the export copy from world shapes, retaining hierarchy."""
    entry = registry.get(key)
    color = (
        entry.display_color
        or (entry.obj.color if isinstance(entry.obj, cq.Assembly) else None)
        or entry.inherited_color
    )
    if entry.children:
        group = cq.Assembly(name=name, color=color)
        if entry.shape is not None:
            group.add(repaired.get(key, entry.shape), name=f"{name} body", color=color)
        for child in entry.children:
            child_entry = registry.get(child)
            _add_repaired_node(
                group, registry, child, child_entry.name or child, repaired
            )
        parent.add(group, name=name, color=color)
    elif entry.shape is not None:
        parent.add(repaired.get(key, entry.shape), name=name, color=color)


def _scale_assembly(assembly: cq.Assembly, factor: float) -> None:
    """Convert a private export copy from model units to glTF meters."""
    for _, node in assembly.traverse():
        if node.obj is not None:
            node.obj = to_compound(node.obj).scale(factor)
        transform = node.loc.wrapped.Transformation()
        translation = transform.TranslationPart()
        transform.SetTranslationPart(
            gp_Vec(
                translation.X() * factor,
                translation.Y() * factor,
                translation.Z() * factor,
            )
        )
        node.loc = cq.Location(transform)


def _export_parts(
    registry: Registry, keys: list[str], flat_parts: bool
) -> list[tuple[str, RegistryEntry]]:
    """The individual displayed shapes in the order used by 3MF export."""
    parts: list[tuple[str, RegistryEntry]] = []
    seen: set[str] = set()

    def visit(key: str) -> None:
        if key in seen:
            return
        seen.add(key)
        entry = registry.get(key)
        if entry.shape is not None:
            parts.append((key, entry))
        if not flat_parts:
            for child in entry.children:
                visit(child)

    for key in keys:
        visit(key)
    return parts


def _decorate_3mf(path: str, parts: list[RegistryEntry]) -> None:
    """Add shown names and colors to CadQuery's unit-aware 3MF meshes.

    CadQuery's 3MF writer emits the geometry and package but anonymous
    uncolored mesh resources. The core 3MF base-material group gives each
    mesh a display color without changing its triangles or placement.
    """
    core = "http://schemas.microsoft.com/3dmanufacturing/core/2015/02"
    ET.register_namespace("", core)
    model_name = "3D/3dmodel.model"
    with ZipFile(path) as package:
        files = [(item, package.read(item.filename)) for item in package.infolist()]
    model = next(data for item, data in files if item.filename == model_name)
    root = ET.fromstring(model)
    resources = root.find(f"{{{core}}}resources")
    if resources is None:
        raise ValueError("3MF export has no resources")
    meshes = [
        object_node
        for object_node in resources.findall(f"{{{core}}}object")
        if object_node.find(f"{{{core}}}mesh") is not None
    ]
    if len(meshes) != len(parts):
        raise ValueError("3MF mesh count differs from the displayed parts")
    used_ids = [int(item.get("id", "0")) for item in resources if item.get("id")]
    materials = ET.Element(
        f"{{{core}}}basematerials", id=str(max(used_ids, default=0) + 1)
    )
    for index, (mesh, entry) in enumerate(zip(meshes, parts)):
        name = entry.name or f"Part {index + 1}"
        mesh.set("name", name)
        color = (
            entry.display_color
            or (entry.obj.color if isinstance(entry.obj, cq.Assembly) else None)
            or entry.inherited_color
        )
        if color is None:
            color = cq.Color(0.75, 0.75, 0.75)
        r, g, b, _ = color.toTuple()
        hex_color = "#{:02X}{:02X}{:02X}".format(
            *(round(max(0, min(1, value)) * 255) for value in (r, g, b))
        )
        ET.SubElement(materials, f"{{{core}}}base", name=name, displaycolor=hex_color)
        material_id = materials.get("id")
        assert material_id is not None
        mesh.set("pid", material_id)
        mesh.set("pindex", str(index))
    resources.insert(0, materials)
    encoded = ET.tostring(root, encoding="utf-8", xml_declaration=True)
    directory = os.path.dirname(os.path.abspath(path))
    with tempfile.NamedTemporaryFile(
        dir=directory, suffix=".3mf", delete=False
    ) as temp:
        temporary = temp.name
    try:
        with ZipFile(temporary, "w") as package:
            for item, data in files:
                package.writestr(item, encoded if item.filename == model_name else data)
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def placed_assembly(entry: RegistryEntry) -> cq.Assembly:
    """A copy of the assembly of ``entry`` as the view shows it.

    A node below the top of an assembly holds its own ``loc``, relative to
    its parent: exported alone, it is placed with its parents' location.
    The view also draws a node without a color in the color it inherits,
    while CadQuery's STEP writer only colors nodes that have one, so the
    copy gives every node its inherited color. The script's own assembly
    is left as it is.
    """

    assembly = entry.obj
    holder = cq.Assembly()
    holder.add(
        assembly,
        loc=(
            None
            if entry.parent_location is None
            else entry.parent_location * assembly.loc
        ),
    )
    placed = holder.children[0]
    _push_colors(placed, entry.inherited_color, entry.display_color)
    return placed


def _push_colors(
    node: cq.Assembly, inherited: cq.Color | None, override: cq.Color | None = None
) -> None:
    color = override or node.color or inherited
    node.color = color
    for child in node.children:
        _push_colors(child, color, override)
