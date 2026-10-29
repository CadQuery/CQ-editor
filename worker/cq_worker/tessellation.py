"""Triangulate OpenCascade shapes into compact, GUI-ready arrays.

Faces are meshed in parallel by BRepMesh and written out through OCCT's glTF
writer, which emits positions, exact surface normals and indices from C++ in a
single pass. Reading that binary back with numpy is orders of magnitude faster
than walking ``Poly_Triangulation`` nodes from Python. The same triangles serve
picking: each is tagged with its OCCT face id from per-face triangle counts.
Edges and free vertices are discretized directly, since glTF has no notion of
them.

Every array travels to the GUI as base64-encoded little-endian binary.
"""

from __future__ import annotations

import atexit
import base64
import contextlib
import hashlib
import io
import json
import os
import shutil
import struct
import tempfile
import threading
import time
from collections import OrderedDict
from dataclasses import dataclass
from functools import cache

import numpy as np
import OCP
from OCP.BRep import BRep_Tool
from OCP.BRepAdaptor import BRepAdaptor_Curve, BRepAdaptor_Curve2d, BRepAdaptor_Surface
from OCP.BRepBndLib import BRepBndLib
from OCP.BRepLProp import BRepLProp_SLProps
from OCP.BRepMesh import BRepMesh_IncrementalMesh
from OCP.Bnd import Bnd_Box
from OCP.BinTools import BinTools, BinTools_FormatVersion
from OCP.GCPnts import GCPnts_TangentialDeflection
from OCP.Message import Message_ProgressRange
from OCP.RWGltf import RWGltf_CafWriter
from OCP.TCollection import TCollection_AsciiString, TCollection_ExtendedString
from OCP.TDocStd import TDocStd_Document
from OCP.TopAbs import TopAbs_EDGE, TopAbs_FACE, TopAbs_REVERSED, TopAbs_VERTEX
from OCP.TopExp import TopExp, TopExp_Explorer
from OCP.TopLoc import TopLoc_Location
from OCP.TopoDS import TopoDS, TopoDS_Shape
from OCP.XCAFDoc import XCAFDoc_DocumentTool

# ``ShapeMap`` is the indexed map of shapes that ``TopExp.MapShapes_s`` fills:
# its 1-based indices are the topology ids the GUI picks. Tests use it too.
try:
    # OCP 7.x exposes generated collection aliases in their OCCT package.
    from OCP.TColStd import TColStd_IndexedDataMapOfStringString as _StringMap
    from OCP.TopTools import TopTools_IndexedMapOfShape as ShapeMap
except ImportError:
    # OCP 8 generates all collection specializations in one module.
    from OCP.collections import (
        IndexedDataMap_TCollection_AsciiString_TCollection_AsciiString as _StringMap,
    )
    from OCP.collections import (
        IndexedMap_TopoDS_Shape_TopTools_ShapeMapHasher as ShapeMap,
    )

from .environment import ocp_version

#: Bump whenever a change to this module meshes the same shape with the same
#: tolerances differently: the version is part of every geometry id.
MESH_FORMAT = 2

#: The angle of an edge with a face on one side only, or none (a wire): an
#: outline, which the GUI always draws.
OPEN_EDGE_ANGLE = 180.0

# Never mesh finer than this, whatever the model size (model units).
MIN_DEFLECTION = 1e-4
# A worker is long-lived, so cached meshes need a hard memory ceiling. The
# serialized arrays dominate the size and this limit is deliberately modest
# compared with the memory normally available to OCCT itself.
DEFAULT_CACHE_BYTES = 256 * 1024 * 1024
# Scratch files of killed workers older than this are removed.
STALE_SCRATCH_SECONDS = 24 * 3600

_SCRATCH_PREFIX = "cq-editor-mesh-"
_scratch_dir: str | None = None

_GLB_MAGIC = 0x46546C67  # "glTF"
_CHUNK_JSON = 0x4E4F534A
_CHUNK_BIN = 0x004E4942
_COMPONENT_TYPES = {
    5121: np.uint8,
    5123: np.uint16,
    5125: np.uint32,
    5126: np.float32,
}
_COMPONENT_COUNTS = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT4": 16}


