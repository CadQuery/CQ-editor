//! The toolbar below the menu, with the same groups as the original:
//! file, objects, run/debug, inspection and view commands.

use egui::{Color32, Pos2, Rect, Response, Sense, Shape, Stroke, Vec2};
use egui_phosphor::regular as icon;
use glam::Vec3;

use crate::app::AppState;
use crate::commands::{self, Action};
use crate::viewer::camera::StandardView;
use crate::viewer::{DisplayMode, SelectionMode};
use crate::worker::protocol::DebugAction;

const BUTTON_SIZE: Vec2 = Vec2::new(28.0, 26.0);

/// Space between buttons, in points.
const SPACING: f32 = 2.0;
/// A separator with the space around it.
const SEPARATOR_WIDTH: f32 = 17.0;
/// "Select:" and its drop-down.
const SELECTION_WIDTH: f32 = 125.0;

/// The toolbar's groups, in the original's order. Those that do not fit the
/// window go, from the last one, to the » menu at its end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Group {
    File,
    Objects,
    Run,
    Inspect,
    View,
    Display,
    Selection,
}

impl Group {
    const ALL: [Group; 7] = [
        Group::File,
        Group::Objects,
        Group::Run,
        Group::Inspect,
        Group::View,
        Group::Display,
        Group::Selection,
    ];

    /// Width of the group in the toolbar, in points.
    fn width(self) -> f32 {
        let buttons = match self {
            Group::File => 5,
            Group::Objects => 2,
            Group::Run => 8,
            Group::Inspect => 1,
            // Fit, the standard views and the named views.
            Group::View => StandardView::ALL.len() + 2,
            // The display modes, section, exploded view and screenshot.
            Group::Display => DisplayMode::ALL.len() + 3,
            Group::Selection => return SELECTION_WIDTH,
        };
        buttons as f32 * (BUTTON_SIZE.x + SPACING)
    }
}

/// How many groups, from the first, fit in `width`. When some do not, the
/// » button takes room too.
fn fitting_groups(width: f32) -> usize {
    let widths: Vec<f32> = Group::ALL
        .iter()
        .enumerate()
        .map(|(index, group)| group.width() + if index > 0 { SEPARATOR_WIDTH } else { 0.0 })
        .collect();
    if widths.iter().sum::<f32>() <= width {
        return Group::ALL.len();
    }
    let budget = width - SEPARATOR_WIDTH - BUTTON_SIZE.x - SPACING;
    let mut used = 0.0;
    widths
        .iter()
        .take_while(|group| {
            used += *group;
            used <= budget
        })
        .count()
}

pub fn ui(ui: &mut egui::Ui, state: &mut AppState) {
    let mut actions = Vec::new();
    let shown = fitting_groups(ui.available_width());

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = SPACING;
        for (index, group) in Group::ALL[..shown].iter().enumerate() {
            if index > 0 {
                separator(ui);
            }
            group_ui(ui, *group, state, &mut actions, false);
        }
        if shown < Group::ALL.len() {
            if shown > 0 {
                separator(ui);
            }
            let more = icon_button(ui, icon::CARET_DOUBLE_RIGHT, true, false)
                .on_hover_text("More tools: the window is too narrow for them all");
            egui::Popup::menu(&more)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    for group in &Group::ALL[shown..] {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = SPACING;
                            group_ui(ui, *group, state, &mut actions, true);
                        });
                    }
                });
        }
    });

    state.queue(actions);
}

