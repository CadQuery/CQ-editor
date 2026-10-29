"""Persistent console state and its connection to the scene."""

import cadquery as cq
import pytest

from cq_worker.console import ConsoleSession
from cq_worker.display import Displayer


def test_console_keeps_variables_between_commands():
    console = ConsoleSession()
    assert console.execute("size = 3") is False
    assert console.execute("volume = size ** 3") is False
    assert console.namespace["volume"] == 27


def test_console_deactivates_rich_output_even_when_a_command_exits():
    sent = []
    console = ConsoleSession(Displayer(sent.append))

    assert console.execute("display('during')", request_id=21) is False
    assert sent == [{"id": 21, "type": "display", "text": "'during'", "image": None}]
    console.namespace["display"]("late")
    assert len(sent) == 1

    with pytest.raises(SystemExit):
        console.execute("raise SystemExit", request_id=22)
    console.namespace["display"]("late after exit")
    assert len(sent) == 1


def test_console_reports_incomplete_multiline_statement():
    console = ConsoleSession()
    assert console.execute("for n in range(3):") is True
    assert console.execute("    total = n") is True
    assert console.execute("") is False
    assert console.namespace["total"] == 2


def test_reset_discards_user_state_and_restores_script_api():
    console = ConsoleSession()
    console.execute("custom = 42")
    console.reset()
    assert "custom" not in console.namespace
    assert console.namespace["cq"] is cq
    assert callable(console.namespace["show_object"])
    assert callable(console.namespace["debug"])


def test_update_variables_ignores_dunder_names():
    console = ConsoleSession()
    console.update_variables({"part_count": 4, "__name__": "overwritten"})
    assert console.namespace["part_count"] == 4
    assert console.namespace["__name__"] == "__console__"


def test_take_shown_returns_objects_once():
    console = ConsoleSession()
    part = cq.Workplane().box(1, 2, 3)
    console.namespace["show_object"](part, name="box", color="red")
    shown = console.take_shown()
    assert list(shown) == ["box"]
    assert shown["box"].obj is part
    assert shown["box"].options["color"] == "red"
    assert not console.take_shown()


def test_debug_api_uses_current_scene_after_take_shown():
    console = ConsoleSession()
    console.take_shown()
    console.namespace["debug"](cq.Workplane().box(1, 1, 1), name="debug-box")
    assert list(console.take_shown()) == ["debug-box"]


def test_console_names_shown_and_debugged_objects_after_their_variables():
    console = ConsoleSession()
    console.execute("plate = cq.Workplane().box(4, 4, 1)")
    console.execute("show_object(plate)")
    console.execute("probe = cq.Workplane().sphere(1)")
    console.execute("debug(probe)")
    shown = console.take_shown()
    assert list(shown) == ["plate", "probe"]
    assert shown["probe"].options == {"color": "red", "alpha": 0.2}

    # The wrapper keeps working after the scene was handed over.
    console.execute("debug(plate)")
    assert list(console.take_shown()) == ["plate"]


def test_selected_returns_what_the_gui_selected_when_called():
    console = ConsoleSession()
    assert console.execute("nothing = selected()") is False
    assert console.namespace["nothing"] == []

    face = cq.Workplane().box(2, 2, 2).faces(">Z").val()
    calls = []

    def resolve():
        calls.append(1)
        return [face]

    console.set_selection(resolve)
    assert not calls, "resolved only when the command calls selected()"
    console.execute("top = selected()[0]")
    assert console.namespace["top"] is face
    assert "picked in the 3D view" in console.namespace["selected"].__doc__


def test_selected_survives_script_variables_but_not_a_reset():
    console = ConsoleSession()
    console.update_variables({"part": 1})
    assert callable(console.namespace["selected"])
    console.reset()
    assert callable(console.namespace["selected"])