class TessellationError(RuntimeError):
    pass


@dataclass(frozen=True)
class Tolerance:
    """Mesh quality settings."""

    #: Chordal deviation, relative to the shape's bounding-box diagonal.
    linear: float = 1e-3
    #: Maximum angle between adjacent mesh normals, in radians.
    angular: float = 0.2


@dataclass
class Mesh:
    positions: np.ndarray  # (n, 3) float32
    normals: np.ndarray  # (n, 3) float32
    indices: np.ndarray  # (m,) uint32, three per triangle
    edges: np.ndarray  # (k, 6) float32, one line segment per row
    points: np.ndarray  # (p, 3) float32
    # Separate pick triangles, only when the display triangles cannot be
    # matched to their faces; normally empty.
    face_positions: np.ndarray  # (n, 3) float32, grouped by OCCT face
    face_indices: np.ndarray  # (m,) uint32, three per triangle
    # 1-based OCCT face id per pick triangle: the display triangles, or the
    # separate pick triangles when ``face_indices`` is set.
    triangle_faces: np.ndarray  # (m / 3,) uint32
    segment_edges: np.ndarray  # (k,) uint32, 1-based OCCT edge ids
    # Degrees between the faces that meet at each segment's edge (see
    # ``edge_angles``), so the GUI can leave out smooth edges.
    segment_angles: np.ndarray  # (k,) float32
    topology_vertices: np.ndarray  # (v, 3) float32, in 1-based id order

    def is_empty(self) -> bool:
        return not (len(self.indices) or len(self.edges) or len(self.points))

    def bounds(self) -> tuple[list[float], list[float]]:
        coords = [
            a.reshape(-1, 3)
            for a in (self.positions, self.edges, self.points)
            if a.size
        ]
        stacked = np.concatenate(coords)
        return stacked.min(axis=0).tolist(), stacked.max(axis=0).tolist()

    def byte_size(self) -> int:
        return sum(
            array.nbytes
            for array in (
                self.positions,
                self.normals,
                self.indices,
                self.edges,
                self.points,
                self.face_positions,
                self.face_indices,
                self.triangle_faces,
                self.segment_edges,
                self.segment_angles,
                self.topology_vertices,
            )
        )

    def non_finite_array(self) -> str | None:
        """Name the first floating mesh array unsafe to put on the wire."""
        for name in (
            "positions",
            "normals",
            "edges",
            "points",
            "segment_angles",
            "face_positions",
            "topology_vertices",
        ):
            if not np.isfinite(getattr(self, name)).all():
                return name
        return None

    def to_message(self, geometry_id: str = "") -> dict:
        lo, hi = self.bounds()
        return {
            "id": geometry_id,
            "positions": _encode(self.positions, "<f4"),
            "normals": _encode(self.normals, "<f4"),
            "indices": _encode(self.indices, "<u4"),
            "edges": _encode(self.edges, "<f4"),
            "points": _encode(self.points, "<f4"),
            "segment_angles": _encode(self.segment_angles, "<f4"),
            "topology": {
                "face_positions": _encode(self.face_positions, "<f4"),
                "face_indices": _encode(self.face_indices, "<u4"),
                "triangle_faces": _encode(self.triangle_faces, "<u4"),
                "segment_edges": _encode(self.segment_edges, "<u4"),
                "vertices": _encode(self.topology_vertices, "<f4"),
            },
            "bbox": [lo, hi],
        }


@cache
def mesh_generator() -> str:
    """What makes a mesh besides the shape and the tolerances.

    This module's format and the OpenCascade bindings it runs on: their
    version and the installed binary itself, so another environment, or OCP
    reinstalled with the same version number, is another generator too.
    """

    try:
        path = os.path.abspath(getattr(OCP, "OCP", OCP).__file__)
        status = os.stat(path)
        binary = f"{os.path.normcase(path)} {status.st_size} {status.st_mtime_ns}"
    except (AttributeError, TypeError, OSError):
        binary = "unknown binary"
    version = getattr(OCP, "__version__", "unknown")
    return f"cq-editor-mesh {MESH_FORMAT}; OCP {version} ({ocp_version()}); {binary}"


