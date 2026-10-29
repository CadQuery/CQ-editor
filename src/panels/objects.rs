//! The object tree: shown CadQuery objects, the axis helpers and the
//! properties of the current object.

use std::collections::HashSet;

use egui::{Color32, RichText, Sense, Vec2};
use egui_phosphor::regular as icon;

use crate::app::AppState;
use crate::commands::Action;
use crate::panels::measurements;
use crate::scene::{Rgb, SceneNode};
use crate::settings::Finish;
use crate::worker::protocol::{EntityMeasurement, ExportFormat, SelectorCandidate};

/// Height the object tree keeps while the property sheet is open.
const TREE_MIN_HEIGHT: f32 = 80.0;
/// Smallest useful property sheet.
const SHEET_MIN_HEIGHT: f32 = 70.0;

pub fn ui(ui: &mut egui::Ui, state: &mut AppState) {
    let mut actions = Vec::new();

    egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(6, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        !state.scene.is_empty(),
                        egui::Button::new(format!("{} Clear all", icon::TRASH)),
                    )
                    .on_hover_text("Remove all objects")
                    .clicked()
                {
                    actions.push(Action::ClearObjects);
                }
                if ui
                    .add_enabled(
                        !state.scene.selection.is_empty(),
                        egui::Button::new(icon::X),
                    )
                    .on_hover_text("Remove the selected objects")
                    .clicked()
                {
                    actions.push(Action::RemoveSelected);
                }
                ui.add_enabled_ui(!state.scene.is_empty(), |ui| {
                    ui.menu_button(format!("{} Export", icon::EXPORT), |ui| {
                        export_menu(ui, &mut actions);
                    })
                    .response
                    .on_hover_text("Export the selection, or everything when nothing is selected");
                });
                if ui
                    .add_enabled(
                        !state.scene.is_empty(),
                        egui::Button::new(format!("{} Check", icon::CHECK_CIRCLE)),
                    )
                    .on_hover_text(
                        "Check selected geometry, or everything when nothing is selected",
                    )
                    .clicked()
                {
                    actions.push(Action::CheckGeometry);
                }
            });
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let has_filter = !state.scene.filter.is_empty();
                let width = (ui.available_width() - if has_filter { 22.0 } else { 0.0 }).max(40.0);
                ui.add(
                    egui::TextEdit::singleline(&mut state.scene.filter)
                        .hint_text(format!(
                            "{} Filter objects by name…",
                            icon::MAGNIFYING_GLASS
                        ))
                        .desired_width(width),
                );
                if has_filter
                    && ui
                        .add(egui::Button::new(icon::X).frame(false))
                        .on_hover_text("Clear object filter")
                        .clicked()
                {
                    state.scene.filter.clear();
                }
            });
        });
    ui.separator();

    if !state.scene.selection.is_empty() {
        property_sheet(ui, |ui| properties(ui, state, &mut actions));
    }

    // The tree takes what the properties panel leaves (panels must be
    // followed by a central panel to keep out of their space).
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ui, |ui| {
            tree(ui, state, &mut actions);
        });

    state.queue(actions);
}

/// Shows the property sheet at the bottom of `ui` and returns its rect.
///
/// A panel grows to fit its contents, so on a short pane the sheet used to
/// take the whole height and the tree was drawn over it. The sheet now
/// scrolls instead, and its height is capped so the tree keeps
/// [`TREE_MIN_HEIGHT`] (or most of a very short pane).
fn property_sheet(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) -> egui::Rect {
    let available = ui.available_height();
    let max_height = (available - TREE_MIN_HEIGHT).max(available * 0.4);
    egui::Panel::bottom("object_properties")
        .resizable(true)
        .default_size(130.0)
        .size_range(SHEET_MIN_HEIGHT.min(max_height)..=max_height)
        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(8, 6)))
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("object_properties_scroll")
                .auto_shrink([false, false])
                .min_scrolled_height(0.0)
                .show(ui, add_contents);
        })
        .response
        .rect
}

