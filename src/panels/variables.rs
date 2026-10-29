//! Variables of the last run, or of the paused frame while debugging.
//!
//! The value column takes the width the name and type columns leave, and
//! clicking a row shows its whole value below the table, where it can be
//! read, selected and copied.

use egui::RichText;
use egui_extras::{Column, TableBuilder};
use egui_phosphor::regular as icon;

use crate::app::AppState;
use crate::worker::protocol::Variable;

/// The value column never gets narrower than this.
const MIN_VALUE_WIDTH: f32 = 80.0;
const DETAIL_DEFAULT_HEIGHT: f32 = 90.0;
const DETAIL_MIN_HEIGHT: f32 = 48.0;

pub fn ui(ui: &mut egui::Ui, state: &mut AppState) {
    let filter_id = ui.id().with("variables_filter");
    let selected_id = ui.id().with("variables_selected");
    let mut filter: String = ui.data_mut(|d| d.get_temp(filter_id).unwrap_or_default());
    let mut selected: Option<String> = ui.data_mut(|d| d.get_temp(selected_id)).flatten();

    egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(6, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(icon::FUNNEL_SIMPLE);
                ui.add(
                    egui::TextEdit::singleline(&mut filter)
                        .hint_text("Filter by name or type")
                        .desired_width(f32::INFINITY),
                );
            });
        });
    ui.data_mut(|d| d.insert_temp(filter_id, filter.clone()));

    if state.variables.is_empty() {
        super::empty_state(ui, icon::BRACKETS_CURLY, "Variables appear after a run");
        return;
    }

    let needle = filter.to_lowercase();
    let rows: Vec<&Variable> = state
        .variables
        .iter()
        .filter(|v| {
            needle.is_empty()
                || v.name.to_lowercase().contains(&needle)
                || v.type_name.to_lowercase().contains(&needle)
        })
        .collect();

    table_and_detail(ui, &rows, &mut selected);
    ui.data_mut(|d| d.insert_temp(selected_id, selected));
}

/// The table and, for the selected row, its detail view below. Returns
/// where the table's rows and the detail view ended up.
fn table_and_detail(
    ui: &mut egui::Ui,
    rows: &[&Variable],
    selected: &mut Option<String>,
) -> (egui::Rect, Option<egui::Rect>) {
    // A selection that is no longer listed (another run, the filter) closes
    // the detail view.
    let detail = selected
        .as_deref()
        .and_then(|name| rows.iter().copied().find(|v| v.name == name));
    if detail.is_none() {
        *selected = None;
    }
    let detail_rect = detail.map(|variable| {
        let max_height = (ui.available_height() * 0.6).max(DETAIL_MIN_HEIGHT);
        egui::Panel::bottom("variable_detail")
            .resizable(true)
            .default_size(DETAIL_DEFAULT_HEIGHT)
            .size_range(DETAIL_MIN_HEIGHT..=max_height)
            .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(8, 6)))
            .show(ui, |ui| {
                if !detail_view(ui, variable) {
                    *selected = None;
                }
            })
            .response
            .rect
    });

    let table_rect = egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ui, |ui| table(ui, rows, selected))
        .inner;
    (table_rect, detail_rect)
}

/// Column widths for a panel `width` wide: name and type get a share of it,
/// so the value column keeps room even in a narrow pane.
pub(crate) fn column_widths(width: f32) -> (f32, f32) {
    let name = (width * 0.3).clamp(50.0, 160.0);
    let type_name = (width * 0.22).clamp(40.0, 110.0);
    (name, type_name)
}

/// Returns the rectangle the rows are shown in.
fn table(ui: &mut egui::Ui, rows: &[&Variable], selected: &mut Option<String>) -> egui::Rect {
    let row_height = ui.text_style_height(&egui::TextStyle::Body) + 4.0;
    let (name_width, type_width) = column_widths(ui.available_width());
    TableBuilder::new(ui)
        // Not the id of the earlier fixed-width layout, whose remembered
        // widths left no room for values.
        .id_salt("variables_table")
        .striped(true)
        .resizable(true)
        .auto_shrink([false, false])
        // The default minimum (200 points) made the table grow over the
        // detail view in a short pane; it scrolls instead.
        .min_scrolled_height(0.0)
        .sense(egui::Sense::click())
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(name_width).at_least(40.0).clip(true))
        .column(Column::initial(type_width).at_least(36.0).clip(true))
        .column(Column::remainder().at_least(MIN_VALUE_WIDTH).clip(true))
        .header(row_height + 2.0, |mut header| {
            for title in ["Name", "Type", "Value"] {
                header.col(|ui| {
                    ui.strong(title);
                });
            }
        })
        .body(|body| {
            body.rows(row_height, rows.len(), |mut row| {
                let variable = rows[row.index()];
                let is_selected = selected.as_deref() == Some(variable.name.as_str());
                row.set_selected(is_selected);
                // Not selectable: a selectable label takes the clicks on
                // its text away from the row.
                row.col(|ui| {
                    ui.add(egui::Label::new(&variable.name).selectable(false));
                });
                row.col(|ui| {
                    ui.add(
                        egui::Label::new(RichText::new(&variable.type_name).weak())
                            .selectable(false),
                    );
                });
                row.col(|ui| {
                    ui.add(
                        egui::Label::new(RichText::new(&variable.value).monospace())
                            .truncate()
                            .selectable(false),
                    );
                });
                let response = row.response();
                // Selects; the detail view's × closes it.
                if response.clicked() {
                    *selected = Some(variable.name.clone());
                }
                response
                    .on_hover_text("Click to see the whole value")
                    .context_menu(|ui| {
                        if ui.button(format!("{} Copy value", icon::COPY)).clicked() {
                            ui.ctx().copy_text(variable.value.clone());
                            ui.close();
                        }
                        if ui.button(format!("{} Copy name", icon::COPY)).clicked() {
                            ui.ctx().copy_text(variable.name.clone());
                            ui.close();
                        }
                    });
            });
        })
        .inner_rect
}

/// The whole value of `variable`. Returns `false` once closed.
fn detail_view(ui: &mut egui::Ui, variable: &Variable) -> bool {
    let mut open = true;
    ui.horizontal(|ui| {
        ui.strong(&variable.name);
        ui.weak(&variable.type_name);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button(icon::X).on_hover_text("Close").clicked() {
                open = false;
            }
            if ui
                .button(icon::COPY)
                .on_hover_text("Copy the value")
                .clicked()
            {
                ui.ctx().copy_text(variable.value.clone());
            }
        });
    });
    egui::ScrollArea::vertical()
        .id_salt("variable_detail_scroll")
        .auto_shrink([false, false])
        // The default minimum (64 points) would grow the panel over the
        // table in a short pane.
        .min_scrolled_height(0.0)
        .show(ui, |ui| {
            ui.add(
                egui::Label::new(RichText::new(&variable.value).monospace())
                    .wrap()
                    .selectable(true),
            );
        });
    open
}

#[cfg(test)]
#[path = "../../tests/rust/panels/test_variables.rs"]
mod tests;
