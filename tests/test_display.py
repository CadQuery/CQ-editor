import pytest

from cq_editor.display import DisplayMode, GlobalMode, effective_mode


def test_hidden_survives_every_global_mode():
    for glob in GlobalMode:
        assert effective_mode(DisplayMode.HIDDEN, glob) is DisplayMode.HIDDEN


def test_as_set_passes_the_item_mode_through():
    for item in DisplayMode:
        assert effective_mode(item, GlobalMode.AS_SET) is item


@pytest.mark.parametrize(
    "glob, expected",
    [
        (GlobalMode.WIREFRAME, DisplayMode.WIREFRAME),
        (GlobalMode.TRANSPARENT, DisplayMode.TRANSPARENT),
        (GlobalMode.SHADED, DisplayMode.SHADED),
    ],
)
def test_override_replaces_the_mode_of_every_visible_item(glob, expected):
    for item in (DisplayMode.WIREFRAME, DisplayMode.TRANSPARENT, DisplayMode.SHADED):
        assert effective_mode(item, glob) is expected


def test_values_are_the_ui_strings():
    assert [m.value for m in DisplayMode] == [
        "Hidden",
        "Wireframe",
        "Transparent",
        "Shaded",
    ]
    assert GlobalMode.AS_SET.value == "As set below"
