//! The tab strip above the editor, shown while more than one document is
//! open: a click shows a tab, the middle button or its × closes it, and its
//! context menu closes others or makes it the project's main script.

use egui::RichText;
use egui_phosphor::regular as icon;

use crate::app::TabInfo;
use crate::commands::{self, Action};
use crate::project;

pub fn ui(ui: &mut egui::Ui, tabs: &[TabInfo], actions: &mut Vec<Action>) {
    let shown = tabs.iter().find(|tab| tab.active).map(|tab| tab.generation);
    // Scrolls to the shown tab when another one is shown.
    let memory = ui.id().with("editor_tabs_shown");
    let changed = ui.data_mut(|data| {
        let previous = data.get_temp::<Option<u64>>(memory);
        data.insert_temp(memory, shown);
        previous != Some(shown)
    });

    egui::ScrollArea::horizontal()
        .id_salt("editor_tabs")
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for tab in tabs {
                    tab_ui(ui, tab, tabs.len(), changed, actions);
                }
            });
        });
    ui.add_space(2.0);
}

fn tab_ui(
    ui: &mut egui::Ui,
    tab: &TabInfo,
    count: usize,
    scroll_to_shown: bool,
    actions: &mut Vec<Action>,
) {
    let glyph = if tab.main {
        icon::PLAY_CIRCLE
    } else if tab.path.as_deref().is_none_or(project::is_script) {
        icon::FILE_PY
    } else {
        icon::FILE_TEXT
    };
    let modified = if tab.modified { " •" } else { "" };
    let mut text = RichText::new(format!("{glyph} {}{modified}", tab.name));
    if tab.active {
        text = text.strong();
    }
    let response = ui.selectable_label(tab.active, text);
    let mut hint = tab.path.as_ref().map_or_else(
        || "Not saved yet".to_owned(),
        |path| path.display().to_string(),
    );
    if tab.main {
        hint.push_str("\nThe project's main script: F5 renders it");
    }
    let response = response.on_hover_text(hint);
    if tab.active && scroll_to_shown {
        response.scroll_to_me(None);
    }
    if response.clicked() {
        actions.push(Action::ActivateTab(tab.generation));
    }
    if response.middle_clicked() {
        actions.push(Action::CloseDocument(tab.generation));
    }
    response.context_menu(|ui| menu(ui, tab, count, actions));

    let close = ui
        .add(egui::Button::new(RichText::new(icon::X).small()).frame(false))
        .on_hover_text(match commands::shortcut(&Action::CloseTab) {
            Some(shortcut) if tab.active => {
                format!("Close ({})", ui.ctx().format_shortcut(&shortcut))
            }
            _ => "Close".to_owned(),
        });
    if close.clicked() {
        actions.push(Action::CloseDocument(tab.generation));
    }
    ui.add_space(6.0);
}

fn menu(ui: &mut egui::Ui, tab: &TabInfo, count: usize, actions: &mut Vec<Action>) {
    let mut item = |ui: &mut egui::Ui, label: &str, action: Action, enabled: bool| {
        if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
            actions.push(action);
            ui.close();
        }
    };
    item(ui, "Close", Action::CloseDocument(tab.generation), true);
    item(
        ui,
        "Close Others",
        Action::CloseOtherTabs(tab.generation),
        count > 1,
    );
    item(ui, "Close All", Action::CloseAllTabs, true);
    if let Some(path) = &tab.path {
        ui.separator();
        if project::is_script(path) {
            if tab.main {
                item(ui, "Clear Main Script", Action::SetMainScript(None), true);
            } else {
                item(
                    ui,
                    "Set as Main Script",
                    Action::SetMainScript(Some(path.clone())),
                    true,
                );
            }
        }
        if ui.button("Copy Path").clicked() {
            ui.ctx().copy_text(path.display().to_string());
            ui.close();
        }
    }
}
