"""Compile and run CadQuery scripts the way CQ-editor always has.

Scripts run as a fresh ``__cq_main__`` module with a small API injected into
their namespace:

``show_object(obj, name=None, options=None, **options)``
    Display ``obj``; options are ``color``, ``alpha`` (opacity), ``rgba`` and
    ``edgecolor``.
``debug(obj, name=None)``
    Display ``obj`` as a translucent red overlay.
``rand_color(alpha=1.0, cfloat=False)``
    A random dark color for ``show_object`` options.
``log(value)``
    Write ``value`` to the GUI log.
``cq``
    The ``cadquery`` module.
"""

from __future__ import annotations

import contextlib
import importlib
import linecache
import logging
import os
import random
import sys
import sysconfig
import traceback
import types
from collections.abc import Callable, Iterator
from dataclasses import dataclass
from inspect import currentframe
from typing import Any, cast

import cadquery as cq

from .assembly_checks import assembly_diagnostics, require_constrained_assembly
from .cad_checks import CheckSuite
from .engineering import (
    BeamSection,
    EngineeringMaterial,
    beam_check,
    require_beam,
    require_tool_access,
    tool_access,
)
from . import provenance
from . import diagnostics
from .debugger import DebugStop
from .fit import cylindrical_fit, require_fit
from .motion import require_clear_path, trace_path
from .scene import ShownObject

#: Pseudo file name of the editor buffer in code objects and tracebacks.
SCRIPT_FILENAME = "<cq-editor>"

#: Seed used before every run, so that ``rand_color`` is reproducible.
RANDOM_SEED = 59798267586177

#: Longest value text sent for the variables panel, whose detail view shows
#: it in full.
MAX_VALUE_LENGTH = 2000

script_log = logging.getLogger("script")


@dataclass
class RunSettings:
    reload_cq: bool = False
    add_script_dir_to_path: bool = True
    change_working_dir: bool = True
    reload_imported_modules: bool = True

    @classmethod
    def from_message(cls, message: dict | None) -> RunSettings:
        message = message or {}
        return cls(
            **{
                name: bool(message[name])
                for name in cls.__dataclass_fields__
                if name in message
            }
        )


def rand_color(alpha: float = 1.0, cfloat: bool = False):
    """Random, not-too-bright color for ``show_object`` options."""

    lower, upper = 10, 100
    channels = [random.randrange(lower, upper) for _ in range(3)]
    if cfloat:
        return (*(c / 255 for c in channels), alpha)
    return {"alpha": alpha, "color": tuple(channels)}


class ShownObjects(dict[str, ShownObject]):
    """Objects passed to ``show_object``, in call order, keyed by name."""

    def __init__(self) -> None:
        super().__init__()
        self._warned_duplicates: set[str] = set()

    def clear(self) -> None:
        super().clear()
        self._warned_duplicates.clear()

    def show(self, obj: Any, name: str | None = None, options=None, **kwargs) -> None:
        merged = {**(options or {}), **kwargs}
        frame = currentframe()
        assert frame is not None and frame.f_back is not None
        caller = frame.f_back.f_back
        self._store(obj, name, merged, caller)

    def debug(self, obj: Any, name: str | None = None) -> None:
        frame = currentframe()
        assert frame is not None and frame.f_back is not None
        caller = frame.f_back.f_back
        self._store(obj, name, {"color": "red", "alpha": 0.2}, caller)

    def _store(
        self,
        obj: Any,
        name: str | None,
        options: dict[str, Any],
        caller: types.FrameType | None,
    ) -> None:
        if not name:
            name = _variable_name(obj, caller) or self._unique_name(obj)
        key = str(name)
        if key in self and key not in self._warned_duplicates:
            self._warned_duplicates.add(key)
            script_log.warning(
                'The name "%s" was used more than once in show_object(); '
                "an earlier object with this name was replaced. "
                "Use distinct names to display objects separately.",
                key,
            )
        self[key] = ShownObject(obj, options, _script_line(caller))

    def _unique_name(self, obj: Any) -> str:
        base = type(obj).__name__
        index = 1
        while f"{base} {index}" in self:
            index += 1
        return f"{base} {index}"


def _script_line(frame) -> int | None:
    """The line of ``frame`` when it runs the editor buffer."""

    if frame is not None and frame.f_code.co_filename == SCRIPT_FILENAME:
        return frame.f_lineno
    return None


def _variable_name(obj: Any, frame) -> str | None:
    """The name ``obj`` is bound to in the calling frame, if any."""

    if frame is None:
        return None
    for name, value in frame.f_locals.items():
        if value is obj and not name.startswith("_"):
            return name
    return None