/// The buttons of `group`. In the » menu (`overflow`), the controls that
/// open popups of their own become submenus and plain buttons.
fn group_ui(
    ui: &mut egui::Ui,
    group: Group,
    state: &AppState,
    actions: &mut Vec<Action>,
    overflow: bool,
) {
    let paused = state.is_paused();
    match group {
        Group::File => {
            tool(
                ui,
                actions,
                icon::FILE_PLUS,
                "New",
                Action::NewFile,
                true,
                false,
            );
            tool(
                ui,
                actions,
                icon::FOLDER_OPEN,
                "Open",
                Action::OpenFile,
                true,
                false,
            );
            tool(
                ui,
                actions,
                icon::FLOPPY_DISK,
                "Save",
                Action::Save,
                true,
                false,
            );
            tool(
                ui,
                actions,
                icon::FLOPPY_DISK_BACK,
                "Save as",
                Action::SaveAs,
                true,
                false,
            );
            tool(
                ui,
                actions,
                icon::ARROWS_CLOCKWISE,
                "Automatic reload and preview",
                Action::ToggleAutoreload,
                true,
                state.settings.editor.autoreload,
            );
        }
        Group::Objects => {
            let has_objects = !state.scene.is_empty();
            tool(
                ui,
                actions,
                icon::TRASH,
                "Clear all objects",
                Action::ClearObjects,
                has_objects,
                false,
            );
            tool(
                ui,
                actions,
                icon::X_SQUARE,
                "Clear current object",
                Action::RemoveSelected,
                !state.scene.selection.is_empty(),
                false,
            );
        }
        Group::Run => {
            // Names the project's main script when another file is shown.
            let render = match state.main_script() {
                Some(main) if Some(main) != state.document.path() => format!(
                    "Render {}",
                    main.file_name().unwrap_or_default().to_string_lossy()
                ),
                _ => "Render".to_owned(),
            };
            // Green, as the run button of most editors: the first thing to
            // press.
            let run = state.palette().success;
            if icon_button_colored(ui, icon::PLAY, true, false, Some(run))
                .on_hover_text(tooltip_text(ui, &render, &Action::Render))
                .clicked()
            {
                actions.push(Action::Render);
            }
            tool(
                ui,
                actions,
                icon::TIMER,
                "Profile: render and show each line's time in the gutter",
                Action::Profile,
                true,
                !state.editor.line_profile.is_empty(),
            );
            tool(
                ui,
                actions,
                icon::BUG,
                "Debug",
                Action::ToggleDebug,
                true,
                state.is_debugging(),
            );
            tool(
                ui,
                actions,
                icon::ARROW_BEND_DOWN_RIGHT,
                "Step",
                Action::Debug(DebugAction::Step),
                paused,
                false,
            );
            tool(
                ui,
                actions,
                icon::ARROW_ELBOW_DOWN_RIGHT,
                "Step in",
                Action::Debug(DebugAction::StepIn),
                paused,
                false,
            );
            tool(
                ui,
                actions,
                icon::ARROW_ELBOW_UP_LEFT,
                "Step out",
                Action::Debug(DebugAction::StepOut),
                paused,
                false,
            );
            tool(
                ui,
                actions,
                icon::FAST_FORWARD,
                "Continue",
                Action::Debug(DebugAction::Continue),
                paused,
                false,
            );
            tool(
                ui,
                actions,
                icon::STOP,
                "Stop",
                Action::Stop,
                state.can_stop(),
                false,
            );
        }
        Group::Inspect => tool(
            ui,
            actions,
            icon::STACK,
            "Inspect CQ object",
            Action::ToggleInspect,
            true,
            state.inspector.show_in_view,
        ),
        Group::View => {
            tool(
                ui,
                actions,
                icon::CORNERS_OUT,
                "Fit",
                Action::Fit,
                true,
                false,
            );
            for view in StandardView::ALL {
                let action = Action::View(view);
                let tooltip = tooltip_text(ui, view.label(), &action);
                if view_button(ui, view).on_hover_text(tooltip).clicked() {
                    actions.push(action);
                }
            }
            let hint = "Named views: save the camera and come back to it";
            if overflow {
                ui.menu_button(icon::BOOKMARKS_SIMPLE, |ui| {
                    super::named_views::manager(ui, state, actions);
                })
                .response
                .on_hover_text(hint);
            } else {
                let views =
                    icon_button(ui, icon::BOOKMARKS_SIMPLE, true, false).on_hover_text(hint);
                egui::Popup::menu(&views)
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| super::named_views::manager(ui, state, actions));
            }
        }
        Group::Display => {
            for mode in DisplayMode::ALL {
                let (glyph, tooltip) = match mode {
                    DisplayMode::Wireframe => (icon::CUBE_TRANSPARENT, "Wireframe"),
                    DisplayMode::Shaded => (icon::CUBE, "Shaded"),
                    DisplayMode::XRay => (icon::INTERSECT_SQUARE, "X-ray: see-through faces"),
                    DisplayMode::HiddenLine => (
                        icon::RECTANGLE_DASHED,
                        "Hidden line: visible edges, hidden ones dashed",
                    ),
                    DisplayMode::Presentation => (
                        icon::SPARKLE,
                        "Presentation: the model for showing, with the lights, material \
                         and background of Preferences > Presentation",
                    ),
                };
                tool(
                    ui,
                    actions,
                    glyph,
                    tooltip,
                    Action::Display(mode),
                    true,
                    state.viewer.display_mode == mode,
                );
            }
            tool(
                ui,
                actions,
                icon::SCISSORS,
                "Section view: cut the model with a plane",
                Action::ToggleSection,
                true,
                state.viewer.section.enabled,
            );
            tool(
                ui,
                actions,
                icon::ARROWS_OUT,
                "Exploded view: move the parts apart",
                Action::ToggleExplode,
                true,
                state.viewer.explode_controls,
            );
            tool(
                ui,
                actions,
                icon::CAMERA,
                "Screenshot",
                Action::Screenshot,
                true,
                false,
            );
        }
        Group::Selection => {
            ui.label("Select:");
            let hint = |mode| match mode {
                SelectionMode::Auto => "Nearest vertex, edge or face",
                SelectionMode::Object => "Whole displayed object",
                SelectionMode::Face => "Faces only",
                SelectionMode::Edge => "Edges only",
                SelectionMode::Vertex => "Vertices only",
            };
            let mut choices = |ui: &mut egui::Ui| {
                for mode in SelectionMode::ALL {
                    let action = Action::SelectionMode(mode);
                    let custom = state.settings.viewer.selection_shortcuts.get(mode).trim();
                    let label = if custom.is_empty() {
                        hint(mode).to_owned()
                    } else {
                        format!("{} ({custom})", hint(mode))
                    };
                    let tooltip = tooltip_text(ui, &label, &action);
                    if ui
                        .selectable_label(state.viewer.selection_mode == mode, mode.label())
                        .on_hover_text(tooltip)
                        .clicked()
                    {
                        actions.push(action);
                    }
                }
            };
            if overflow {
                choices(ui);
            } else {
                egui::ComboBox::from_id_salt("topology_selection_mode")
                    .width(72.0)
                    .selected_text(state.viewer.selection_mode.label())
                    .show_ui(ui, choices);
            }
        }
    }
}

