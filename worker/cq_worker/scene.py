"""Turn the objects a script shows into scene nodes the GUI can draw.

A scene node carries display options, triangulated geometry and, for
assemblies, child nodes mirroring the assembly hierarchy. Every node gets a
key; the :class:`Registry` maps keys back to the live CadQuery objects so the
GUI can later ask for exports or stack inspection by key.
"""

from __future__ import annotations

import itertools
import logging
import math
import time
from collections.abc import Callable, Iterable, Mapping
from dataclasses import asdict, dataclass, field
from typing import Any

import cadquery as cq
from OCP.TopoDS import TopoDS_Shape

from .provenance import creation_line
from .tessellation import GeometryCache, Tolerance, TopologyIndex

log = logging.getLogger(__name__)

Color = tuple[float, float, float]


def location_matrix(location: cq.Location) -> list[float]:
    """Return an OCCT rigid transform in column-major order for glam/WebGPU."""

    transform = location.wrapped.Transformation()
    return [
        transform.Value(row, column) if row <= 3 else (1.0 if column == 4 else 0.0)
        for column in range(1, 5)
        for row in range(1, 5)
    ]


def is_cq_object(obj: Any) -> bool:
    return isinstance(obj, (cq.Workplane, cq.Shape, cq.Assembly, cq.Sketch))


def find_cq_objects(namespace: Mapping[str, Any]) -> dict[str, ShownObject]:
    """Every CadQuery object bound to a public name, for scripts that never
    call ``show_object``."""

    return {
        name: ShownObject(value)
        for name, value in namespace.items()
        if not name.startswith("_") and is_cq_object(value)
    }


def to_compound(obj: Any) -> cq.Compound:
    """Fuse anything shape-like into a single compound.

    Accepts workplanes, shapes, sketches, raw ``TopoDS_Shape`` objects and
    lists of those. Non-shape stack entries (vectors, locations) are skipped,
    and an empty input yields an empty compound.
    """

    shapes: list[cq.Shape] = []
    for item in obj if isinstance(obj, list) else [obj]:
        if isinstance(item, cq.Workplane):
            for value in item.vals():
                if isinstance(value, cq.Sketch):
                    shapes.extend(value)
                elif isinstance(value, cq.Shape):
                    shapes.append(value)
        elif isinstance(item, cq.Shape):
            shapes.append(item)
        elif isinstance(item, cq.Sketch):
            # Iterating a sketch yields its faces (or edges) with the sketch
            # placement applied.
            shapes.extend(item)
        elif isinstance(item, TopoDS_Shape):
            shapes.append(cq.Shape.cast(item))
        else:
            raise ValueError(f"Invalid type {type(item)}")
    return cq.Compound.makeCompound(shapes)


def parse_color(value: Any) -> tuple[Color, float | None]:
    """Normalize a user color specification to sRGB floats plus optional alpha.

    Supported: ``cq.Color``, ``"#rgb"``/``"#rrggbb"``/``"#rrggbbaa"``, color
    names known to OpenCascade (``"red"``, ``"steelblue"``, ...), and tuples
    of 3 or 4 components, either ints in 0..255 or floats in 0..1.
    """

    if isinstance(value, cq.Color):
        r, g, b, a = value.toTuple()
        return (r, g, b), a

    if isinstance(value, str):
        text = value.strip()
        if text.startswith("#"):
            digits = text[1:]
            if len(digits) in (3, 4):
                digits = "".join(c * 2 for c in digits)
            if len(digits) not in (6, 8):
                raise ValueError(f"Invalid hex color {value!r}")
            channels = [
                int(digits[i : i + 2], 16) / 255 for i in range(0, len(digits), 2)
            ]
            alpha = channels[3] if len(channels) == 4 else None
            return (channels[0], channels[1], channels[2]), alpha
        return parse_color(cq.Color(text))

    if isinstance(value, (tuple, list)) and len(value) in (3, 4):
        if all(isinstance(c, int) for c in value):
            channels = [c / 255 for c in value]
        else:
            channels = [float(c) for c in value]
        if not all(0.0 <= c <= 1.0 for c in channels):
            raise ValueError(f"Color components out of range: {value!r}")
        alpha = channels[3] if len(channels) == 4 else None
        return (channels[0], channels[1], channels[2]), alpha

    raise ValueError(f"Unsupported color specification {value!r}")


