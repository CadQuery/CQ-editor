//! Requirements declared by expect() in the last script run.

use egui_phosphor::regular as icon;

use crate::app::AppState;
use crate::commands::Action;

pub fn ui(ui: &mut egui::Ui, state: &mut AppState) {
    if state.cad_checks.is_empty() {
        super::empty_state(
            ui,
            icon::CHECK_CIRCLE,
            "No CAD requirements in the last run",
        );
        return;
    }
    let failed = state.cad_checks.iter().filter(|check| !check.ok).count();
    let total = state.cad_checks.len();
    ui.strong(format!("{} passed · {} failed", total - failed, failed));
    ui.separator();
    let mut actions = Vec::new();
    egui::ScrollArea::vertical().show(ui, |ui| {
        for check in &state.cad_checks {
            let color = if check.ok {
                ui.visuals().strong_text_color()
            } else {
                ui.visuals().error_fg_color
            };
            let symbol = if check.ok {
                icon::CHECK_CIRCLE
            } else {
                icon::X_CIRCLE
            };
            let location = check
                .line
                .map(|line| format!(" · line {line}"))
                .unwrap_or_default();
            let label = format!("{symbol} {} · {}{location}", check.name, check.kind);
            let response = ui.add(
                egui::Label::new(egui::RichText::new(label).color(color))
                    .sense(egui::Sense::click()),
            );
            ui.indent(response.id, |ui| {
                ui.label(&check.detail);
            });
            if response.clicked()
                && let Some(line) = check.line
            {
                if let Some(document) = state.cad_checks_document {
                    actions.push(Action::GoToScriptLine { document, line });
                } else {
                    actions.push(Action::GoToLine(line));
                }
            }
            ui.separator();
        }
    });
    state.queue(actions);
}
