"""Color and opacity inputs accepted by show_object."""

import math

import pytest

from cq_worker.scene import DisplayOptions, non_finite, parse_color


@pytest.mark.parametrize(
    ("color", "rgb", "alpha"),
    [
        ("#f00", (1, 0, 0), None),
        ("#11223344", (0x11 / 255, 0x22 / 255, 0x33 / 255), 0x44 / 255),
        ([0, 128, 255], (0, 128 / 255, 1), None),
        ((0.2, 0.4, 0.6, 0.8), (0.2, 0.4, 0.6), 0.8),
    ],
)
def test_supported_color_forms(color, rgb, alpha):
    actual_rgb, actual_alpha = parse_color(color)
    assert actual_rgb == pytest.approx(rgb)
    assert actual_alpha == (pytest.approx(alpha) if alpha is not None else None)


@pytest.mark.parametrize("color", ["#12", "#zzzzzz", (-1, 0, 0), (0.0, 1.1, 0.0)])
def test_invalid_color_forms_are_rejected(color):
    with pytest.raises(ValueError):
        parse_color(color)


@pytest.mark.parametrize(("input_alpha", "expected"), [(-2, 0), (0.25, 0.25), (5, 1)])
def test_display_opacity_is_clamped(input_alpha, expected):
    assert DisplayOptions.parse({"alpha": input_alpha}).alpha == expected


@pytest.mark.parametrize(
    "options",
    [
        {"alpha": math.nan},
        {"alpha": math.inf},
        {"rgba": (1, 0, 0, math.nan)},
    ],
)
def test_non_finite_opacity_is_ignored(options, caplog):
    parsed = DisplayOptions.parse(options)
    assert parsed.alpha is None
    assert "finite number" in caplog.text
    assert non_finite(parsed.to_message()) is None


def test_one_invalid_option_keeps_the_others(caplog):
    parsed = DisplayOptions.parse({"color": "#zz", "alpha": 0.5, "edgecolor": "blue"})
    assert parsed.color is None
    assert parsed.alpha == 0.5
    assert parsed.edge_color == pytest.approx((0, 0, 1), abs=1e-3)
    assert "Ignoring invalid display option" in caplog.text


def test_non_finite_numbers_are_found_where_they_are():
    assert non_finite({"parts": [{"transform": [1.0, 2.0]}]}) is None
    assert non_finite({"parts": [{"transform": [1.0, math.nan]}]}) == (
        "parts[0].transform[1]"
    )
    assert non_finite({"alpha": math.inf, "mesh": "AAAA"}) == "alpha"


def test_explicit_opacity_overrides_color_opacity():
    options = DisplayOptions.parse({"color": "#ff000080", "alpha": 0.9})
    assert options.color == pytest.approx((1, 0, 0))
    assert options.alpha == 0.9


def test_display_options_message_has_all_fields():
    assert DisplayOptions.parse(None).to_message() == {
        "color": None,
        "alpha": None,
        "edge_color": None,
        "finish": None,
    }


@pytest.mark.parametrize(
    ("material", "finish"),
    [
        ("matte plastic", "matte_plastic"),
        ("Glossy-Plastic", "glossy_plastic"),
        ("brushed_steel", "brushed_steel"),
        ("  ALUMINIUM ", "aluminum"),
        ("wood", "wood"),
        ("default", "default"),
    ],
)
def test_materials_are_named_loosely(material, finish):
    assert DisplayOptions.parse({"material": material}).finish == finish
    assert DisplayOptions.parse({"material": material}).to_message()["finish"] == finish


@pytest.mark.parametrize("material", ["unobtainium", 3, None])
def test_an_unknown_material_is_ignored_and_the_rest_applies(material, caplog):
    parsed = DisplayOptions.parse({"material": material, "color": "red"})
    assert parsed.finish is None
    assert parsed.color == pytest.approx((1, 0, 0), abs=1e-3)
    assert "material" in caplog.text
    assert parsed.to_message()["finish"] is None