fn tree(ui: &mut egui::Ui, state: &mut AppState, actions: &mut Vec<Action>) {
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        // The default minimum (64 px) would overflow a squeezed pane.
        .min_scrolled_height(0.0)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            let default = state.default_color();
            let filter = state.scene.filter.trim().to_owned();
            egui::CollapsingHeader::new(RichText::new("CQ models").strong())
                .default_open(true)
                .show(ui, |ui| {
                    if state.scene.is_empty() {
                        ui.weak(state.empty_view_hint());
                    } else {
                        let isolated = state.scene.isolation_members();
                        let mut any_shown = false;
                        for node in &state.scene.nodes {
                            if node_matches_filter(node, &filter) {
                                any_shown = true;
                                node_row(
                                    ui,
                                    state,
                                    node,
                                    default,
                                    isolated.as_ref(),
                                    &filter,
                                    actions,
                                );
                            }
                        }
                        if !any_shown {
                            ui.weak("No matching objects");
                        }
                    }
                });
            egui::CollapsingHeader::new(RichText::new("Helpers").strong())
                .default_open(true)
                .show(ui, |ui| {
                    for axis in &mut state.scene.axes {
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut axis.visible, "");
                            swatch(ui, axis.color, 1.0);
                            ui.label(format!("{} axis", axis.name));
                        });
                    }
                });
        });
}

/// Returns `true` when `node` or any of its descendants matches `query`
/// (case-insensitive substring match on the object/component name).
pub fn node_matches_filter(node: &SceneNode, query: &str) -> bool {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return true;
    }
    let needle = trimmed.to_lowercase();
    node.name.to_lowercase().contains(&needle)
        || node
            .children
            .iter()
            .any(|child| node_matches_filter(child, trimmed))
}

fn export_menu(ui: &mut egui::Ui, actions: &mut Vec<Action>) {
    for format in ExportFormat::ALL {
        if ui
            .button(format!("Export as {}…", format.label()))
            .clicked()
        {
            actions.push(Action::Export(format));
        }
    }
}