def finite(value: Any, what: str) -> float:
    """``value`` as a float; NaN and infinities cannot reach the GUI."""

    number = float(value)
    if not math.isfinite(number):
        raise ValueError(f"{what} must be a finite number, not {value!r}")
    return number


def non_finite(value: Any, where: str = "") -> str | None:
    """Where ``value``, a message, holds a number JSON cannot encode (NaN or
    an infinity), such as ``parts[0].transform``; ``None`` when it holds
    none. Strings, which hold the mesh arrays, are not looked into."""

    if isinstance(value, float):
        return None if math.isfinite(value) else where or "a number"
    if isinstance(value, dict):
        items = (
            (f"{where}.{key}" if where else str(key), v) for key, v in value.items()
        )
    elif isinstance(value, (list, tuple)):
        items = ((f"{where}[{index}]", v) for index, v in enumerate(value))
    else:
        return None
    for place, item in items:
        found = non_finite(item, place)
        if found is not None:
            return found
    return None


#: The surface looks ``show_object(..., material="matte plastic")`` offers:
#: plastics and metals with their roughness, and a procedural wood grain.
#: ``default`` is the plain shading of a CAD view.
FINISHES = (
    "default",
    "matte plastic",
    "glossy plastic",
    "brushed steel",
    "aluminum",
    "wood",
)
_FINISH_ALIASES = {"aluminium": "aluminum"}


def finish_name(value: Any) -> str:
    """The GUI's name of the material ``value`` names: case, spaces,
    hyphens and underscores do not matter (``"Matte-Plastic"``)."""

    if not isinstance(value, str):
        raise TypeError(
            f"material must be a name such as 'matte plastic', not {value!r}"
        )
    words = " ".join(value.replace("_", " ").replace("-", " ").lower().split())
    words = _FINISH_ALIASES.get(words, words)
    if words not in FINISHES:
        known = ", ".join(repr(name) for name in FINISHES)
        raise ValueError(f"unknown material {value!r}; known materials: {known}")
    return words.replace(" ", "_")


@dataclass
class DisplayOptions:
    """The ``options`` mapping accepted by ``show_object``."""

    color: Color | None = None
    #: Opacity in 0..1 (1 is opaque).
    alpha: float | None = None
    edge_color: Color | None = None
    #: The look of the surface, from ``material``: one of :data:`FINISHES`,
    #: as the GUI names it (``matte_plastic``).
    finish: str | None = None

    @classmethod
    def parse(cls, options: Mapping[str, Any] | None) -> DisplayOptions:
        """Each option on its own: an invalid one is ignored with a warning,
        and the others still apply."""

        result = cls()
        if not options:
            return result

        def rgba(value) -> None:
            r, g, b, a = value
            color, _ = parse_color((r, g, b))
            result.color, result.alpha = color, finite(a, "the rgba alpha")

        def color(value) -> None:
            result.color, alpha = parse_color(value)
            if alpha is not None and alpha < 1.0 and result.alpha is None:
                result.alpha = alpha

        def alpha(value) -> None:
            result.alpha = finite(value, "alpha")

        def edge_color(value) -> None:
            result.edge_color, _ = parse_color(value)

        def material(value) -> None:
            result.finish = finish_name(value)

        for key, apply in (
            ("rgba", rgba),
            ("color", color),
            ("alpha", alpha),
            ("edgecolor", edge_color),
            ("material", material),
        ):
            if key in options:
                try:
                    apply(options[key])
                except (TypeError, ValueError) as error:
                    log.warning("Ignoring invalid display option: %s", error)
        if result.alpha is not None:
            result.alpha = min(max(result.alpha, 0.0), 1.0)
        return result

    def to_message(self) -> dict:
        return {
            "color": list(self.color) if self.color else None,
            "alpha": self.alpha,
            "edge_color": list(self.edge_color) if self.edge_color else None,
            "finish": self.finish,
        }


@dataclass
class ShownObject:
    obj: Any
    options: Mapping[str, Any] = field(default_factory=dict)
    #: The script line of the ``show_object`` call, if made by the script.
    line: int | None = None


@dataclass(frozen=True)
class PartSource:
    """What a displayed part was made from, as the script holds it."""

    #: The workplane, shape or sketch of the script, in its own coordinates.
    obj: Any
    #: Where an assembly places it; identity for shown objects.
    location: cq.Location


