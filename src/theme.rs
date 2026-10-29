//! Look and feel: egui visuals for the light and dark themes plus the colors
//! of the editor, syntax highlighting and 3D view that go with them.

use egui::{
    Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Shadow, Stroke,
    Theme, Vec2, Visuals,
};

use crate::editor::EditorColors;
use crate::editor::highlight::SyntaxColors;
use crate::settings;
use crate::viewer::{CubeColors, ViewerColors};

/// Colors shared across the UI.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub dark: bool,
    pub window: Color32,
    pub panel: Color32,
    pub surface: Color32,
    pub border: Color32,
    pub text: Color32,
    pub text_weak: Color32,
    pub accent: Color32,
    pub error: Color32,
    pub warning: Color32,
    pub success: Color32,
}

impl Palette {
    pub const DARK: Palette = Palette {
        dark: true,
        window: Color32::from_rgb(30, 31, 34),
        panel: Color32::from_rgb(43, 45, 48),
        surface: Color32::from_rgb(30, 31, 34),
        border: Color32::from_rgb(57, 59, 64),
        text: Color32::from_rgb(223, 225, 229),
        text_weak: Color32::from_rgb(140, 143, 148),
        accent: Color32::from_rgb(53, 116, 240),
        error: Color32::from_rgb(247, 84, 100),
        warning: Color32::from_rgb(242, 197, 92),
        success: Color32::from_rgb(95, 184, 101),
    };

    pub const LIGHT: Palette = Palette {
        dark: false,
        window: Color32::from_rgb(247, 248, 250),
        panel: Color32::from_rgb(235, 236, 240),
        surface: Color32::from_rgb(255, 255, 255),
        border: Color32::from_rgb(211, 213, 219),
        text: Color32::from_rgb(31, 35, 40),
        text_weak: Color32::from_rgb(108, 112, 126),
        accent: Color32::from_rgb(53, 116, 240),
        error: Color32::from_rgb(211, 47, 47),
        warning: Color32::from_rgb(184, 134, 11),
        success: Color32::from_rgb(46, 125, 50),
    };

    pub fn of(theme: Theme) -> Palette {
        match theme {
            Theme::Dark => Palette::DARK,
            Theme::Light => Palette::LIGHT,
        }
    }

    pub fn editor(&self) -> EditorColors {
        if self.dark {
            EditorColors {
                syntax: SyntaxColors {
                    text: Color32::from_rgb(212, 212, 212),
                    keyword: Color32::from_rgb(86, 156, 214),
                    constant: Color32::from_rgb(86, 156, 214),
                    builtin: Color32::from_rgb(78, 201, 176),
                    editor_api: Color32::from_rgb(220, 220, 170),
                    self_param: Color32::from_rgb(156, 220, 254),
                    string: Color32::from_rgb(206, 145, 120),
                    number: Color32::from_rgb(181, 206, 168),
                    comment: Color32::from_rgb(106, 153, 85),
                    decorator: Color32::from_rgb(197, 134, 192),
                    function: Color32::from_rgb(220, 220, 170),
                    class: Color32::from_rgb(78, 201, 176),
                    operator: Color32::from_rgb(180, 180, 180),
                },
                gutter_text: Color32::from_rgb(98, 101, 108),
                gutter_text_current: Color32::from_rgb(170, 173, 180),
                current_line: Color32::from_rgb(38, 40, 46),
                ruler: Color32::from_rgb(55, 57, 63),
                breakpoint: Color32::from_rgb(229, 83, 83),
                debug_line: Color32::from_rgba_unmultiplied(242, 197, 92, 38),
                debug_marker: Color32::from_rgb(242, 197, 92),
                error_line: Color32::from_rgba_unmultiplied(247, 84, 100, 40),
                error_text: self.error,
                search_match: Color32::from_rgba_unmultiplied(255, 210, 60, 55),
                search_current: Color32::from_rgba_unmultiplied(255, 140, 0, 130),
            }
        } else {
            EditorColors {
                syntax: SyntaxColors {
                    text: Color32::from_rgb(31, 31, 31),
                    keyword: Color32::from_rgb(0, 0, 255),
                    constant: Color32::from_rgb(0, 0, 255),
                    builtin: Color32::from_rgb(38, 127, 153),
                    editor_api: Color32::from_rgb(121, 94, 38),
                    self_param: Color32::from_rgb(0, 16, 128),
                    string: Color32::from_rgb(163, 21, 21),
                    number: Color32::from_rgb(9, 134, 88),
                    comment: Color32::from_rgb(0, 128, 0),
                    decorator: Color32::from_rgb(175, 0, 219),
                    function: Color32::from_rgb(121, 94, 38),
                    class: Color32::from_rgb(38, 127, 153),
                    operator: Color32::from_rgb(70, 70, 70),
                },
                gutter_text: Color32::from_rgb(160, 164, 171),
                gutter_text_current: Color32::from_rgb(60, 64, 72),
                current_line: Color32::from_rgb(242, 245, 250),
                ruler: Color32::from_rgb(226, 228, 233),
                breakpoint: Color32::from_rgb(224, 62, 62),
                debug_line: Color32::from_rgba_unmultiplied(255, 200, 0, 60),
                debug_marker: Color32::from_rgb(232, 164, 0),
                error_line: Color32::from_rgba_unmultiplied(230, 60, 60, 36),
                error_text: self.error,
                search_match: Color32::from_rgba_unmultiplied(255, 220, 0, 100),
                search_current: Color32::from_rgba_unmultiplied(255, 150, 0, 160),
            }
        }
    }