class GeometryCache:
    """Content-addressed LRU cache of tessellated OCCT shapes.

    The identifier is calculated from a triangle-free BREP representation,
    the two tessellation tolerances and the :func:`mesh_generator`. It is
    stable when a script recreates the same part, unlike Python object
    identity or OCCT's in-process hash code. The GUI keeps meshes across
    worker restarts, including a switch to another Python environment, and
    the generator ensures an id never names a mesh another mesher made.
    """

    def __init__(
        self, max_bytes: int = DEFAULT_CACHE_BYTES, generator: str | None = None
    ) -> None:
        self.max_bytes = max(0, max_bytes)
        generator_bytes = (generator or mesh_generator()).encode()
        self._generator = struct.pack("<I", len(generator_bytes)) + generator_bytes
        self._meshes: OrderedDict[str, Mesh] = OrderedDict()
        self._aliases: dict[str, str] = {}
        self._bytes = 0

    def identifier(self, shape: TopoDS_Shape, tolerance: Tolerance) -> str:
        stream = io.BytesIO()
        BinTools.Write_s(
            shape,
            stream,
            False,
            False,
            BinTools_FormatVersion.BinTools_FormatVersion_CURRENT,
        )
        digest = hashlib.blake2b(digest_size=16, person=b"cq-editor-mesh")
        digest.update(self._generator)
        digest.update(struct.pack("<dd", tolerance.linear, tolerance.angular))
        digest.update(stream.getbuffer())
        return digest.hexdigest()

    def canonical_id(self, geometry_id: str) -> str:
        return self._aliases.get(geometry_id, geometry_id)

    def get(self, geometry_id: str) -> Mesh | None:
        geometry_id = self.canonical_id(geometry_id)
        mesh = self._meshes.get(geometry_id)
        if mesh is not None:
            self._meshes.move_to_end(geometry_id)
        return mesh

    def put(self, geometry_id: str, mesh: Mesh) -> None:
        if self.max_bytes == 0 or mesh.byte_size() > self.max_bytes:
            return
        previous = self._meshes.pop(geometry_id, None)
        if previous is not None:
            self._bytes -= previous.byte_size()
        self._meshes[geometry_id] = mesh
        self._bytes += mesh.byte_size()
        while self._bytes > self.max_bytes:
            removed_id, removed = self._meshes.popitem(last=False)
            self._bytes -= removed.byte_size()
            self._aliases = {
                alias: target
                for alias, target in self._aliases.items()
                if target != removed_id
            }

    def tessellate(
        self,
        shape: TopoDS_Shape,
        tolerance: Tolerance,
        topology: TopologyIndex,
        geometry_id: str | None = None,
    ) -> tuple[str, Mesh | None]:
        geometry_id = self.canonical_id(
            geometry_id or self.identifier(shape, tolerance)
        )
        cached = self.get(geometry_id)
        if cached is not None:
            return geometry_id, cached
        mesh = tessellate(shape, tolerance, topology)
        if mesh is not None:
            self.put(geometry_id, mesh)
            # OCCT changes internal shape flags while meshing. Treat the
            # resulting signature as an alias so the same live part remains
            # cacheable in later console/debug requests.
            after = self.identifier(shape, tolerance)
            if after != geometry_id:
                self._aliases[after] = geometry_id
        return geometry_id, mesh