fn node_row(
    ui: &mut egui::Ui,
    state: &AppState,
    node: &SceneNode,
    default: Rgb,
    isolated: Option<&HashSet<&str>>,
    filter: &str,
    actions: &mut Vec<Action>,
) {
    let isolated_out = isolated.is_some_and(|members| !members.contains(node.key.as_str()));
    let id = ui.make_persistent_id(("scene_node", &node.key));
    let mut header = |ui: &mut egui::Ui| {
        let mut visible = node.visible;
        if ui
            .checkbox(&mut visible, "")
            .on_hover_text("Show / hide")
            .changed()
        {
            actions.push(Action::SetVisible {
                key: node.key.clone(),
                visible,
            });
        }
        if let Some((color, alpha)) = node.display_color(default) {
            swatch(ui, color, alpha);
        }
        let selected = state.scene.is_selected(&node.key);
        let invalid = state
            .geometry_checks
            .get(&node.key)
            .is_some_and(|report| !report.valid);
        let mut text = RichText::new(if invalid {
            format!("{} {}", icon::WARNING_CIRCLE, node.name)
        } else {
            node.name.clone()
        });
        if invalid {
            text = text.color(ui.visuals().warn_fg_color);
        }
        if !node.visible || isolated_out {
            text = text.weak();
        }
        let response = ui
            .selectable_label(selected, text)
            .on_hover_text(if isolated_out {
                format!("{} · hidden by the isolation", node.kind)
            } else {
                node.kind.clone()
            });
        if response.clicked() {
            actions.push(Action::Select {
                key: Some(node.key.clone()),
                topology: None,
                extend: ui.input(|i| i.modifiers.command),
            });
        }
        if response.double_clicked() {
            actions.push(Action::FitSelection);
        }
        response.context_menu(|ui| {
            if !selected {
                actions.push(Action::Select {
                    key: Some(node.key.clone()),
                    topology: None,
                    extend: false,
                });
            }
            if ui
                .button(format!("{} Zoom to", icon::MAGNIFYING_GLASS_PLUS))
                .clicked()
            {
                actions.push(Action::FitSelection);
                ui.close();
            }
            if ui
                .add_enabled(
                    node.line.is_some(),
                    egui::Button::new(format!("{} Go to Source", icon::ARROW_SQUARE_OUT)),
                )
                .on_hover_text("The script line that created it")
                .on_disabled_hover_text("No script line is known (shown from the console?)")
                .clicked()
            {
                actions.push(Action::GoToSource {
                    key: node.key.clone(),
                    step: None,
                });
                ui.close();
            }
            let (label, visible) = if node.visible {
                ("Hide", false)
            } else {
                ("Show", true)
            };
            if ui
                .button(format!(
                    "{} {label}",
                    if visible { icon::EYE } else { icon::EYE_SLASH }
                ))
                .clicked()
            {
                actions.push(Action::SetVisible {
                    key: node.key.clone(),
                    visible,
                });
                ui.close();
            }
            let isolating = isolated.is_some();
            if ui
                .button(format!(
                    "{} {}",
                    icon::SELECTION_FOREGROUND,
                    if isolating {
                        "End Isolation"
                    } else {
                        "Isolate"
                    }
                ))
                .on_hover_text(if isolating {
                    "Show the other objects again"
                } else {
                    "Show only the selected objects"
                })
                .clicked()
            {
                actions.push(Action::ToggleIsolate);
                ui.close();
            }
            if ui.button(format!("{} Show All", icon::EYE)).clicked() {
                actions.push(Action::ShowAll);
                ui.close();
            }
            ui.separator();
            if ui
                .button(format!("{} Check Geometry", icon::CHECK_CIRCLE))
                .clicked()
            {
                actions.push(Action::CheckGeometry);
                ui.close();
            }
            export_menu(ui, actions);
            ui.separator();
            if ui.button(format!("{} Remove", icon::TRASH)).clicked() {
                actions.push(Action::RemoveSelected);
                ui.close();
            }
        });
    };

    if node.children.is_empty() {
        ui.horizontal(|ui| {
            // Line up with rows that have an expand arrow.
            ui.add_space(ui.spacing().icon_width + ui.spacing().item_spacing.x);
            header(ui);
        });
    } else {
        let has_matching_child = !filter.is_empty()
            && node
                .children
                .iter()
                .any(|child| node_matches_filter(child, filter));
        let mut collapsing =
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true);
        if has_matching_child {
            collapsing.set_open(true);
        }
        collapsing.show_header(ui, header).body(|ui| {
            for child in &node.children {
                if !has_matching_child || node_matches_filter(child, filter) {
                    node_row(ui, state, child, default, isolated, filter, actions);
                }
            }
        });
    }
}

/// A small rounded square showing a color.
fn swatch(ui: &mut egui::Ui, color: Rgb, alpha: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
    let [r, g, b] = color.map(|c| (c * 255.0).round() as u8);
    let fill = Color32::from_rgba_unmultiplied(r, g, b, (alpha.max(0.25) * 255.0) as u8);
    ui.painter().rect_filled(rect, 3.0, fill);
    ui.painter().rect_stroke(
        rect,
        3.0,
        ui.visuals().widgets.noninteractive.bg_stroke,
        egui::StrokeKind::Inside,
    );
}