    pub fn viewer(&self) -> ViewerColors {
        let cube = if self.dark {
            CubeColors {
                face: Color32::from_rgb(205, 208, 214),
                chamfer: Color32::from_rgb(170, 174, 182),
                outline: Color32::from_rgb(70, 73, 80),
                text: Color32::from_rgb(40, 42, 46),
                highlight: Color32::from_rgb(90, 160, 255),
            }
        } else {
            CubeColors {
                face: Color32::from_rgb(222, 224, 229),
                chamfer: Color32::from_rgb(190, 193, 200),
                outline: Color32::from_rgb(110, 113, 120),
                text: Color32::from_rgb(35, 37, 41),
                highlight: Color32::from_rgb(70, 140, 255),
            }
        };
        ViewerColors {
            selection: Color32::from_rgb(90, 170, 255),
            hover: Color32::from_rgb(140, 195, 255),
            overlay: Color32::from_rgb(255, 92, 54),
            grid: Color32::from_rgb(178, 178, 178),
            cube,
            overlay_text: Color32::from_rgb(240, 240, 242),
            overlay_background: Color32::from_rgba_unmultiplied(22, 23, 26, 210),
        }
    }
}

/// Font family of headings.
const SEMIBOLD: &str = "semibold";

/// Inter for the interface, JetBrains Mono for code, Phosphor for icons;
/// egui's built-in fonts remain as fallbacks for other characters.
pub fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    for (name, bytes) in [
        (
            "inter",
            &include_bytes!("../assets/fonts/Inter-Regular.ttf")[..],
        ),
        (
            "inter-semibold",
            &include_bytes!("../assets/fonts/Inter-SemiBold.ttf")[..],
        ),
        (
            "jetbrains-mono",
            &include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf")[..],
        ),
    ] {
        fonts.font_data.insert(
            name.to_owned(),
            std::sync::Arc::new(FontData::from_static(bytes)),
        );
    }
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "inter".to_owned());
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .insert(0, "jetbrains-mono".to_owned());
    // After the text fonts, so icons are found without shadowing letters.
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    // Cloned only now, so headings (window titles) have the icons too.
    let mut semibold = fonts.families[&FontFamily::Proportional].clone();
    semibold.insert(0, "inter-semibold".to_owned());
    fonts
        .families
        .insert(FontFamily::Name(SEMIBOLD.into()), semibold);
    fonts
}

/// Installs the light and dark visuals and selects the preferred theme.
pub fn apply(ctx: &egui::Context, theme: settings::Theme) {
    for egui_theme in [Theme::Light, Theme::Dark] {
        ctx.set_visuals_of(egui_theme, visuals(&Palette::of(egui_theme)));
    }
    ctx.all_styles_mut(|style| {
        use egui::TextStyle;
        style.text_styles = [
            (TextStyle::Small, FontId::proportional(10.5)),
            (TextStyle::Body, FontId::proportional(13.0)),
            (TextStyle::Button, FontId::proportional(13.0)),
            (TextStyle::Monospace, FontId::monospace(12.5)),
            (
                TextStyle::Heading,
                FontId::new(17.0, FontFamily::Name(SEMIBOLD.into())),
            ),
        ]
        .into();
        let spacing = &mut style.spacing;
        spacing.item_spacing = Vec2::new(8.0, 5.0);
        spacing.button_padding = Vec2::new(7.0, 3.0);
        spacing.menu_margin = Margin::same(6);
        spacing.window_margin = Margin::same(12);
        spacing.interact_size.y = 22.0;
        spacing.scroll = egui::style::ScrollStyle::thin();
    });
    ctx.set_theme(match theme {
        settings::Theme::System => egui::ThemePreference::System,
        settings::Theme::Light => egui::ThemePreference::Light,
        settings::Theme::Dark => egui::ThemePreference::Dark,
    });
}