@dataclass(frozen=True)
class TopologyIndex:
    """1-based OCCT subshape order used by picking and the live registry.

    The identifiers remain valid only while this exact registry entry is
    alive.  OCCT does not guarantee that the same number names the same
    semantic feature after the model is rebuilt or remodeled.
    """

    faces: tuple[TopoDS_Shape, ...]
    edges: tuple[TopoDS_Shape, ...]
    vertices: tuple[TopoDS_Shape, ...]

    @classmethod
    def build(cls, shape: TopoDS_Shape) -> TopologyIndex:
        def mapped(kind) -> tuple[TopoDS_Shape, ...]:
            index = ShapeMap()
            TopExp.MapShapes_s(shape, kind, index)
            return tuple(index.FindKey(i) for i in range(1, index.Extent() + 1))

        return cls(mapped(TopAbs_FACE), mapped(TopAbs_EDGE), mapped(TopAbs_VERTEX))

    def get(self, kind: str, identifier: int) -> TopoDS_Shape:
        collection = {
            "face": self.faces,
            "edge": self.edges,
            "vertex": self.vertices,
        }.get(kind)
        if collection is None:
            raise ValueError(f"Unknown topology kind {kind!r}")
        if identifier < 1 or identifier > len(collection):
            raise IndexError(f"Unknown {kind} id {identifier}")
        return collection[identifier - 1]


def tessellate(
    shape: TopoDS_Shape,
    tolerance: Tolerance,
    topology: TopologyIndex | None = None,
) -> Mesh | None:
    """Mesh ``shape``; returns ``None`` when it has no drawable geometry."""

    box = Bnd_Box()
    BRepBndLib.Add_s(shape, box, False)
    if box.IsVoid():
        return None

    diagonal = box.CornerMin().Distance(box.CornerMax())
    deflection = max(tolerance.linear * diagonal, MIN_DEFLECTION)

    topology = topology or TopologyIndex.build(shape)
    positions, normals, indices = _mesh_faces(shape, deflection, tolerance.angular)
    triangle_faces = _display_triangle_faces(shape, positions, indices)
    if triangle_faces is None:
        face_positions, face_indices, triangle_faces = _topology_faces(topology.faces)
    else:
        face_positions, face_indices = _empty(3), np.zeros(0, dtype=np.uint32)
    edges, segment_edges = _discretize_edges(
        topology.edges, deflection, tolerance.angular
    )
    segment_angles = edge_angles(topology)[segment_edges - 1]
    mesh = Mesh(
        positions=positions,
        normals=normals,
        indices=indices,
        edges=edges,
        points=_free_vertices(shape),
        face_positions=face_positions,
        face_indices=face_indices,
        triangle_faces=triangle_faces,
        segment_edges=segment_edges,
        segment_angles=segment_angles.astype(np.float32),
        topology_vertices=_topology_vertices(topology.vertices),
    )
    return None if mesh.is_empty() else mesh


def _encode(array: np.ndarray, dtype: str) -> str:
    data = np.ascontiguousarray(array, dtype=dtype).tobytes()
    return base64.b64encode(data).decode("ascii")


def _empty(columns: int) -> np.ndarray:
    return np.zeros((0, columns), dtype=np.float32)


def _topods(kind: str, shape: TopoDS_Shape) -> TopoDS_Shape:
    """Downcast a shape with the generated spelling used by OCP 7 or 8."""

    caster = getattr(TopoDS, f"{kind}_s", None) or getattr(TopoDS, kind)
    return caster(shape)


def _mesh_faces(
    shape: TopoDS_Shape, deflection: float, angular: float
) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    if not TopExp_Explorer(shape, TopAbs_FACE).More():
        return _empty(3), _empty(3), np.zeros(0, dtype=np.uint32)

    BRepMesh_IncrementalMesh(shape, deflection, False, angular, True)

    document = TDocStd_Document(TCollection_ExtendedString("XmlOcaf"))
    XCAFDoc_DocumentTool.ShapeTool_s(document.Main()).AddShape(shape, False)

    path = _scratch_path()
    try:
        writer = RWGltf_CafWriter(TCollection_AsciiString(path), True)
        writer.SetMergeFaces(True)
        ok = writer.Perform(document, _StringMap(), Message_ProgressRange())
        if not ok:
            raise TessellationError("the glTF writer failed to export the mesh")
        with open(path, "rb") as file:
            data = file.read()
    finally:
        with contextlib.suppress(FileNotFoundError):
            os.unlink(path)

    return read_glb(data)


