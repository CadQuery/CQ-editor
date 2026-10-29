"""Small binary glTF fixtures cover mesh decoding and world transforms."""

import json
import math
import struct

import numpy as np
import pytest

from cq_worker.tessellation import TessellationError, _node_matrix, read_glb


def chunk(payload, kind, padding):
    payload += padding * (-len(payload) % 4)
    return struct.pack("<II", len(payload), kind) + payload


def glb(document, binary=b""):
    json_chunk = chunk(json.dumps(document).encode(), 0x4E4F534A, b" ")
    binary_chunk = chunk(binary, 0x004E4942, b"\x00") if binary else b""
    body = json_chunk + binary_chunk
    return struct.pack("<III", 0x46546C67, 2, 12 + len(body)) + body


def test_reads_triangle_positions_normals_and_indices_in_world_space():
    positions = np.array([[0, 0, 0], [1, 0, 0], [0, 1, 0]], dtype="<f4")
    normals = np.array([[0, 0, 1]] * 3, dtype="<f4")
    indices = np.array([0, 1, 2], dtype="<u2")
    binary = positions.tobytes() + normals.tobytes() + indices.tobytes()
    document = {
        "bufferViews": [
            {"byteOffset": 0},
            {"byteOffset": positions.nbytes},
            {"byteOffset": positions.nbytes + normals.nbytes},
        ],
        "accessors": [
            {"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3"},
            {"bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC3"},
            {"bufferView": 2, "componentType": 5123, "count": 3, "type": "SCALAR"},
        ],
        "meshes": [
            {"primitives": [{"attributes": {"POSITION": 0, "NORMAL": 1}, "indices": 2}]}
        ],
        "nodes": [{"mesh": 0, "translation": [10, 20, 30]}],
        "scenes": [{"nodes": [0]}],
    }

    actual_positions, actual_normals, actual_indices = read_glb(glb(document, binary))
    assert actual_positions == pytest.approx(positions + [10, 20, 30])
    assert actual_normals == pytest.approx(normals)
    assert actual_indices.tolist() == [0, 1, 2]


@pytest.mark.parametrize("header", [(0, 2), (0x46546C67, 1)])
def test_rejects_wrong_glb_magic_or_version(header):
    with pytest.raises(TessellationError, match="header"):
        read_glb(struct.pack("<III", *header, 12))


def test_rejects_glb_without_json_chunk():
    with pytest.raises(TessellationError, match="without a JSON chunk"):
        read_glb(struct.pack("<III", 0x46546C67, 2, 12))


def test_empty_scene_returns_empty_mesh_arrays():
    positions, normals, indices = read_glb(glb({"scenes": [{"nodes": []}]}))
    assert positions.shape == normals.shape == (0, 3)
    assert indices.size == 0


def test_node_transform_applies_scale_rotation_and_translation():
    half_turn = math.sqrt(0.5)
    matrix = _node_matrix(
        {
            "scale": [2, 1, 1],
            "rotation": [0, 0, half_turn, half_turn],
            "translation": [10, 20, 0],
        }
    )
    assert (matrix @ np.array([1, 0, 0, 1])) == pytest.approx([10, 22, 0, 1])