fn visuals(p: &Palette) -> Visuals {
    let mut v = if p.dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    let radius = CornerRadius::same(5);

    v.override_text_color = Some(p.text);
    v.weak_text_color = Some(p.text_weak);
    v.hyperlink_color = p.accent;
    v.window_fill = p.panel;
    v.panel_fill = p.window;
    v.faint_bg_color = if p.dark {
        Color32::from_rgb(36, 38, 41)
    } else {
        Color32::from_rgb(241, 242, 245)
    };
    v.extreme_bg_color = p.surface;
    v.code_bg_color = if p.dark {
        Color32::from_rgb(38, 40, 44)
    } else {
        Color32::from_rgb(238, 240, 244)
    };
    v.error_fg_color = p.error;
    v.warn_fg_color = p.warning;
    v.window_corner_radius = CornerRadius::same(8);
    v.menu_corner_radius = CornerRadius::same(6);
    v.window_stroke = Stroke::new(1.0, p.border);
    let shadow_alpha = if p.dark { 110 } else { 40 };
    v.window_shadow = Shadow {
        offset: [0, 6],
        blur: 18,
        spread: 0,
        color: Color32::from_black_alpha(shadow_alpha),
    };
    v.popup_shadow = Shadow {
        offset: [0, 4],
        blur: 12,
        spread: 0,
        color: Color32::from_black_alpha(shadow_alpha),
    };
    v.selection.bg_fill = p.accent.gamma_multiply(if p.dark { 0.55 } else { 0.35 });
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.striped = true;
    v.indent_has_left_vline = false;
    v.collapsing_header_frame = false;

    let hover_fill = if p.dark {
        Color32::from_rgb(58, 61, 66)
    } else {
        Color32::from_rgb(223, 225, 231)
    };
    let idle_fill = if p.dark {
        Color32::from_rgb(49, 51, 56)
    } else {
        Color32::from_rgb(230, 232, 236)
    };

    let widgets = &mut v.widgets;
    widgets.noninteractive.bg_fill = p.panel;
    widgets.noninteractive.weak_bg_fill = p.panel;
    widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    widgets.noninteractive.corner_radius = radius;

    widgets.inactive.bg_fill = idle_fill;
    widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    widgets.inactive.bg_stroke = Stroke::NONE;
    widgets.inactive.fg_stroke = Stroke::new(1.0, p.text);
    widgets.inactive.corner_radius = radius;

    widgets.hovered.bg_fill = hover_fill;
    widgets.hovered.weak_bg_fill = hover_fill;
    widgets.hovered.bg_stroke = Stroke::NONE;
    widgets.hovered.fg_stroke = Stroke::new(1.5, p.text);
    widgets.hovered.corner_radius = radius;
    widgets.hovered.expansion = 0.0;

    widgets.active.bg_fill = p.accent.gamma_multiply(0.6);
    widgets.active.weak_bg_fill = p.accent.gamma_multiply(0.45);
    widgets.active.bg_stroke = Stroke::NONE;
    widgets.active.fg_stroke = Stroke::new(1.5, p.text);
    widgets.active.corner_radius = radius;
    widgets.active.expansion = 0.0;

    widgets.open.bg_fill = hover_fill;
    widgets.open.weak_bg_fill = hover_fill;
    widgets.open.bg_stroke = Stroke::NONE;
    widgets.open.fg_stroke = Stroke::new(1.0, p.text);
    widgets.open.corner_radius = radius;
    v
}

/// The dock area style matching the palette.
pub fn dock_style(style: &egui::Style, p: &Palette) -> egui_dock::Style {
    let mut dock = egui_dock::Style::from_egui(style);
    dock.main_surface_border_stroke = Stroke::NONE;
    dock.separator.width = 3.0;
    dock.separator.extra_interact_width = 4.0;
    dock.separator.color_idle = p.window;
    dock.separator.color_hovered = p.accent.gamma_multiply(0.7);
    dock.separator.color_dragged = p.accent;

    dock.tab_bar.bg_fill = p.window;
    dock.tab_bar.hline_color = p.border;
    dock.tab_bar.height = 30.0;
    dock.tab_bar.corner_radius = CornerRadius::ZERO;

    dock.tab.tab_body.bg_fill = p.surface;
    dock.tab.tab_body.stroke = Stroke::new(1.0, p.border);
    dock.tab.tab_body.corner_radius = CornerRadius::same(0);
    dock.tab.tab_body.inner_margin = Margin::same(0);
    dock.tab.hline_below_active_tab_name = true;
    let corner = CornerRadius {
        nw: 6,
        ne: 6,
        sw: 0,
        se: 0,
    };
    for interaction in [
        &mut dock.tab.active,
        &mut dock.tab.focused,
        &mut dock.tab.active_with_kb_focus,
        &mut dock.tab.focused_with_kb_focus,
    ] {
        interaction.bg_fill = p.surface;
        interaction.text_color = p.text;
        interaction.outline_color = p.border;
        interaction.corner_radius = corner;
    }
    for interaction in [&mut dock.tab.inactive, &mut dock.tab.inactive_with_kb_focus] {
        interaction.bg_fill = p.window;
        interaction.text_color = p.text_weak;
        interaction.outline_color = Color32::TRANSPARENT;
        interaction.corner_radius = corner;
    }
    dock.tab.hovered.bg_fill = if p.dark {
        Color32::from_rgb(40, 42, 46)
    } else {
        Color32::from_rgb(240, 241, 244)
    };
    dock.tab.hovered.text_color = p.text;
    dock.tab.hovered.corner_radius = corner;
    dock.overlay.selection_color = p.accent.gamma_multiply(0.35);
    dock
}

#[cfg(test)]
#[path = "../tests/rust/app/test_theme.rs"]
mod tests;
