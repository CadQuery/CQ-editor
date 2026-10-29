//! Source symbols and static references across the open project.

use egui_phosphor::regular as icon;

use crate::app::AppState;
use crate::commands::Action;
use crate::worker::protocol::{NavigationLocation, NavigationMode, OutlineSymbol, RequestId};

#[derive(Default)]
pub struct NavigationState {
    pub symbols: Vec<OutlineSymbol>,
    pub locations: Vec<NavigationLocation>,
    pub mode: Option<NavigationMode>,
    pub document: u64,
    pub source: String,
    pub outline_request: Option<RequestId>,
    pub navigation_request: Option<RequestId>,
    pub navigation_source: String,
    pub navigation_document: u64,
    pub truncated: bool,
    pub available: bool,
    pub filter: String,
}

pub fn ui(ui: &mut egui::Ui, state: &mut AppState) {
    if state.navigation.document != state.document_generation
        || state.navigation.source != state.document.text
    {
        state.request_outline();
    }
    ui.horizontal(|ui| {
        ui.label("Symbols");
        if ui
            .small_button(icon::ARROW_CLOCKWISE)
            .on_hover_text("Refresh symbols")
            .clicked()
        {
            state.request_outline();
        }
    });
    ui.add(egui::TextEdit::singleline(&mut state.navigation.filter).hint_text("Filter symbols"));
    ui.separator();
    let filter = state.navigation.filter.to_lowercase();
    let mut actions = Vec::new();
    egui::ScrollArea::vertical().show(ui, |ui| {
        for symbol in &state.navigation.symbols {
            if !symbol.name.to_lowercase().contains(&filter) {
                continue;
            }
            let glyph = match symbol.kind.as_str() {
                "class" => icon::CUBE,
                "function" => icon::FUNCTION,
                _ => icon::BRACKETS_CURLY,
            };
            ui.horizontal(|ui| {
                ui.add_space(symbol.depth.min(8) as f32 * 12.0);
                if ui
                    .link(format!("{glyph}  {}  :{}", symbol.name, symbol.line))
                    .clicked()
                {
                    actions.push(Action::GoToScriptLine {
                        document: state.navigation.document,
                        line: symbol.line,
                    });
                }
            });
        }
        if let Some(mode) = state.navigation.mode {
            ui.separator();
            let label = if mode == NavigationMode::Definition {
                "Definitions"
            } else {
                "References"
            };
            ui.strong(format!("{label} ({})", state.navigation.locations.len()));
            if !state.navigation.available {
                ui.label("Install jedi in the selected Python environment to navigate code.");
            } else if state.navigation.locations.is_empty() {
                ui.label("No locations found.");
            }
            for location in &state.navigation.locations {
                let path = location.path.as_deref().unwrap_or("Current document");
                let file = std::path::Path::new(path)
                    .file_name()
                    .map_or(path.into(), |name| name.to_string_lossy());
                if ui
                    .link(format!(
                        "{} · {}:{} · {} ({})",
                        file,
                        location.line,
                        location.column + 1,
                        location.name,
                        location.kind
                    ))
                    .on_hover_text(path)
                    .clicked()
                {
                    if let Some(path) = &location.path {
                        actions.push(Action::OpenFileAt {
                            path: path.into(),
                            line: location.line,
                        });
                    } else {
                        actions.push(Action::GoToScriptLine {
                            document: state.navigation.navigation_document,
                            line: location.line,
                        });
                    }
                }
            }
            if state.navigation.truncated {
                ui.label("Showing the first 200 locations.");
            }
        }
    });
    state.queue(actions);
}
