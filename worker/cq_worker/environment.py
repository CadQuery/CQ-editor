"""Describe the Python/CadQuery/OCP environment used by the worker."""

from __future__ import annotations

import importlib.metadata
import platform
import sys
from typing import Any

import cadquery as cq


def distribution_version(name: str) -> str:
    """Return an installed distribution version without hiding bad metadata."""

    try:
        return importlib.metadata.version(name)
    except importlib.metadata.PackageNotFoundError:
        return "unknown"


def ocp_version() -> str:
    """The OCP release, also for conda installs, which carry no pip metadata."""

    version = distribution_version("cadquery-ocp")
    if version != "unknown":
        return version
    import OCP

    return str(getattr(OCP, "__version__", "unknown"))


def runtime_environment() -> dict[str, Any]:
    """Versions needed to reproduce worker tests, timings and failures."""

    return {
        "python": platform.python_version(),
        "implementation": platform.python_implementation(),
        "cadquery": cq.__version__,
        "ocp": ocp_version(),
        "platform": platform.platform(),
        "executable": sys.executable,
    }


def summary() -> str:
    environment = runtime_environment()
    return (
        f"Python {environment['python']} ({environment['implementation']}) · "
        f"CadQuery {environment['cadquery']} · OCP {environment['ocp']} · "
        f"{environment['platform']}"
    )
