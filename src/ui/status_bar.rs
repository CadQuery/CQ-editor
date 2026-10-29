//! The status bar: last message on the left; Python worker state, cursor
//! position and file format on the right.

use egui::RichText;
use egui_phosphor::regular as icon;

use crate::app::{AppState, StatusKind};
use crate::document::Encoding;
use crate::worker;

pub fn ui(ui: &mut egui::Ui, state: &AppState) {
    let visuals = ui.visuals().clone();
    let success = state.palette().success;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 14.0;
        // The fixed indicators are laid out first, from the right; the
        // message gets what remains, so a long error cannot push them out.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (glyph, text, color) = match state.worker.status() {
                worker::Status::Ready(info) => (
                    icon::CIRCLE,
                    format!("Python {} · CadQuery {}", info.python, info.cadquery),
                    success,
                ),
                worker::Status::Starting => (
                    icon::HOURGLASS,
                    "Starting Python…".to_owned(),
                    visuals.warn_fg_color,
                ),
                worker::Status::NotStarted => (
                    icon::CIRCLE_DASHED,
                    "Python not started".to_owned(),
                    visuals.weak_text_color(),
                ),
                worker::Status::Failed(_) => (
                    icon::WARNING_OCTAGON,
                    "Python unavailable".to_owned(),
                    visuals.error_fg_color,
                ),
            };
            let python = state
                .worker
                .config()
                .map(|c| c.python.display().to_string())
                .unwrap_or_default();
            let tooltip = match state.worker.status() {
                worker::Status::Failed(reason) => format!("{reason}\n{python}"),
                worker::Status::Ready(info) => {
                    format!("CadQuery {} · OCP {}\n{python}", info.cadquery, info.ocp)
                }
                _ => python,
            };
            ui.label(RichText::new(text).small())
                .on_hover_text(&tooltip);
            ui.label(RichText::new(glyph).color(color).small())
                .on_hover_text(&tooltip);

            match state.document.encoding() {
                Encoding::Utf8 => {
                    ui.label(RichText::new("UTF-8").small().weak());
                }
                legacy => {
                    ui.label(
                        RichText::new(format!("{} → UTF-8", legacy.label()))
                            .small()
                            .color(visuals.warn_fg_color),
                    )
                    .on_hover_text(format!(
                        "The file was read as {}; saving converts it to UTF-8",
                        legacy.label()
                    ));
                }
            }
            ui.label(
                RichText::new(state.document.line_ending().label())
                    .small()
                    .weak(),
            );
            let (line, column) = state.editor.cursor_position(&state.document.text);
            ui.label(RichText::new(format!("Ln {line}, Col {column}")).small());
            if let Some(main) = state.main_script()
                && Some(main) != state.document.path()
            {
                let name = main.file_name().unwrap_or_default().to_string_lossy();
                ui.label(RichText::new(format!("{} {name}", icon::PLAY_CIRCLE)).small())
                    .on_hover_text(format!(
                        "F5 renders the project's main script:\n{}",
                        main.display()
                    ));
            }
            if state.settings.editor.autoreload {
                ui.label(
                    RichText::new(format!("{} Autoreload", icon::ARROWS_CLOCKWISE))
                        .small()
                        .weak(),
                )
                .on_hover_text("The script re-runs whenever its file changes on disk");
            }

            if let Some(status) = &state.status {
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    let (glyph, color) = match status.kind {
                        StatusKind::Info => (icon::INFO, visuals.weak_text_color()),
                        StatusKind::Success => (icon::CHECK_CIRCLE, success),
                        StatusKind::Warning => (icon::WARNING, visuals.warn_fg_color),
                        StatusKind::Error => (icon::X_CIRCLE, visuals.error_fg_color),
                    };
                    ui.label(RichText::new(glyph).color(color));
                    // Shows the whole text on hover when truncated.
                    ui.add(egui::Label::new(&status.text).truncate());
                });
            }
        });
    });
}