fn properties(ui: &mut egui::Ui, state: &mut AppState, actions: &mut Vec<Action>) {
    let default = state.default_color();
    let show_edges = state.settings.viewer.show_edges;
    let Some(key) = state.scene.selection.last().cloned() else {
        return;
    };
    let topology = state
        .scene
        .current_topology()
        .filter(|selection| selection.node == key)
        .cloned();
    let measured = topology.as_ref().and_then(|selection| {
        state
            .measurements
            .result
            .as_ref()
            .and_then(|result| measurements::measurement_of(result, selection))
            .cloned()
    });
    let measuring = state.measurements.loading;
    let selector = topology
        .as_ref()
        .and_then(|_| state.selector.candidate().cloned());
    let Some(node) = state.scene.find_mut(&key) else {
        return;
    };

    ui.strong(&node.name);
    if let Some(selection) = &topology {
        let label = node
            .parts
            .get(selection.part)
            .and_then(|part| {
                part.face_attributes.iter().find(|face| {
                    selection.entity.kind() == "Face" && face.id == selection.entity.identifier()
                })
            })
            .and_then(|face| face.name.as_deref())
            .map_or_else(
                || {
                    format!(
                        "{} {}",
                        selection.entity.kind(),
                        selection.entity.identifier()
                    )
                },
                str::to_owned,
            );
        ui.label(format!("{label} · part {}", selection.part + 1));
    }
    egui::Grid::new("object_properties_grid")
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            if let Some(entity) = &measured {
                measured_rows(ui, entity);
            } else if topology.is_some() && measuring {
                ui.label("Geometry");
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.weak("Measuring…");
                });
                ui.end_row();
            }
            if let Some(candidate) = &selector {
                selector_row(ui, candidate, actions);
            }

            ui.label("Type");
            ui.weak(&node.kind);
            ui.end_row();
            if let Some(material) = node.parts.first().and_then(|part| part.material.as_ref()) {
                ui.label("Material");
                ui.weak(format!(
                    "{} · {} {}",
                    material.name, material.density, material.density_unit
                ));
                ui.end_row();
            }

            if let Some(line) = node.line {
                ui.label("Source");
                if ui
                    .link(format!("line {line} {}", icon::ARROW_SQUARE_OUT))
                    .on_hover_text("Go to the script line that created it (Ctrl+Shift+G)")
                    .clicked()
                {
                    actions.push(Action::GoToSource {
                        key: key.clone(),
                        step: None,
                    });
                }
                ui.end_row();
            }

            ui.label("Visible");
            let mut visible = node.visible;
            if ui.checkbox(&mut visible, "").changed() {
                actions.push(Action::SetVisible {
                    key: key.clone(),
                    visible,
                });
            }
            ui.end_row();

            if !node.parts.is_empty() {
                let (color, alpha) = node
                    .parts
                    .first()
                    .map(|part| node.part_color(part, default))
                    .unwrap_or((default, 1.0));
                let mut srgb = color.map(|c| (c * 255.0).round() as u8);
                ui.label("Color");
                ui.horizontal(|ui| {
                    if ui.color_edit_button_srgb(&mut srgb).changed() {
                        node.overrides.color = Some(srgb.map(|c| f32::from(c) / 255.0));
                    }
                    let overrides = &mut node.overrides;
                    if (overrides.color.is_some() || overrides.alpha.is_some())
                        && ui
                            .small_button("Reset")
                            .on_hover_text("Use the script's colors")
                            .clicked()
                    {
                        overrides.color = None;
                        overrides.alpha = None;
                    }
                });
                ui.end_row();

                ui.label("Opacity");
                let mut opacity = alpha;
                if ui
                    .add(egui::Slider::new(&mut opacity, 0.05..=1.0).fixed_decimals(2))
                    .changed()
                {
                    node.overrides.alpha = Some(opacity);
                }
                ui.end_row();

                ui.label("Edges");
                ui.horizontal(|ui| {
                    let mut edges = node.overrides.edges.unwrap_or(show_edges);
                    if ui
                        .checkbox(&mut edges, "")
                        .on_hover_text(
                            "Show this object's edges in the Shaded and X-Ray modes \
                             (Wireframe and Hidden Line always show them)",
                        )
                        .changed()
                    {
                        node.overrides.edges = Some(edges);
                    }
                    if node.overrides.edges.is_some()
                        && ui
                            .small_button("Reset")
                            .on_hover_text("Follow Preferences > 3D View > Show edges")
                            .clicked()
                    {
                        node.overrides.edges = None;
                    }
                });
                ui.end_row();

                ui.label("Material");
                ui.horizontal(|ui| {
                    let current = node.finish();
                    egui::ComboBox::from_id_salt(("finish", key.as_str()))
                        .selected_text(current.label())
                        .show_ui(ui, |ui| {
                            for finish in Finish::ALL {
                                if ui
                                    .selectable_label(finish == current, finish.label())
                                    .clicked()
                                {
                                    node.overrides.finish = Some(finish);
                                }
                            }
                        })
                        .response
                        .on_hover_text(
                            "How the faces are shaded: Default is the plain shading of a CAD \
                             view; the materials have their own gloss, and metals reflect a \
                             studio around the model. Scripts choose one with \
                             show_object(..., material=\"matte plastic\")",
                        );
                    if node.overrides.finish.is_some()
                        && ui
                            .small_button("Reset")
                            .on_hover_text("Use the script's material")
                            .clicked()
                    {
                        node.overrides.finish = None;
                    }
                });
                ui.end_row();
            }

            let bounds = node.subtree_bounds();
            if !bounds.is_empty() {
                let size = bounds.size();
                ui.label("Size");
                ui.weak(format!(
                    "{} × {} × {}",
                    number(size.x),
                    number(size.y),
                    number(size.z)
                ));
                ui.end_row();
            }
            let triangles = node.triangle_count();
            if triangles > 0 {
                ui.label("Triangles");
                ui.weak(triangles.to_string());
                ui.end_row();
            }
        });
    if node.inspectable {
        ui.add_space(4.0);
        ui.weak(format!(
            "{} Its construction stack is in CQ Inspector",
            icon::STACK
        ));
    }
}