@dataclass
class RegistryEntry:
    obj: Any
    #: Geometry to export for this node, already placed in world space.
    shape: cq.Shape | None
    children: list[str] = field(default_factory=list)
    #: Topological indices for each displayed part, in the same order as the
    #: scene node's parts. They keep picking ids tied to live OCCT subshapes.
    topologies: list[TopologyIndex] = field(default_factory=list)
    #: The source of each part, in the same order: what selectors and code
    #: generated for a picked entity refer to.
    sources: list[PartSource] = field(default_factory=list)
    #: Entity features per (part, kind), computed once for selectors.
    selector_features: dict[tuple[int, str], Any] = field(default_factory=dict)
    #: For a node below the top of an assembly: where its parents place it
    #: (world space) and the color it inherits from them. An export of the
    #: node alone puts it where the view shows it.
    parent_location: cq.Location | None = None
    inherited_color: cq.Color | None = None
    #: The name and display color of this node as shown by the script.
    name: str = ""
    display_color: cq.Color | None = None
    material: cq.Material | None = None
    face_attributes: list[dict] = field(default_factory=list)
    part_materials: list[cq.Material | None] = field(default_factory=list)
    part_shapes: list[cq.Shape] = field(default_factory=list)

    def subshape(self, part: int, kind: str, identifier: int) -> cq.Shape:
        try:
            topology = self.topologies[part]
        except IndexError:
            raise IndexError(f"Unknown part {part}") from None
        return cq.Shape.cast(topology.get(kind, identifier))


class Registry:
    """Keeps the CadQuery objects behind scene node keys alive.

    ``prefix`` namespaces the keys. The GUI keeps showing objects after it
    restarts the worker, so a fresh worker must never hand out the keys of
    its predecessor: releasing the stale objects would free the new ones.
    """

    def __init__(self, prefix: str = "n") -> None:
        self._prefix = prefix
        self._entries: dict[str, RegistryEntry] = {}
        self._counter = itertools.count(1)

    def add(self, entry: RegistryEntry) -> str:
        key = f"{self._prefix}{next(self._counter)}"
        self._entries[key] = entry
        return key

    def get(self, key: str) -> RegistryEntry:
        try:
            return self._entries[key]
        except KeyError:
            raise KeyError(
                f"Unknown object {key!r}; it may have been cleared "
                "or come from a previous Python session (render again)"
            ) from None

    def release(self, keys: Iterable[str]) -> None:
        for key in keys:
            entry = self._entries.pop(key, None)
            if entry is not None:
                self.release(entry.children)

    def keys(self) -> list[str]:
        return list(self._entries)

    def clear(self) -> None:
        self._entries.clear()

    def subtree_shapes(self, keys: Iterable[str]) -> list[cq.Shape]:
        shapes, seen = [], set()

        def visit(key: str) -> None:
            if key in seen:
                return
            seen.add(key)
            entry = self.get(key)
            if entry.shape is not None:
                shapes.append(entry.shape)
            for child in entry.children:
                visit(child)

        for key in keys:
            visit(key)
        return shapes


@dataclass
class SceneStats:
    """Where building the scenes of one request spent its time (seconds)."""

    #: Content ids: hashing each shape's BREP.
    identify: float = 0.0
    #: Topology indices and meshing of geometry not sent as a reference.
    tessellate: float = 0.0
    #: Base64 encoding of the mesh arrays.
    encode: float = 0.0
    #: Meshes triangulated now, served from the worker's cache, or sent as
    #: bare references to geometry the GUI holds.
    meshed: int = 0
    cached: int = 0
    referenced: int = 0
    #: Raw array bytes of the meshes sent in full.
    mesh_bytes: int = 0

    def to_message(self) -> dict:
        return asdict(self)


class BuildCancelled(Exception):
    """The GUI cancelled the request while its scene was being built."""


class MeshTransferError(Exception):
    """A single part cannot be sent within the transport's wire limit."""


# Base64 expands raw arrays by 4/3; leave room for JSON and metadata below
# the GUI's 512 MiB limit for one mesh-bearing message.
MAX_STREAMED_MESH_BYTES = 350 * 1024 * 1024


