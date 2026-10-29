"""Per-line profiling of a script run (FEAT-E2).

``sys.settrace`` follows the editor buffer and project modules line by line: the
time between two events of a frame goes to the line that was running, with
everything that line called, so a line's time is all the time it cost. Calls
from the script straight into CadQuery are also timed, per function, as the
line's *operations*. Code outside the project is not traced line by line,
which keeps the overhead to one Python call per function call.
"""

from __future__ import annotations

import linecache
import os
import sys
import time
import types
from collections import defaultdict
from typing import Any

import cadquery

#: CadQuery's own files: calls into them are operations.
_CADQUERY = os.path.dirname(os.path.abspath(cadquery.__file__)) + os.sep
#: Operations reported per line, the slowest first.
OPERATIONS_PER_LINE = 8


class LineProfiler:
    """Runs a script and measures it; :meth:`result` is the profile."""

    def __init__(self, filename: str, project_root: str | None = None) -> None:
        self.filename = filename
        self.project_root = (
            os.path.normcase(os.path.realpath(project_root)) if project_root else None
        )
        #: (file, line) -> [seconds, times it started]
        self._lines: defaultdict[tuple[str, int], list] = defaultdict(lambda: [0.0, 0])
        #: (file, line, function) -> [seconds, calls]
        self._operations: defaultdict[tuple[str, int, str], list] = defaultdict(
            lambda: [0.0, 0]
        )
        #: Script frames being run -> [file, line, since]
        self._running: dict[types.FrameType, list] = {}
        self._sources: dict[str, str] = {}
        self._paths: dict[str, str] = {}
        self._traceable: dict[str, bool] = {}

    def _source_path(self, path: str) -> str:
        if path not in self._paths:
            self._paths[path] = (
                self.filename
                if path == self.filename
                else os.path.normcase(os.path.realpath(path))
            )
        return self._paths[path]

    def _traced(self, path: str) -> bool:
        if path in self._traceable:
            return self._traceable[path]
        result = self._in_project(path)
        self._traceable[path] = result
        return result

    def _in_project(self, path: str) -> bool:
        if path == self.filename:
            return True
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

    def run(self, code: types.CodeType, namespace: dict) -> None:
        """Executes ``code`` like ``exec``, measuring it."""

        previous = sys.gettrace()
        sys.settrace(self._call)
        try:
            exec(code, namespace)
        finally:
            sys.settrace(previous)
            # Frames left without a return event (none in practice).
            now = time.perf_counter()
            for path, line, since in self._running.values():
                if line is not None:
                    self._lines[(path, line)][0] += now - since
            self._running.clear()

    def result(self) -> dict[str, Any]:
        """``lines`` (line, seconds, hits) in line order and, per line, its
        slowest ``operations`` (line, name, seconds, calls)."""

        operations: defaultdict[tuple[str, int], list] = defaultdict(list)
        for (path, line, name), (seconds, calls) in self._operations.items():
            operations[(path, line)].append(
                {
                    "file": None if path == self.filename else path,
                    "line": line,
                    "name": name,
                    "seconds": seconds,
                    "calls": calls,
                }
            )
        slowest = []
        for key in sorted(operations):
            ranked = sorted(operations[key], key=lambda op: op["seconds"], reverse=True)
            slowest.extend(ranked[:OPERATIONS_PER_LINE])
        return {
            "lines": [
                {
                    "file": None if path == self.filename else path,
                    "line": line,
                    "seconds": seconds,
                    "hits": hits,
                }
                for (path, line), (seconds, hits) in sorted(self._lines.items())
                if hits
            ],
            "operations": slowest,
            "sources": self._sources,
        }

    def _call(self, frame: types.FrameType, event: str, arg: Any):
        """The global trace function: picks the frames worth following."""

        if event != "call":
            return None
        code = frame.f_code
        path = self._source_path(code.co_filename)
        if self._traced(path):
            self._running[frame] = [path, None, time.perf_counter()]
            if path != self.filename and path not in self._sources:
                linecache.checkcache(path)
                source = "".join(linecache.getlines(path))
                if len(source) <= 2_000_000:
                    self._sources[path] = source
            return self._line
        caller = frame.f_back
        if (
            caller is not None
            and caller in self._running
            and code.co_filename.startswith(_CADQUERY)
        ):
            return self._operation(
                frame, self._running[caller][0], caller.f_lineno, code
            )
        return None

    def _line(self, frame: types.FrameType, event: str, arg: Any):
        """A script frame's trace function: time goes to the running line."""

        now = time.perf_counter()
        running = self._running.get(frame)
        if running is None:
            return self._line
        path, line, since = running
        if line is not None:
            self._lines[(path, line)][0] += now - since
        running[2] = now
        if event == "line":
            running[1] = frame.f_lineno
            self._lines[(path, frame.f_lineno)][1] += 1
        elif event == "return":
            del self._running[frame]
        return self._line

    def _operation(
        self, frame: types.FrameType, path: str, line: int, code: types.CodeType
    ):
        # Its own lines are not followed, only its return.
        frame.f_trace_lines = False
        name = code.co_qualname
        started = time.perf_counter()

        def returned(frame: types.FrameType, event: str, arg: Any):
            if event == "return":
                stats = self._operations[(path, line, name)]
                stats[0] += time.perf_counter() - started
                stats[1] += 1
            return returned

        return returned