def script_api(
    shown: ShownObjects,
    allowances: list[tuple[str, str, str]] | None = None,
    checks: CheckSuite | None = None,
) -> dict[str, Any]:
    """Names injected into the script namespace."""

    def show_object(obj, name=None, options=None, **kwargs):
        shown.show(obj, name, options, **kwargs)

    def debug(obj, name=None):
        shown.debug(obj, name)

    def expect(obj, name=None):
        frame = currentframe()
        assert frame is not None
        suite = checks if checks is not None else CheckSuite()
        return suite.expect(obj, name, _script_line(frame.f_back))

    def allow_interference(first: str, second: str, *, reason: str) -> None:
        if not all(
            isinstance(value, str) and value.strip()
            for value in (first, second, reason)
        ):
            raise ValueError(
                "An interference allowance needs two part names and a reason"
            )
        first, second, reason = first.strip(), second.strip(), reason.strip()
        if first == second:
            raise ValueError("An interference allowance needs two distinct part names")
        if allowances is None:
            raise RuntimeError("Declare interference allowances in the model script")
        allowances.append((first, second, reason))

    return {
        "show_object": show_object,
        "debug": debug,
        "expect": expect,
        "rand_color": rand_color,
        "cylindrical_fit": cylindrical_fit,
        "require_fit": require_fit,
        "trace_path": trace_path,
        "require_clear_path": require_clear_path,
        "assembly_diagnostics": assembly_diagnostics,
        "require_constrained_assembly": require_constrained_assembly,
        "BeamSection": BeamSection,
        "EngineeringMaterial": EngineeringMaterial,
        "beam_check": beam_check,
        "require_beam": require_beam,
        "tool_access": tool_access,
        "require_tool_access": require_tool_access,
        "allow_interference": allow_interference,
        "log": lambda value: script_log.info(str(value)),
        "cq": cq,
    }


def compile_script(source: str) -> types.CodeType:
    # Registering the source lets tracebacks show the offending lines.
    lines = source.splitlines(keepends=True)
    linecache.cache[SCRIPT_FILENAME] = (len(source), None, lines, SCRIPT_FILENAME)
    return compile(source, SCRIPT_FILENAME, "exec")


def new_module(path: str | None) -> types.ModuleType:
    module = types.ModuleType("__cq_main__")
    if path:
        module.__file__ = path
    return module


@contextlib.contextmanager
def script_environment(path: str | None, settings: RunSettings) -> Iterator[None]:
    """Temporarily adjust ``sys.path``, the working directory and the module
    cache according to ``settings``."""

    script_dir = os.path.dirname(os.path.abspath(path)) if path else None
    with contextlib.ExitStack() as stack:
        if script_dir and os.path.isdir(script_dir):
            if settings.add_script_dir_to_path:
                sys.path.insert(0, script_dir)
                stack.callback(_remove_path, script_dir)
            if settings.change_working_dir:
                stack.enter_context(contextlib.chdir(script_dir))
        if settings.reload_imported_modules:
            stack.enter_context(unload_new_user_modules())
        yield


def _remove_path(entry: str) -> None:
    with contextlib.suppress(ValueError):
        sys.path.remove(entry)


@contextlib.contextmanager
def unload_new_user_modules() -> Iterator[None]:
    """Forget modules imported while the context is active, so that edits to
    a script's helper modules are picked up by the next run.

    Library modules (standard library and site-packages) stay loaded:
    re-importing C extensions such as numpy is not safe.
    """

    before = set(sys.modules)
    try:
        yield
    finally:
        for name in set(sys.modules) - before:
            module = sys.modules.get(name)
            if module is not None and not _is_library_module(module):
                del sys.modules[name]


_LIBRARY_ROOTS = tuple(
    os.path.normcase(os.path.abspath(p))
    for p in {
        sys.prefix,
        sys.base_prefix,
        sys.exec_prefix,
        *(
            sysconfig.get_paths().get(k, "")
            for k in ("stdlib", "platstdlib", "purelib", "platlib")
        ),
    }
    if p
)


def _is_library_module(module: types.ModuleType) -> bool:
    path = getattr(module, "__file__", None)
    if not path:
        return True  # built-in or namespace package
    path = os.path.normcase(os.path.abspath(path))
    return path.startswith(_LIBRARY_ROOTS)


