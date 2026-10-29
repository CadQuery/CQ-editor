from pathlib import Path

import pytest

from cq_worker import navigation


def test_outline_lists_nested_symbols_without_running_script():
    source = """
VALUE = 4
class Fixture:
    def make(self):
        raise RuntimeError("must not run")
async def build():
    pass
"""
    assert [(s["name"], s["kind"], s["depth"]) for s in navigation.outline(source)] == [
        ("VALUE", "variable", 0),
        ("Fixture", "class", 0),
        ("make", "function", 1),
        ("build", "function", 0),
    ]


@pytest.mark.skipif(navigation.jedi is None, reason="jedi is optional")
def test_definition_and_references_cross_project_files(tmp_path: Path):
    helper = tmp_path / "helper.py"
    helper.write_text("def make_box():\n    return 1\n", encoding="utf-8")
    source = "from helper import make_box\nresult = make_box()\n"
    main = tmp_path / "main.py"
    main.write_text(source, encoding="utf-8")

    definition = navigation.navigate(
        source, str(main), str(tmp_path), 2, 13, "definition"
    )
    assert definition["available"]
    assert definition["locations"] == [
        {
            "name": "make_box",
            "kind": "function",
            "path": str(helper.resolve()),
            "line": 1,
            "column": 4,
        }
    ]
    references = navigation.navigate(
        source, str(main), str(tmp_path), 2, 13, "references"
    )
    assert {(item["path"], item["line"]) for item in references["locations"]} == {
        (str(helper.resolve()), 1),
        (None, 1),
        (None, 2),
    }


def test_invalid_cursor_and_optional_jedi(tmp_path: Path, monkeypatch):
    assert (
        navigation.navigate("x = 1", None, str(tmp_path), 5, 0, "definition")[
            "locations"
        ]
        == []
    )
    monkeypatch.setattr(navigation, "jedi", None)
    assert (
        navigation.navigate("x = 1", None, None, 1, 0, "definition")["available"]
        is False
    )
    assert navigation.outline("def f():\n    pass\n")[0]["name"] == "f"


def test_rejects_unknown_mode():
    with pytest.raises(ValueError, match="Unknown navigation mode"):
        navigation.navigate("", None, None, 1, 0, "rename")
