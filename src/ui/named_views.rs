//! The named views of the document, as a toolbar popup and a View submenu.

use egui_phosphor::regular as icon;

use crate::app::AppState;
use crate::commands::Action;

/// The full list: go to, update or delete each view, and save the current
/// camera under a typed name.
pub fn manager(ui: &mut egui::Ui, state: &AppState, actions: &mut Vec<Action>) {
    ui.set_min_width(240.0);
    ui.strong("Named views");
    let views = &state.viewer.named_views;
    if views.is_empty() {
        ui.weak("None yet: save the current view below.");
    }
    for (index, view) in views.iter().enumerate() {
        ui.horizontal(|ui| {
            let go = ui
                .add(egui::Button::new(&view.name).frame(false))
                .on_hover_text(format!(
                    "Go to this view ({})",
                    view.projection.label().to_lowercase()
                ));
            if go.clicked() {
                actions.push(Action::GoToView(index));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button(icon::TRASH)
                    .on_hover_text("Delete")
                    .clicked()
                {
                    actions.push(Action::DeleteView(index));
                }
                if ui
                    .small_button(icon::ARROWS_CLOCKWISE)
                    .on_hover_text("Replace with the current view")
                    .clicked()
                {
                    actions.push(Action::UpdateView(index));
                }
            });
        });
    }
    ui.separator();
    let id = ui.id().with("new_view_name");
    let mut name = ui.data_mut(|data| data.get_temp::<String>(id).unwrap_or_default());
    let mut save = false;
    ui.horizontal(|ui| {
        let field = ui.add(
            egui::TextEdit::singleline(&mut name)
                .hint_text("Name")
                .desired_width(150.0),
        );
        save |= field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
        save |= ui
            .button(format!("{} Save", icon::BOOKMARK_SIMPLE))
            .on_hover_text("Save the current view; an existing name is replaced")
            .clicked();
    });
    if save {
        actions.push(Action::SaveView(
            Some(name.trim().to_owned()).filter(|name| !name.is_empty()),
        ));
        name.clear();
    }
    ui.data_mut(|data| data.insert_temp(id, name));
}

/// View > Named Views: the views to go to, and saving under the next
/// free name.
pub fn submenu(ui: &mut egui::Ui, state: &AppState, actions: &mut Vec<Action>) {
    for (index, view) in state.viewer.named_views.iter().enumerate() {
        if ui.button(&view.name).clicked() {
            actions.push(Action::GoToView(index));
            ui.close();
        }
    }
    if !state.viewer.named_views.is_empty() {
        ui.separator();
    }
    if ui
        .button(format!("{} Save Current View", icon::BOOKMARK_SIMPLE))
        .on_hover_text("As \"View N\"; rename or delete views from the toolbar")
        .clicked()
    {
        actions.push(Action::SaveView(None));
        ui.close();
    }
}