fn tool(
    ui: &mut egui::Ui,
    actions: &mut Vec<Action>,
    glyph: &str,
    tooltip: &str,
    action: Action,
    enabled: bool,
    selected: bool,
) {
    if icon_button(ui, glyph, enabled, selected)
        .on_hover_text(tooltip_text(ui, tooltip, &action))
        .clicked()
    {
        actions.push(action);
    }
}

fn tooltip_text(ui: &egui::Ui, label: &str, action: &Action) -> String {
    match commands::shortcut(action) {
        Some(shortcut) => format!("{label} ({})", ui.ctx().format_shortcut(&shortcut)),
        None => label.to_owned(),
    }
}

fn separator(ui: &mut egui::Ui) {
    ui.add_space(4.0);
    ui.separator();
    ui.add_space(4.0);
}

fn button_frame(ui: &mut egui::Ui, enabled: bool, selected: bool) -> (Rect, Response) {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(BUTTON_SIZE, sense);
    let visuals = ui.visuals();
    let fill = if selected {
        visuals.selection.bg_fill
    } else if enabled && response.is_pointer_button_down_on() {
        visuals.widgets.active.weak_bg_fill
    } else if enabled && response.hovered() {
        visuals.widgets.hovered.weak_bg_fill
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 5.0, fill);
    (rect, response)
}

fn icon_button(ui: &mut egui::Ui, glyph: &str, enabled: bool, selected: bool) -> Response {
    icon_button_colored(ui, glyph, enabled, selected, None)
}

/// An icon button whose glyph has `color` when it is enabled.
fn icon_button_colored(
    ui: &mut egui::Ui,
    glyph: &str,
    enabled: bool,
    selected: bool,
    color: Option<Color32>,
) -> Response {
    let (rect, response) = button_frame(ui, enabled, selected);
    let color = if enabled {
        color.unwrap_or_else(|| ui.visuals().text_color())
    } else {
        ui.visuals().weak_text_color().gamma_multiply(0.6)
    };
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        egui::FontId::proportional(17.0),
        color,
    );
    response
}

