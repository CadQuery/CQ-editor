"""Compatibility contract for the supported CadQuery/OCP families."""

import os
import re
import sys

import cadquery as cq

from cq_worker.environment import runtime_environment
from cq_worker.tessellation import Tolerance, TopologyIndex, tessellate


def _release(version: str) -> tuple[int, ...]:
    return tuple(int(part) for part in re.findall(r"\d+", version))


def test_runtime_is_inside_the_supported_version_families():
    environment = runtime_environment()

    assert (3, 10) <= sys.version_info[:2] < (3, 15)
    assert (2, 7) <= _release(environment["cadquery"]) < (2, 10)
    assert (7, 8, 1) <= _release(environment["ocp"]) < (8, 1)

    expected_cadquery = os.environ.get("CQ_EDITOR_EXPECT_CADQUERY")
    expected_ocp = os.environ.get("CQ_EDITOR_EXPECT_OCP")
    if expected_cadquery:
        assert environment["cadquery"].startswith(expected_cadquery)
    if expected_ocp:
        assert environment["ocp"].startswith(expected_ocp)


def test_ocp_version_is_known_without_pip_metadata(monkeypatch):
    """Conda installs OCP without pip metadata; the bindings know their version."""

    import OCP

    from cq_worker import environment

    monkeypatch.setattr(environment, "distribution_version", lambda name: "unknown")
    assert environment.ocp_version() == OCP.__version__


def test_direct_ocp_calls_work_for_a_real_shape():
    """Exercise the generated APIs that differ between OCP 7 and OCP 8."""

    shape = cq.Workplane().box(3, 4, 5).val().wrapped
    topology = TopologyIndex.build(shape)
    mesh = tessellate(shape, Tolerance(), topology)

    assert mesh is not None
    assert len(topology.faces) == 6
    assert len(mesh.triangle_faces) == len(mesh.indices) // 3
    assert set(mesh.triangle_faces) == set(range(1, 7))