def _scratch_path() -> str:
    """The file of this thread's glTF round trip.

    OCCT's glTF writer only writes files, so every mesh passes through one
    private directory per worker process. A worker killed while meshing (Run
    > Stop) cannot clean up after itself, so the next worker removes such
    leftovers once they are old enough to belong to no running process.
    """

    global _scratch_dir
    if _scratch_dir is None or not os.path.isdir(_scratch_dir):
        _remove_stale_scratch(tempfile.gettempdir())
        _scratch_dir = tempfile.mkdtemp(prefix=_SCRATCH_PREFIX)
        atexit.register(shutil.rmtree, _scratch_dir, True)
    return os.path.join(_scratch_dir, f"mesh-{threading.get_ident()}.glb")


def _remove_stale_scratch(directory: str, max_age: float = STALE_SCRATCH_SECONDS):
    cutoff = time.time() - max_age
    with contextlib.suppress(OSError), os.scandir(directory) as entries:
        for entry in entries:
            # Directories of this module, and the single files that releases
            # before it created with the same prefix.
            if not entry.name.startswith(_SCRATCH_PREFIX) and not (
                entry.name.startswith("cq-editor-") and entry.name.endswith(".glb")
            ):
                continue
            with contextlib.suppress(OSError):
                if entry.stat().st_mtime >= cutoff:
                    continue
                if entry.is_dir(follow_symlinks=False):
                    shutil.rmtree(entry.path, ignore_errors=True)
                else:
                    os.unlink(entry.path)


def read_glb(data: bytes) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    """Collect every triangle primitive of a binary glTF into world space."""

    magic, version, _length = struct.unpack_from("<III", data, 0)
    if magic != _GLB_MAGIC or version != 2:
        raise TessellationError("unexpected glTF header")

    document, binary = None, b""
    offset = 12
    while offset < len(data):
        chunk_length, chunk_type = struct.unpack_from("<II", data, offset)
        chunk = data[offset + 8 : offset + 8 + chunk_length]
        if chunk_type == _CHUNK_JSON:
            document = json.loads(chunk)
        elif chunk_type == _CHUNK_BIN:
            binary = chunk
        offset += 8 + chunk_length
    if document is None:
        raise TessellationError("glTF file without a JSON chunk")

    reader = _AccessorReader(document, binary)
    positions, normals, indices = [], [], []
    vertex_count = 0

    def visit(node_index: int, parent: np.ndarray) -> None:
        nonlocal vertex_count
        node = document["nodes"][node_index]
        transform = parent @ _node_matrix(node)
        if "mesh" in node:
            rotation = transform[:3, :3]
            for primitive in document["meshes"][node["mesh"]]["primitives"]:
                if primitive.get("mode", 4) != 4:  # only triangle lists
                    continue
                attributes = primitive["attributes"]
                pos = reader.read(attributes["POSITION"]).reshape(-1, 3)
                if "NORMAL" in attributes:
                    nor = reader.read(attributes["NORMAL"]).reshape(-1, 3)
                else:
                    nor = np.zeros_like(pos)
                if "indices" in primitive:
                    idx = reader.read(primitive["indices"]).astype(np.uint32)
                else:
                    idx = np.arange(len(pos), dtype=np.uint32)

                positions.append(pos @ rotation.T + transform[:3, 3])
                nor = nor @ rotation.T
                lengths = np.linalg.norm(nor, axis=1, keepdims=True)
                normals.append(nor / np.where(lengths > 0, lengths, 1))
                indices.append(idx + vertex_count)
                vertex_count += len(pos)
        for child in node.get("children", []):
            visit(child, transform)

    scene = document.get("scenes", [{}])[document.get("scene", 0)]
    for root in scene.get("nodes", []):
        visit(root, np.eye(4))

    if not positions:
        return _empty(3), _empty(3), np.zeros(0, dtype=np.uint32)
    return (
        np.concatenate(positions).astype(np.float32),
        np.concatenate(normals).astype(np.float32),
        np.concatenate(indices),
    )