def reload_cadquery() -> None:
    """Re-import CadQuery itself, for people developing CadQuery."""

    # Order matters: dependencies first.
    for name in (
        "cadquery.types",
        "cadquery.occ_impl.geom",
        "cadquery.occ_impl.shapes",
        "cadquery.occ_impl.importers.dxf",
        "cadquery.occ_impl.importers",
        "cadquery.occ_impl.solver",
        "cadquery.occ_impl.assembly",
        "cadquery.occ_impl.sketch_solver",
        "cadquery.hull",
        "cadquery.selectors",
        "cadquery.sketch",
        "cadquery.occ_impl.exporters.svg",
        "cadquery.cq",
        "cadquery.occ_impl.exporters.dxf",
        "cadquery.occ_impl.exporters.amf",
        "cadquery.occ_impl.exporters.json",
        "cadquery.occ_impl.exporters",
        "cadquery.assembly",
        "cadquery",
    ):
        module = sys.modules.get(name)
        if module is not None:
            importlib.reload(module)


def describe_variables(namespace: dict[str, Any], hidden: set[str]) -> list[dict]:
    """Rows for the variables panel."""

    rows = []
    for name, value in namespace.items():
        if name.startswith("_") or name in hidden:
            continue
        try:
            text = str(value)
        except Exception as error:  # a broken __str__ must not break the run
            text = f"<unprintable: {type(error).__name__}>"
        if len(text) > MAX_VALUE_LENGTH:
            text = text[: MAX_VALUE_LENGTH - 1] + "…"
        rows.append({"name": name, "type": type(value).__name__, "value": text})
    return rows


def describe_exception(error: BaseException, source: str = "") -> dict:
    """Traceback summary for the GUI, starting at the first script frame."""

    frames = traceback.extract_tb(error.__traceback__)
    while frames and frames[0].filename != SCRIPT_FILENAME:
        frames.pop(0)

    rows = [
        {
            "file": frame.filename,
            "line": frame.lineno,
            "function": frame.name,
            "code": (frame.line or "").strip(),
        }
        for frame in frames
    ]
    if isinstance(error, SyntaxError):
        rows.append(
            {
                "file": error.filename or SCRIPT_FILENAME,
                "line": error.lineno or 0,
                "function": "",
                "code": (error.text or "").strip(),
            }
        )

    script_lines = [
        cast(int, row["line"]) for row in rows if row["file"] == SCRIPT_FILENAME
    ]
    return {
        "type_name": type(error).__name__,
        "message": str(error),
        "frames": rows,
        "line": script_lines[-1] if script_lines else None,
        "text": "".join(traceback.format_exception(error)),
        "diagnostic": diagnostics.diagnose(
            error, source, script_lines[-1] if script_lines else None
        ),
    }


class Execution:
    """One run of the editor buffer."""

    def __init__(self, source: str, path: str | None, settings: RunSettings) -> None:
        self.source = source
        self.path = path
        self.settings = settings
        self.module = new_module(path)
        self.shown = ShownObjects()
        self.interference_allowances: list[tuple[str, str, str]] = []
        self.checks = CheckSuite()
        self.api = script_api(self.shown, self.interference_allowances, self.checks)
        self.error: dict | None = None
        #: Set when a debugging session was ended by the user.
        self.stopped = False
        #: Set when Stop interrupted the script (``KeyboardInterrupt``);
        #: :attr:`error` then says where.
        self.interrupted = False

    @property
    def namespace(self) -> dict[str, Any]:
        return self.module.__dict__

    def run(
        self, executor: Callable[[types.CodeType, dict], None] | None = None
    ) -> bool:
        """Execute the script; returns ``False`` (with :attr:`error` set) on failure.

        ``executor`` replaces the plain ``exec`` call, which is how the
        debugger hooks in.
        """

        random.seed(RANDOM_SEED)
        # Helper modules may have been created or edited since the last run.
        importlib.invalidate_caches()
        if self.settings.reload_cq:
            reload_cadquery()

        try:
            code = compile_script(self.source)
        except SyntaxError as error:
            self.error = describe_exception(error, self.source)
            return False

        self.namespace.update(self.api)
        try:
            with script_environment(self.path, self.settings):
                with provenance.recording(SCRIPT_FILENAME):
                    (executor or _exec)(code, self.namespace)
        except DebugStop:
            self.stopped = True
        except SystemExit:
            pass  # a script calling sys.exit() simply ends early
        except KeyboardInterrupt as error:
            self.interrupted = True
            self.error = describe_exception(error, self.source)
            return False
        except Exception as error:
            # Lets the console's %debug-style post mortem tools find it.
            sys.last_traceback = error.__traceback__
            self.error = describe_exception(error, self.source)
            return False
        finally:
            self._remove_api()
        return True

    def hidden_names(self) -> set[str]:
        return set(self.api) - {"cq"}

    def _remove_api(self) -> None:
        for name, value in self.api.items():
            if name != "cq" and self.namespace.get(name) is value:
                del self.namespace[name]


def _exec(code: types.CodeType, namespace: dict) -> None:
    exec(code, namespace)
