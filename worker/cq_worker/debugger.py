"""Line-level stepping debugger for a script and its project modules.

Built on ``sys.settrace``: only frames executing the editor buffer or project
files are traced, so library code (CadQuery, OCP) runs at full speed. When it decides
to pause it calls back into the worker, which reports the paused state to the
GUI and blocks until the next command arrives.
"""

from __future__ import annotations

import enum
import os
import sys
import types
from collections.abc import Callable, Iterable, Mapping


class Action(enum.Enum):
    CONTINUE = "continue"
    STEP = "step"
    STEP_IN = "step_in"
    STEP_OUT = "step_out"
    STOP = "stop"


class DebugStop(BaseException):
    """Raised inside the script to end a debugging session early.

    Like ``KeyboardInterrupt``, it is not an ``Exception``: a script's own
    ``except Exception`` must not swallow the user's request to stop.
    """


class Debugger:
    """Runs code objects compiled from ``filename`` under the tracer.

    ``on_pause(frame)`` is invoked whenever execution stops at a line; it
    returns the next :class:`Action` and may update :attr:`breakpoints`.
    """

    def __init__(
        self,
        filename: str,
        breakpoints: Iterable[int],
        on_pause: Callable[[types.FrameType], Action],
        project_root: str | None = None,
        module_breakpoints: Mapping[str, Iterable[int]] | None = None,
    ) -> None:
        self.filename = filename
        self.project_root = (
            os.path.normcase(os.path.realpath(project_root)) if project_root else None
        )
        self.breakpoints: dict[str, set[int]] = {}
        self.set_breakpoints(breakpoints, module_breakpoints)
        self._on_pause = on_pause
        # Like the original editor, start running until the first breakpoint.
        self._action = Action.CONTINUE
        self._depth = 0
        self._paths: dict[str, str] = {}
        self._traceable: dict[str, bool] = {}

    def set_breakpoints(
        self,
        main: Iterable[int],
        modules: Mapping[str, Iterable[int]] | None = None,
    ) -> None:
        self.breakpoints = {self.filename: set(main)}
        for path, lines in (modules or {}).items():
            normalized = os.path.normcase(os.path.realpath(path))
            if self._in_project(normalized):
                self.breakpoints[normalized] = set(lines)

    def source_path(self, frame: types.FrameType) -> str:
        path = frame.f_code.co_filename
        if path not in self._paths:
            self._paths[path] = (
                self.filename
                if path == self.filename
                else os.path.normcase(os.path.realpath(path))
            )
        return self._paths[path]

    def _in_project(self, path: str) -> bool:
        if self.project_root is None:
            return False
        try:
            if os.path.commonpath((self.project_root, path)) != self.project_root:
                return False
            for prefix in (sys.prefix, sys.base_prefix, sys.exec_prefix):
                prefix = os.path.normcase(os.path.realpath(prefix))
                if os.path.commonpath((prefix, path)) == prefix:
                    return False
            return "site-packages" not in path.split(os.sep)
        except ValueError:
            return False

    def _traced(self, frame: types.FrameType) -> bool:
        path = self.source_path(frame)
        if path not in self._traceable:
            self._traceable[path] = path == self.filename or self._in_project(path)
        return self._traceable[path]

    def run(self, code: types.CodeType, namespace: dict) -> None:
        previous = sys.gettrace()
        sys.settrace(self._trace_calls)
        try:
            exec(code, namespace)
        finally:
            sys.settrace(previous)

    def _trace_calls(self, frame: types.FrameType, event: str, arg):
        if not self._traced(frame):
            return None  # don't trace library code
        return self._trace_lines

    def _trace_lines(self, frame: types.FrameType, event: str, arg):
        if event == "line" and self._should_pause(frame):
            self._pause(frame)
        return self._trace_lines

    def _should_pause(self, frame: types.FrameType) -> bool:
        if frame.f_lineno in self.breakpoints.get(self.source_path(frame), ()):
            return True
        depth = self._script_depth(frame)
        if self._action is Action.STEP_IN:
            return True
        if self._action is Action.STEP:
            return depth <= self._depth
        if self._action is Action.STEP_OUT:
            return depth < self._depth
        return False

    def _pause(self, frame: types.FrameType) -> None:
        self._action = self._on_pause(frame)
        self._depth = self._script_depth(frame)
        if self._action is Action.STOP:
            raise DebugStop

    def _script_depth(self, frame: types.FrameType | None) -> int:
        """Number of active frames that execute the script or project files."""

        depth = 0
        while frame is not None:
            if self._traced(frame):
                depth += 1
            frame = frame.f_back
        return depth
