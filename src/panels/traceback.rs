//! The error of the last run. Clicking a frame jumps to its line: in the
//! script that ran, or in the imported module, which opens in a tab.

use std::path::Path;

use egui::RichText;
use egui_extras::{Column, TableBuilder};
use egui_phosphor::regular as icon;

use crate::app::AppState;
use crate::commands::Action;
use crate::worker::protocol::SCRIPT_FILENAME;

pub fn ui(ui: &mut egui::Ui, state: &mut AppState) {
    let Some(error) = &state.traceback else {
        super::empty_state(ui, icon::CHECK_CIRCLE, "No errors");
        return;
    };
    let mut actions = Vec::new();
    let script = state.traceback_document;
    let script_name = script
        .and_then(|document| state.document_of(document))
        .map_or_else(|| "script".to_owned(), |document| document.display_name());

    egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let error_color = ui.visuals().error_fg_color;
                ui.label(RichText::new(icon::WARNING_CIRCLE).color(error_color));
                let summary = format!("{}: {}", error.type_name, error.message);
                ui.add(
                    egui::Label::new(RichText::new(&summary).color(error_color).strong())
                        .truncate(),
                )
                .on_hover_text(&summary);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(icon::COPY)
                        .on_hover_text("Copy the full traceback")
                        .clicked()
                    {
                        ui.ctx().copy_text(error.text.clone());
                    }
                });
            });
        });

    if let Some(diagnostic) = &error.diagnostic {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                let operation = diagnostic.receiver.as_ref().map_or_else(
                    || diagnostic.operation.clone(),
                    |receiver| format!("{receiver}.{}", diagnostic.operation),
                );
                ui.strong(format!("Possible CAD causes · {operation}"));
                if ui.button(format!("Line {}", diagnostic.line)).clicked() {
                    if let Some(document) = script {
                        actions.push(Action::GoToScriptLine {
                            document,
                            line: diagnostic.line,
                        });
                    } else {
                        actions.push(Action::GoToLine(diagnostic.line));
                    }
                }
            });
            for hint in &diagnostic.hints {
                ui.label(format!("• {hint}"));
            }
            ui.weak("These are suggestions; the original traceback below is unchanged.");
        });
        ui.add_space(4.0);
    }

    let row_height = ui.text_style_height(&egui::TextStyle::Body) + 6.0;
    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .auto_shrink([false, false])
        .sense(egui::Sense::click())
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(140.0).at_least(60.0).clip(true))
        .column(Column::initial(50.0).at_least(40.0))
        .column(Column::initial(110.0).at_least(50.0).clip(true))
        .column(Column::remainder().clip(true))
        .header(row_height, |mut header| {
            for title in ["File", "Line", "Function", "Code"] {
                header.col(|ui| {
                    ui.strong(title);
                });
            }
        })
        .body(|body| {
            body.rows(row_height, error.frames.len(), |mut row| {
                let frame = &error.frames[row.index()];
                let in_script = frame.file == SCRIPT_FILENAME;
                // A module of the project, or any Python file that exists.
                let module = (!in_script)
                    .then(|| Path::new(&frame.file))
                    .filter(|path| path.is_absolute() && path.is_file());
                let file = if in_script {
                    script_name.as_str()
                } else {
                    frame.file.as_str()
                };
                // Not selectable: a selectable label takes the clicks on
                // its text away from the row, which jumps to the line.
                let cell = |text: RichText| egui::Label::new(text).selectable(false);
                row.col(|ui| {
                    if in_script {
                        ui.add(cell(RichText::new(file).strong()));
                    } else if module.is_some() {
                        let name = Path::new(file)
                            .file_name()
                            .map_or_else(|| file.to_owned(), |n| n.to_string_lossy().into_owned());
                        ui.add(cell(RichText::new(name))).on_hover_text(format!(
                            "{file}\nClick to open it at line {}",
                            frame.line
                        ));
                    } else {
                        ui.add(cell(RichText::new(file).weak()))
                            .on_hover_text(&frame.file);
                    }
                });
                row.col(|ui| {
                    ui.add(cell(RichText::new(frame.line.to_string())));
                });
                row.col(|ui| {
                    ui.add(cell(RichText::new(&frame.function).weak()));
                });
                row.col(|ui| {
                    ui.add(cell(RichText::new(&frame.code).monospace()).truncate());
                });
                if row.response().clicked() {
                    match (in_script, script, module) {
                        (true, Some(document), _) => actions.push(Action::GoToScriptLine {
                            document,
                            line: frame.line,
                        }),
                        (true, None, _) => actions.push(Action::GoToLine(frame.line)),
                        (false, _, Some(path)) => actions.push(Action::OpenFileAt {
                            path: path.to_path_buf(),
                            line: frame.line,
                        }),
                        (false, _, None) => {}
                    }
                }
            });
        });
    state.queue(actions);
}