class _AccessorReader:
    def __init__(self, document: dict, binary: bytes) -> None:
        self._accessors = document.get("accessors", [])
        self._views = document.get("bufferViews", [])
        self._binary = binary

    def read(self, index: int) -> np.ndarray:
        accessor = self._accessors[index]
        view = self._views[accessor["bufferView"]]
        dtype = np.dtype(_COMPONENT_TYPES[accessor["componentType"]]).newbyteorder("<")
        components = _COMPONENT_COUNTS[accessor["type"]]
        count = accessor["count"]
        start = view.get("byteOffset", 0) + accessor.get("byteOffset", 0)
        stride = view.get("byteStride", 0)
        element_size = dtype.itemsize * components

        if stride and stride != element_size:
            rows = np.frombuffer(
                self._binary,
                dtype=np.uint8,
                count=stride * (count - 1) + element_size,
                offset=start,
            )
            rows = np.lib.stride_tricks.as_strided(
                rows, shape=(count, element_size), strides=(stride, 1)
            )
            return np.ascontiguousarray(rows).view(dtype).reshape(-1)
        return np.frombuffer(
            self._binary, dtype=dtype, count=count * components, offset=start
        )


def _node_matrix(node: dict) -> np.ndarray:
    if "matrix" in node:
        # glTF matrices are column-major.
        return np.array(node["matrix"], dtype=float).reshape(4, 4).T

    matrix = np.eye(4)
    if "scale" in node:
        matrix = np.diag([*node["scale"], 1.0]) @ matrix
    if "rotation" in node:
        x, y, z, w = node["rotation"]
        rotation = np.array(
            [
                [1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
                [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
                [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)],
            ]
        )
        rotated = np.eye(4)
        rotated[:3, :3] = rotation
        matrix = rotated @ matrix
    if "translation" in node:
        translated = np.eye(4)
        translated[:3, 3] = node["translation"]
        matrix = translated @ matrix
    return matrix


def _display_triangle_faces(
    shape: TopoDS_Shape, positions: np.ndarray, indices: np.ndarray
) -> np.ndarray | None:
    """The 1-based OCCT face id of every display triangle.

    The glTF writer emits the triangles of each meshed face in explorer
    order. Validate every proposed face group against its OCCT triangulation
    bounds and a few triangle centroids: equal counts alone cannot detect an
    exporter that changed face order. This remains much cheaper than walking
    every node from Python. Returns ``None`` on any mismatch, and the caller
    extracts separate pick triangles instead of guessing.
    """

    if len(indices) % 3:
        return None
    triangles = np.asarray(indices).reshape(-1, 3)
    if triangles.size and (
        int(triangles.min()) < 0 or int(triangles.max()) >= len(positions)
    ):
        return None

    index = ShapeMap()
    TopExp.MapShapes_s(shape, TopAbs_FACE, index)
    identifiers, counts = [], []
    offset = 0
    explorer = TopExp_Explorer(shape, TopAbs_FACE)
    while explorer.More():
        face = _topods("Face", explorer.Current())
        location = TopLoc_Location()
        triangulation = BRep_Tool.Triangulation_s(face, location)
        if triangulation is not None and triangulation.NbTriangles() > 0:
            count = triangulation.NbTriangles()
            group = triangles[offset : offset + count]
            if len(group) != count or not _face_group_matches(
                triangulation, location, positions, group
            ):
                return None
            identifiers.append(index.FindIndex(face))
            counts.append(count)
            offset += count
        explorer.Next()
    faces = np.repeat(np.array(identifiers, dtype=np.uint32), counts)
    return faces if offset == len(triangles) else None


def _face_group_matches(
    triangulation,
    location: TopLoc_Location,
    positions: np.ndarray,
    triangles: np.ndarray,
) -> bool:
    """Check a glTF triangle group against OCCT geometry, not just its size."""

    points = positions[triangles]
    actual_bounds = np.concatenate((points.min(axis=(0, 1)), points.max(axis=(0, 1))))

    box = Bnd_Box()
    if not triangulation.MinMax(box, location.Transformation(), True):
        return False
    lo, hi = box.CornerMin(), box.CornerMax()
    expected_bounds = np.array(
        [lo.X(), lo.Y(), lo.Z(), hi.X(), hi.Y(), hi.Z()], dtype=np.float32
    )
    if not _f32_close(actual_bounds, expected_bounds).all():
        return False

    actual_centers = points.mean(axis=1)
    sample_indices = {
        1,
        (triangulation.NbTriangles() + 1) // 2,
        triangulation.NbTriangles(),
    }
    transform = location.Transformation()
    for triangle_index in sample_indices:
        triangle = triangulation.Triangle(triangle_index)
        expected_center = np.array(
            [
                (
                    point.X(),
                    point.Y(),
                    point.Z(),
                )
                for point in (
                    triangulation.Node(triangle.Value(corner)).Transformed(transform)
                    for corner in (1, 2, 3)
                )
            ],
            dtype=np.float32,
        ).mean(axis=0)
        if not _f32_close(actual_centers, expected_center).all(axis=1).any():
            return False
    return True


def _f32_close(actual: np.ndarray, expected: np.ndarray) -> np.ndarray:
    """Compare serialized f32 geometry within two units in the last place."""

    actual = np.asarray(actual, dtype=np.float32)
    expected = np.asarray(expected, dtype=np.float32)
    spacing = np.maximum(np.abs(np.spacing(actual)), np.abs(np.spacing(expected)))
    tolerance = np.maximum(spacing * 2, np.float32(1e-5))
    return np.abs(actual - expected) <= tolerance


def _topology_faces(
    faces: tuple[TopoDS_Shape, ...],
) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    """Return separate pick triangles tagged with their 1-based OCCT face id.

    Slow (it walks every node from Python); only a fallback for display
    triangles that :func:`_display_triangle_faces` cannot match.
    """

    positions: list[np.ndarray] = []
    indices: list[np.ndarray] = []
    identifiers: list[np.ndarray] = []
    offset = 0
    for identifier, raw_face in enumerate(faces, start=1):
        face = _topods("Face", raw_face)
        location = TopLoc_Location()
        triangulation = BRep_Tool.Triangulation_s(face, location)
        if triangulation is None:
            continue

        transform = location.Transformation()
        nodes = np.array(
            [
                (point.X(), point.Y(), point.Z())
                for point in (
                    triangulation.Node(i).Transformed(transform)
                    for i in range(1, triangulation.NbNodes() + 1)
                )
            ],
            dtype=np.float32,
        ).reshape(-1, 3)
        triangles = np.array(
            [
                [triangle.Value(i) - 1 + offset for i in (1, 2, 3)]
                for triangle in triangulation.Triangles()
            ],
            dtype=np.uint32,
        ).reshape(-1, 3)
        if face.Orientation() == TopAbs_REVERSED and len(triangles):
            triangles[:, [1, 2]] = triangles[:, [2, 1]]

        positions.append(nodes)
        indices.append(triangles.reshape(-1))
        identifiers.append(np.full(len(triangles), identifier, dtype=np.uint32))
        offset += len(nodes)

    if not positions:
        return _empty(3), np.zeros(0, dtype=np.uint32), np.zeros(0, dtype=np.uint32)
    return (
        np.concatenate(positions),
        np.concatenate(indices),
        np.concatenate(identifiers),
    )


def edge_angles(topology: TopologyIndex) -> np.ndarray:
    """The angle in degrees between the faces that meet at each edge of
    ``topology``, in id order, measured at the middle of the edge.

    0 where they meet smoothly: a fillet's border, a seam, a face split in
    two. 90 at the corner of a box. :data:`OPEN_EDGE_ANGLE` for an edge of
    one face only, or of none (a wire), and wherever a normal cannot be
    found, so that such edges are always drawn. The GUI leaves out the
    edges below an angle the user chooses.
    """

    index = ShapeMap()
    for edge in topology.edges:
        index.Add(edge)
    normals: list[list[np.ndarray | None]] = [[] for _ in topology.edges]
    for raw_face in topology.faces:
        face = _topods("Face", raw_face)
        surface = BRepAdaptor_Surface(face)
        properties = BRepLProp_SLProps(surface, 1, 1e-7)
        reversed_face = face.Orientation() == TopAbs_REVERSED
        explorer = TopExp_Explorer(face, TopAbs_EDGE)
        while explorer.More():
            identifier = index.FindIndex(explorer.Current())
            if identifier:
                edge = _topods("Edge", explorer.Current())
                normals[identifier - 1].append(
                    _normal_at_middle(edge, face, properties, reversed_face)
                )
            explorer.Next()

    angles = np.full(len(topology.edges), OPEN_EDGE_ANGLE, dtype=np.float32)
    for position, found in enumerate(normals):
        # A seam is listed twice by its one face, and is smooth.
        if len(found) < 2 or any(normal is None for normal in found):
            continue
        present = [normal for normal in found if normal is not None]
        cosines = [
            float(np.dot(a, b)) for i, a in enumerate(present) for b in present[i + 1 :]
        ]
        angles[position] = np.degrees(np.arccos(np.clip(min(cosines), -1.0, 1.0)))
    return angles


def _normal_at_middle(
    edge, face, properties: BRepLProp_SLProps, reversed_face: bool
) -> np.ndarray | None:
    """The outward normal of ``face`` at the middle of ``edge``, one of its
    edges, from the edge's curve on the face; ``None`` when it is not
    defined there (a cone's apex) or cannot be computed."""

    try:
        curve = BRepAdaptor_Curve2d(edge, face)
        uv = curve.Value(0.5 * (curve.FirstParameter() + curve.LastParameter()))
        properties.SetParameters(uv.X(), uv.Y())
        if not properties.IsNormalDefined():
            return None
        normal = properties.Normal()
    except Exception:  # no curve on the surface, or an unbounded parameter
        return None
    vector = np.array((normal.X(), normal.Y(), normal.Z()))
    return -vector if reversed_face else vector


def _discretize_edges(
    edges: tuple[TopoDS_Shape, ...], deflection: float, angular: float
) -> tuple[np.ndarray, np.ndarray]:

    segments: list[np.ndarray] = []
    identifiers: list[np.ndarray] = []
    for identifier, raw_edge in enumerate(edges, start=1):
        edge = _topods("Edge", raw_edge)
        if BRep_Tool.Degenerated_s(edge):
            continue
        try:
            sampler = GCPnts_TangentialDeflection(
                BRepAdaptor_Curve(edge), angular, deflection
            )
        except Exception:  # an edge without a 3D curve; nothing to draw
            continue
        count = sampler.NbPoints()
        if count < 2:
            continue
        polyline = np.array(
            [
                (p.X(), p.Y(), p.Z())
                for p in (sampler.Value(j) for j in range(1, count + 1))
            ],
            dtype=np.float32,
        )
        edge_segments = np.hstack((polyline[:-1], polyline[1:]))
        segments.append(edge_segments)
        identifiers.append(np.full(len(edge_segments), identifier, dtype=np.uint32))

    if not segments:
        return _empty(6), np.zeros(0, dtype=np.uint32)
    return np.concatenate(segments), np.concatenate(identifiers)


def _topology_vertices(vertices: tuple[TopoDS_Shape, ...]) -> np.ndarray:
    points = []
    for raw_vertex in vertices:
        point = BRep_Tool.Pnt_s(_topods("Vertex", raw_vertex))
        points.append((point.X(), point.Y(), point.Z()))
    return np.array(points, dtype=np.float32).reshape(-1, 3)


def _free_vertices(shape: TopoDS_Shape) -> np.ndarray:
    """Vertices that are not part of any edge (e.g. a ``.vertices()`` result)."""

    points = []
    explorer = TopExp_Explorer(shape, TopAbs_VERTEX, TopAbs_EDGE)
    while explorer.More():
        p = BRep_Tool.Pnt_s(_topods("Vertex", explorer.Current()))
        points.append((p.X(), p.Y(), p.Z()))
        explorer.Next()
    return np.array(points, dtype=np.float32).reshape(-1, 3)