def count_parts(obj: Any) -> int:
    """How many meshes showing ``obj`` takes: one per shape of an assembly."""

    if isinstance(obj, cq.Assembly):
        own = 0 if obj.obj is None else 1
        return own + sum(count_parts(child) for child in obj.children)
    return 1


class SceneBuilder:
    def __init__(
        self,
        registry: Registry,
        tolerance: Tolerance,
        merge_assemblies: bool = False,
        geometry_cache: GeometryCache | None = None,
        known_geometry: Iterable[str] = (),
    ) -> None:
        self.registry = registry
        self.tolerance = tolerance
        self.merge_assemblies = merge_assemblies
        self.geometry_cache = geometry_cache or GeometryCache()
        # Updated as a response is built, so repeated parts in the same
        # assembly become references after the first full mesh.
        self.known_geometry = set(known_geometry)
        # Tessellation mutates OCCT's polygonal cache. Remember the signature
        # by source object so reuse within one run is unaffected by that.
        self._source_geometry: dict[int, str] = {}
        self.stats = SceneStats()
        #: Told (parts done, parts in all) as :meth:`build` goes.
        self.progress: Callable[[int, int], None] | None = None
        #: Asked before each part; true stops :meth:`build`.
        self.cancelled: Callable[[], bool] | None = None
        #: Send one complete part before its containing scene node is done.
        self.on_part: Callable[[str, dict], None] | None = None
        self._done = 0
        self._total = 0

    def build(self, objects: Mapping[str, ShownObject]) -> list[dict]:
        """The scene nodes of ``objects``. Raises :class:`BuildCancelled`,
        having released what it registered, when :attr:`cancelled` says so."""

        self._total = sum(count_parts(shown.obj) for shown in objects.values())
        self._done = 0
        if self.progress is not None:
            self.progress(0, self._total)
        registered = set(self.registry.keys())
        try:
            return self._build(objects)
        except (BuildCancelled, MeshTransferError, KeyboardInterrupt):
            self.registry.release(set(self.registry.keys()) - registered)
            raise

    def _build(self, objects: Mapping[str, ShownObject]) -> list[dict]:
        nodes = []
        for name, shown in objects.items():
            try:
                node = self.node(
                    name, shown.obj, DisplayOptions.parse(shown.options), shown.line
                )
            except (BuildCancelled, MeshTransferError):
                raise
            except Exception as error:
                log.error(
                    "Cannot display %s: %s: %s", name, type(error).__name__, error
                )
                continue
            if node is None:
                continue
            # One object with a NaN, such as in its placement, must not make
            # the whole reply impossible to send.
            where = non_finite(node)
            if where is not None:
                log.error("Cannot display %s: %s is not a finite number", name, where)
                self.registry.release([node["key"]])
                continue
            nodes.append(node)
        return nodes

    def node(
        self,
        name: str,
        obj: Any,
        options: DisplayOptions,
        shown_line: int | None = None,
    ) -> dict | None:
        """The node of a shown object. Its ``line`` is where the script
        created the object, else where it showed it."""

        message = self._node(name, obj, options)
        if message is not None and message["line"] is None:
            message["line"] = shown_line
        return message

    def _node(self, name: str, obj: Any, options: DisplayOptions) -> dict | None:
        if isinstance(obj, cq.Assembly):
            if self.merge_assemblies:
                return self._merged_assembly_node(name, obj, options)
            return self._assembly_node(
                name, obj, options, cq.Location(), None, top=True
            )

        shape = to_compound(obj)
        topology = TopologyIndex.build(shape.wrapped)
        part = self._part(shape, topology, None, None, cq.Location(), obj, name)
        if part is None:
            return None
        self._emit_part(name, part)
        key = self.registry.add(
            RegistryEntry(
                obj,
                shape,
                topologies=[topology],
                sources=[PartSource(obj, cq.Location())],
                name=str(name),
                display_color=(cq.Color(*options.color) if options.color else None),
            )
        )
        return self._message(key, name, obj, options, [part], [])

    def _assembly_node(
        self,
        name: str,
        assembly: cq.Assembly,
        options: DisplayOptions,
        parent_location: cq.Location,
        inherited_color: cq.Color | None,
        top: bool = False,
        inherited_material: cq.Material | None = None,
    ) -> dict | None:
        location = parent_location * assembly.loc
        color = assembly.color if assembly.color is not None else inherited_color
        material = (
            assembly.material if assembly.material is not None else inherited_material
        )

        shape, parts, topologies, sources = None, [], [], []
        if assembly.obj is not None:
            # A node can have both a shape and children.
            local_shape = to_compound(assembly.obj)
            shape = local_shape.moved(location)
            topology = TopologyIndex.build(shape.wrapped)
            part = self._part(
                local_shape,
                None,
                *self._part_color(color or self._material_color(material)),
                location,
                assembly.obj,
                name,
            )
            if part is not None:
                attributes = self._face_attributes(assembly, local_shape)
                part["face_attributes"] = attributes
                part["material"] = self._material_message(material)
                self._emit_part(name, part)
                parts.append(part)
                topologies.append(topology)
                sources.append(PartSource(assembly.obj, location))

        children = [
            child_node
            for child in assembly.children
            if (
                child_node := self._assembly_node(
                    child.name,
                    child,
                    options,
                    location,
                    color,
                    inherited_material=material,
                )
            )
            is not None
        ]
        if not parts and not children:
            return None

        entry = RegistryEntry(
            assembly,
            shape,
            children=[c["key"] for c in children],
            topologies=topologies,
            sources=sources,
            parent_location=None if top else parent_location,
            inherited_color=None if top else inherited_color,
            name=str(name),
            display_color=(cq.Color(*options.color) if options.color else None),
            material=material,
            face_attributes=attributes if parts else [],
        )
        key = self.registry.add(entry)
        message = self._message(key, name, assembly, options, parts, children)
        if assembly.obj is not None:
            # More telling than "Assembly" for the parts of an assembly.
            message["kind"] = type(assembly.obj).__name__
        return message

    def _merged_assembly_node(
        self, name: str, assembly: cq.Assembly, options: DisplayOptions
    ) -> dict | None:
        parts, topologies, sources, materials = [], [], [], []
        shapes: list[cq.Shape] = []

        def collect(
            node: cq.Assembly, parent: cq.Location, inherited, inherited_mat
        ) -> None:
            location = parent * node.loc
            color = node.color if node.color is not None else inherited
            material = node.material if node.material is not None else inherited_mat
            if node.obj is not None:
                local_shape = to_compound(node.obj)
                shape = local_shape.moved(location)
                shapes.append(shape)
                topology = TopologyIndex.build(shape.wrapped)
                part = self._part(
                    local_shape,
                    None,
                    *self._part_color(color or self._material_color(material)),
                    location,
                    node.obj,
                    node.name,
                )
                if part is not None:
                    part["face_attributes"] = self._face_attributes(node, local_shape)
                    part["material"] = self._material_message(material)
                    self._emit_part(node.name, part)
                    parts.append(part)
                    topologies.append(topology)
                    sources.append(PartSource(node.obj, location))
                    materials.append(material)
            for child in node.children:
                collect(child, location, color, material)

        collect(assembly, cq.Location(), None, None)
        if not parts:
            return None
        key = self.registry.add(
            RegistryEntry(
                assembly,
                cq.Compound.makeCompound(shapes),
                topologies=topologies,
                sources=sources,
                name=str(name),
                display_color=(cq.Color(*options.color) if options.color else None),
                part_materials=materials,
                part_shapes=shapes,
            )
        )
        return self._message(key, name, assembly, options, parts, [])

    @staticmethod
    def _part_color(color: cq.Color | None) -> tuple[Color | None, float | None]:
        if color is None:
            return None, None
        rgb, alpha = parse_color(color)
        return rgb, alpha

    def _emit_part(self, name: str, part: dict) -> None:
        if self.on_part is not None:
            where = non_finite(part)
            if where is not None:
                raise ValueError(
                    f"Cannot display {name}: {where} is not a finite number"
                )
            self.on_part(name, part)
            part["mesh"] = {"id": part["mesh"]["id"]}

    @staticmethod
    def _material_color(material: cq.Material | None) -> cq.Color | None:
        if material is None or not (
            material.wrapped_vis.HasPbrMaterial()
            or material.wrapped_vis.HasCommonMaterial()
        ):
            return None
        value = material.wrapped_vis.BaseColor()
        rgb = value.GetRGB()
        return cq.Color(rgb.Red(), rgb.Green(), rgb.Blue(), value.Alpha())

    @staticmethod
    def _material_message(material: cq.Material | None) -> dict | None:
        if material is None:
            return None
        return {
            "name": material.name,
            "density": finite(material.density, "material density"),
            "density_unit": material.densityUnit,
        }

    @staticmethod
    def _face_attributes(assembly: cq.Assembly, shape: cq.Shape) -> list[dict]:
        topology = TopologyIndex.build(shape.wrapped)
        index = {
            cq.Shape.cast(raw_face): identifier
            for identifier, raw_face in enumerate(topology.faces, 1)
        }
        attributes: dict[int, dict] = {}
        for field_name, values in (
            ("name", assembly._subshape_names),
            ("layer", assembly._subshape_layers),
            ("color", assembly._subshape_colors),
        ):
            for subshape, value in values.items():
                for face in subshape.Faces():
                    identifier = index.get(face)
                    if identifier is None:
                        continue
                    item = attributes.setdefault(identifier, {"id": identifier})
                    if field_name == "color":
                        rgb, alpha = parse_color(value)
                        item["color"] = list(rgb)
                        item["alpha"] = alpha
                    else:
                        item[field_name] = value
        return [attributes[identifier] for identifier in sorted(attributes)]

    def _part(
        self,
        shape: cq.Shape,
        topology: TopologyIndex | None,
        color: Color | None,
        alpha: float | None,
        location: cq.Location,
        source: Any,
        name: str,
    ) -> dict | None:
        if self.cancelled is not None and self.cancelled():
            raise BuildCancelled()
        try:
            return self._mesh_part(
                shape, topology, color, alpha, location, source, name
            )
        finally:
            self._done += 1
            if self.progress is not None:
                self.progress(min(self._done, self._total), self._total)

    def _mesh_part(
        self,
        shape: cq.Shape,
        topology: TopologyIndex | None,
        color: Color | None,
        alpha: float | None,
        location: cq.Location,
        source: Any,
        name: str,
    ) -> dict | None:
        stats = self.stats
        source_id = id(source)
        geometry_id = self._source_geometry.get(source_id)
        if geometry_id is None:
            started = time.perf_counter()
            geometry_id = self.geometry_cache.identifier(shape.wrapped, self.tolerance)
            geometry_id = self.geometry_cache.canonical_id(geometry_id)
            self._source_geometry[source_id] = geometry_id
            stats.identify += time.perf_counter() - started

        if geometry_id in self.known_geometry:
            mesh_message = {"id": geometry_id}
            stats.referenced += 1
        else:
            started = time.perf_counter()
            cached = self.geometry_cache.get(geometry_id) is not None
            topology = topology or TopologyIndex.build(shape.wrapped)
            _, mesh = self.geometry_cache.tessellate(
                shape.wrapped, self.tolerance, topology, geometry_id
            )
            stats.tessellate += time.perf_counter() - started
            if mesh is None:
                return None
            invalid = mesh.non_finite_array()
            if invalid is not None:
                raise ValueError(
                    f"Cannot display {name}: mesh {invalid} contains NaN or infinity"
                )
            if mesh.byte_size() > MAX_STREAMED_MESH_BYTES:
                raise MeshTransferError(
                    f"{name}: one part is too large to transfer; raise Deviation "
                    "in Preferences > 3D View or split the part"
                )
            started = time.perf_counter()
            mesh_message = mesh.to_message(geometry_id)
            stats.encode += time.perf_counter() - started
            if cached:
                stats.cached += 1
            else:
                stats.meshed += 1
            stats.mesh_bytes += mesh.byte_size()
            self.known_geometry.add(geometry_id)
        part = {
            "mesh": mesh_message,
            "transform": location_matrix(location),
            "color": list(color) if color else None,
            "alpha": alpha,
        }
        return part

    @staticmethod
    def _message(key, name, obj, options, parts, children) -> dict:
        line = creation_line(obj)
        if line is None and isinstance(obj, cq.Assembly):
            line = creation_line(obj.obj)
        return {
            "key": key,
            "name": str(name),
            "kind": type(obj).__name__,
            "line": line,
            "inspectable": isinstance(obj, cq.Workplane) and obj.parent is not None,
            **options.to_message(),
            "parts": parts,
            "children": children,
        }