/// A small isometric cube with the face of `view` highlighted; hidden faces
/// (back, left, bottom) are drawn through the cube with dashed edges.
fn view_button(ui: &mut egui::Ui, view: StandardView) -> Response {
    let (rect, response) = button_frame(ui, true, false);
    let text = ui.visuals().text_color();
    let accent = ui.visuals().hyperlink_color;
    paint_view_icon(ui.painter(), rect.shrink(5.0), view, text, accent);
    response
}

fn paint_view_icon(
    painter: &egui::Painter,
    rect: Rect,
    view: StandardView,
    line: Color32,
    accent: Color32,
) {
    let rotation = StandardView::Iso.rotation().inverse();
    let scale = rect.height() / 3.2;
    let project = |p: Vec3| {
        let v = rotation * (p * 2.0 - Vec3::ONE);
        Pos2::new(rect.center().x + v.x * scale, rect.center().y - v.y * scale)
    };
    // Faces as (outward normal, corners).
    let faces: [(Vec3, [Vec3; 4]); 6] = [
        (
            Vec3::Z,
            [
                Vec3::new(0.0, 0.0, 1.0),
                Vec3::new(1.0, 0.0, 1.0),
                Vec3::new(1.0, 1.0, 1.0),
                Vec3::new(0.0, 1.0, 1.0),
            ],
        ),
        (
            Vec3::NEG_Z,
            [
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(1.0, 1.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
            ],
        ),
        (
            Vec3::NEG_Y,
            [
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 1.0),
                Vec3::new(0.0, 0.0, 1.0),
            ],
        ),
        (
            Vec3::Y,
            [
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.0, 1.0, 1.0),
                Vec3::new(1.0, 1.0, 1.0),
                Vec3::new(1.0, 1.0, 0.0),
            ],
        ),
        (
            Vec3::X,
            [
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(1.0, 1.0, 0.0),
                Vec3::new(1.0, 1.0, 1.0),
                Vec3::new(1.0, 0.0, 1.0),
            ],
        ),
        (
            Vec3::NEG_X,
            [
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
                Vec3::new(0.0, 1.0, 1.0),
                Vec3::new(0.0, 1.0, 0.0),
            ],
        ),
    ];
    let eye = StandardView::Iso.orientation().0;
    let highlighted = match view {
        StandardView::Iso => None,
        other => Some(other.orientation().0),
    };

    // Hidden geometry first, so visible faces cover it.
    for (normal, corners) in &faces {
        let visible = normal.dot(eye) > 0.0;
        let points: Vec<Pos2> = corners.iter().map(|c| project(*c)).collect();
        let is_target = highlighted.is_some_and(|h| h == *normal);
        if !visible {
            if is_target {
                painter.add(Shape::convex_polygon(
                    points.clone(),
                    accent.gamma_multiply(0.55),
                    Stroke::NONE,
                ));
            }
            for i in 0..4 {
                let dashes = Shape::dashed_line(
                    &[points[i], points[(i + 1) % 4]],
                    Stroke::new(0.8, line.gamma_multiply(0.45)),
                    1.5,
                    1.5,
                );
                painter.extend(dashes);
            }
        }
    }
    for (normal, corners) in &faces {
        if normal.dot(eye) <= 0.0 {
            continue;
        }
        let mut points: Vec<Pos2> = corners.iter().map(|c| project(*c)).collect();
        let area: f32 = points
            .iter()
            .zip(points.iter().cycle().skip(1))
            .map(|(a, b)| a.x * b.y - b.x * a.y)
            .sum();
        if area < 0.0 {
            points.reverse();
        }
        let fill = if highlighted.is_some_and(|h| h == *normal) || view == StandardView::Iso {
            accent.gamma_multiply(if view == StandardView::Iso { 0.45 } else { 0.9 })
        } else {
            Color32::TRANSPARENT
        };
        painter.add(Shape::convex_polygon(points, fill, Stroke::new(1.0, line)));
    }
}

#[cfg(test)]
#[path = "../../tests/rust/app/test_toolbar.rs"]
mod tests;