/// The selector of the picked entities, with Copy and Insert buttons.
fn selector_row(ui: &mut egui::Ui, candidate: &SelectorCandidate, actions: &mut Vec<Action>) {
    ui.label("Selector");
    ui.horizontal(|ui| {
        let text = RichText::new(&candidate.expression).monospace();
        let label = ui.add(egui::Label::new(text).truncate());
        let tip = if candidate.index_based {
            format!(
                "{} (by index: may change with the model)",
                candidate.description
            )
        } else {
            candidate.description.clone()
        };
        label.on_hover_text(tip);
        if ui
            .small_button(icon::COPY)
            .on_hover_text("Copy the selector")
            .clicked()
        {
            actions.push(Action::CopySelector);
        }
        if ui
            .small_button(icon::CURSOR_TEXT)
            .on_hover_text("Insert the selector at the editor's cursor")
            .clicked()
        {
            actions.push(Action::InsertSelector);
        }
    });
    ui.end_row();
}

/// The exact size and position of the selected face, edge or vertex.
fn measured_rows(ui: &mut egui::Ui, entity: &EntityMeasurement) {
    let mut row = |label: &str, value: String| {
        ui.label(label);
        ui.monospace(value);
        ui.end_row();
    };
    row("Geometry", measurements::title_case(&entity.geometry_type));
    if let Some(area) = entity.area {
        row("Area", compact(area));
    }
    if let Some(length) = entity.length {
        row("Length", compact(length));
    }
    if let Some(radius) = entity.radius {
        row("Radius", compact(radius));
    }
    let point = |[x, y, z]: [f64; 3]| format!("({}, {}, {})", compact(x), compact(y), compact(z));
    match entity.position {
        Some(position) => row("Position", point(position)),
        None => row("Center", point(entity.center)),
    }
}

/// A short number for the narrow property sheet: four decimals, without
/// the rounding noise of exact geometry (`-6.9e-18` is 0). The
/// Measurements panel keeps the full precision.
fn compact(value: f64) -> String {
    let text = format!("{value:.4}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" { "0" } else { text }.to_owned()
}

/// Compact number formatting for sizes.
fn number(value: f32) -> String {
    let text = format!("{value:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

#[cfg(test)]
#[path = "../../tests/rust/panels/test_objects.rs"]
mod tests;
