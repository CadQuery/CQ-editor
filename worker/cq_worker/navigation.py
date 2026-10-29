"""Static navigation of the editor's unsaved Python source.

Jedi reads source and project files but never executes the CAD script.  The
outline uses the standard library so it remains available without Jedi.
"""

from __future__ import annotations

import ast
from pathlib import Path

try:
    import jedi
except ImportError:  # optional editor service
    jedi = None

MAX_LOCATIONS = 200
MAX_SYMBOLS = 500


def outline(source: str) -> list[dict]:
    try:
        tree = ast.parse(source)
    except SyntaxError:
        return []
    symbols: list[dict] = []

    def visit(body: list[ast.stmt], depth: int = 0) -> None:
        for node in body:
            if len(symbols) >= MAX_SYMBOLS:
                return
            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
                kind = "class" if isinstance(node, ast.ClassDef) else "function"
                symbols.append(
                    {
                        "name": node.name,
                        "kind": kind,
                        "line": node.lineno,
                        "depth": depth,
                    }
                )
                visit(node.body, depth + 1)
            elif depth == 0 and isinstance(node, (ast.Assign, ast.AnnAssign)):
                targets = (
                    node.targets if isinstance(node, ast.Assign) else [node.target]
                )
                for target in targets:
                    if isinstance(target, ast.Name) and len(symbols) < MAX_SYMBOLS:
                        symbols.append(
                            {
                                "name": target.id,
                                "kind": "variable",
                                "line": node.lineno,
                                "depth": 0,
                            }
                        )

    visit(tree.body)
    return symbols


def navigate(
    source: str,
    path: str | None,
    project_root: str | None,
    line: int,
    column: int,
    mode: str,
) -> dict:
    if mode not in {"definition", "references"}:
        raise ValueError("Unknown navigation mode")
    if jedi is None:
        return {"available": False, "locations": [], "truncated": False}
    lines = source.splitlines()
    if line < 1 or line > len(lines) or column < 0 or column > len(lines[line - 1]):
        return {"available": True, "locations": [], "truncated": False}

    current = Path(path).resolve() if path else None
    root = Path(project_root).resolve() if project_root else None
    # A stable path is needed for relative imports and references.  This
    # synthetic name is never returned as a file to open.
    script_path = current or (root or Path.cwd()) / "__cq_untitled__.py"
    project = jedi.Project(str(root or script_path.parent))
    script = jedi.Script(source, path=script_path, project=project)
    if mode == "definition":
        names = script.goto(line, column, follow_imports=True)
    else:
        names = script.get_references(line, column, include_builtins=False)

    locations: list[dict] = []
    seen: set[tuple[str | None, int, int]] = set()
    truncated = False
    for name in names:
        module = getattr(name, "module_path", None)
        if module is None or not name.line:
            continue
        resolved = Path(module).resolve()
        if mode == "references" and root and not resolved.is_relative_to(root):
            continue
        if resolved != script_path and (
            not resolved.is_file() or resolved.suffix != ".py"
        ):
            continue
        target = None if resolved == script_path else str(resolved)
        location = (target, name.line, name.column or 0)
        if location in seen:
            continue
        seen.add(location)
        if len(locations) == MAX_LOCATIONS:
            truncated = True
            break
        locations.append(
            {
                "name": name.name,
                "kind": name.type,
                "path": target,
                "line": name.line,
                "column": name.column or 0,
            }
        )
    return {"available": True, "locations": locations, "truncated": truncated}
