//! The application log: script output, Python logging, worker diagnostics
//! and the editor's own messages.

use egui::{Color32, RichText};
use egui_phosphor::regular as icon;

use crate::app::AppState;
use crate::commands::Action;
use crate::logging::{self, Level};

pub struct LogView {
    filter: String,
    show_debug: bool,
    show_info: bool,
    show_warnings: bool,
    show_errors: bool,
    /// Indices of the entries passing the filter, and what they were
    /// computed for.
    visible: Vec<usize>,
    computed_for: Option<(u64, String, [bool; 4])>,
}

impl Default for LogView {
    fn default() -> Self {
        Self {
            filter: String::new(),
            show_debug: false,
            show_info: true,
            show_warnings: true,
            show_errors: true,
            visible: Vec::new(),
            computed_for: None,
        }
    }
}

impl LogView {
    fn shows(&self, level: Level) -> bool {
        match level {
            Level::Error => self.show_errors,
            Level::Warn => self.show_warnings,
            Level::Info => self.show_info,
            Level::Debug => self.show_debug,
        }
    }

    fn refresh(&mut self) {
        let key = (
            logging::revision(),
            self.filter.clone(),
            [
                self.show_debug,
                self.show_info,
                self.show_warnings,
                self.show_errors,
            ],
        );
        if self.computed_for.as_ref() == Some(&key) {
            return;
        }
        let needle = self.filter.to_lowercase();
        self.visible = logging::with_entries(|entries| {
            entries
                .iter()
                .enumerate()
                .filter(|(_, e)| self.shows(e.level))
                .filter(|(_, e)| needle.is_empty() || matches_filter(e, &needle))
                .map(|(i, _)| i)
                .collect()
        });
        self.computed_for = Some(key);
    }
}

/// Whether the text or the source of `entry` contains `needle`, which is
/// lowercase: the filter ignores case in both.
fn matches_filter(entry: &logging::Entry, needle: &str) -> bool {
    entry.text.to_lowercase().contains(needle) || entry.source.to_lowercase().contains(needle)
}

pub fn ui(ui: &mut egui::Ui, state: &mut AppState) {
    let mut actions = Vec::new();
    let view = &mut state.log_view;

    egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(6, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(icon::FUNNEL_SIMPLE);
                ui.add(
                    egui::TextEdit::singleline(&mut view.filter)
                        .hint_text("Filter")
                        .desired_width(160.0),
                );
                ui.toggle_value(&mut view.show_errors, "Errors");
                ui.toggle_value(&mut view.show_warnings, "Warnings");
                ui.toggle_value(&mut view.show_info, "Info");
                ui.toggle_value(&mut view.show_debug, "Debug");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(icon::ERASER)
                        .on_hover_text("Clear the log")
                        .clicked()
                    {
                        actions.push(Action::ClearLog);
                    }
                    if ui
                        .button(icon::COPY)
                        .on_hover_text("Copy the shown lines")
                        .clicked()
                    {
                        let text = logging::with_entries(|entries| {
                            view.visible
                                .iter()
                                .filter_map(|&i| entries.get(i))
                                .map(|e| {
                                    format!(
                                        "[{}] {} {}: {}",
                                        e.time,
                                        e.level.label(),
                                        e.source,
                                        e.text
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join("\n")
                        });
                        ui.ctx().copy_text(text);
                    }
                });
            });
        });
    ui.separator();

    view.refresh();
    let colors = [
        ui.visuals().error_fg_color,
        ui.visuals().warn_fg_color,
        ui.visuals().text_color(),
        ui.visuals().weak_text_color(),
    ];
    let row_height = ui.text_style_height(&egui::TextStyle::Monospace) + 2.0;
    let visible = &view.visible;
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .stick_to_bottom(true)
        .show_rows(ui, row_height, visible.len(), |ui, rows| {
            logging::with_entries(|entries| {
                for &index in &visible[rows] {
                    let Some(entry) = entries.get(index) else {
                        continue;
                    };
                    let color: Color32 = match entry.level {
                        Level::Error => colors[0],
                        Level::Warn => colors[1],
                        Level::Info => colors[2],
                        Level::Debug => colors[3],
                    };
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        ui.label(RichText::new(&entry.time).monospace().weak());
                        ui.label(RichText::new(&entry.source).monospace().weak());
                        ui.add(
                            egui::Label::new(RichText::new(&entry.text).monospace().color(color))
                                .extend(),
                        );
                    });
                }
            });
        });
    state.queue(actions);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_filter_ignores_case_in_sources_too() {
        let entry = logging::Entry {
            time: "12:00:00".to_owned(),
            level: Level::Warn,
            source: "OCP.BRepMesh".to_owned(),
            text: "Mesh warning".to_owned(),
        };
        assert!(matches_filter(&entry, "brepmesh"));
        assert!(matches_filter(&entry, "warning"));
        assert!(!matches_filter(&entry, "python"));
    }
}
