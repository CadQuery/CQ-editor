"""Code completion, call signatures and documentation, powered by jedi when
installed."""

from __future__ import annotations

from typing import Any

try:
    import jedi
except ImportError:  # completion is optional
    jedi = None

MAX_COMPLETIONS = 200
#: Documentation beyond this is cut: a tooltip is not a manual, and it must
#: fit above or below the line it describes.
MAX_DOCUMENTATION_CHARS = 1200
MAX_DOCUMENTATION_LINES = 14


def is_available() -> bool:
    return jedi is not None


def complete(
    source: str,
    line: int,
    column: int,
    path: str | None,
    namespace: dict[str, Any],
) -> dict:
    """Completions at ``line`` (1-based) / ``column`` (0-based, in characters).

    Right after ``(`` or ``,`` the user is typing arguments, so call
    signatures are returned instead of names.
    """

    if jedi is None:
        return {"available": False, "items": [], "signatures": []}

    interpreter = jedi.Interpreter(source, [namespace], path=path)
    current_line = source.split("\n")[line - 1] if source else ""
    before_cursor = current_line[:column].rstrip()

    signatures = [
        signature.to_string() for signature in interpreter.get_signatures(line, column)
    ]
    if before_cursor.endswith(("(", ",")):
        return {"available": True, "items": [], "signatures": signatures}

    items = [
        {
            "name": completion.name,
            "insert": completion.complete or "",
            "kind": completion.type,
        }
        for completion in interpreter.complete(line, column)[:MAX_COMPLETIONS]
        if not completion.name.startswith("__")
    ]
    return {"available": True, "items": items, "signatures": signatures}


def describe(
    source: str,
    line: int,
    column: int,
    path: str | None,
    namespace: dict[str, Any],
) -> dict:
    """Documentation of the name at ``line`` (1-based) / ``column``
    (0-based, in characters), for the editor's hover tooltip.

    Functions and classes give their call signatures and docstring, modules
    their docstring, and variables the type of what they hold. Keywords
    are left out: their help is the language reference.
    """

    empty = {"signatures": [], "text": ""}
    if jedi is None:
        return {"available": False, **empty}

    interpreter = jedi.Interpreter(source, [namespace], path=path)
    for name in interpreter.help(line, column):
        if name.type == "keyword":
            continue
        signatures = [
            signature.to_string().replace("<lambda>", name.name, 1)
            for signature in name.get_signatures()
        ]
        text = name.docstring(raw=True).strip()
        if not signatures and not text:
            types = sorted(
                {inferred.name for inferred in interpreter.infer(line, column)}
                - {name.name}
            )
            if types:
                signatures = [f"{name.name}: {' | '.join(types)}"]
        if signatures or text:
            return {"available": True, "signatures": signatures, "text": _cut(text)}
    return {"available": True, **empty}


def _cut(text: str) -> str:
    cut = "\n".join(text.splitlines()[:MAX_DOCUMENTATION_LINES])
    if len(cut) > MAX_DOCUMENTATION_CHARS:
        cut = cut[:MAX_DOCUMENTATION_CHARS].rstrip()
    if len(cut) < len(text.strip()):
        cut += " …"
    return cut
