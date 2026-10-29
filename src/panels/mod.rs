//! The dockable panels and their default arrangement.

mod checks;
mod console;
mod inspector;
mod log;
mod measurements;
mod navigation;
mod objects;
mod parameters;
mod project;
mod traceback;
mod variables;

pub use console::{ConsoleState, EntryKind};
pub use inspector::InspectorState;
pub use log::LogView;
pub use measurements::{
    MeasurementState, SelectorPreviewState, SelectorState, SelectorUse, format_measurement,
};
pub use navigation::NavigationState;
pub use parameters::ParametersState;

use egui_dock::{DockState, NodeIndex, NodePath, TabViewer};
use egui_phosphor::regular as icon;
use serde::{Deserialize, Serialize};

use crate::app::AppState;
use crate::commands::Action;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tab {
    Editor,
    Viewer,
    Objects,
    Measurements,
    Checks,
    Navigation,
    Parameters,
    Variables,
    Inspector,
    Console,
    Traceback,
    Log,
    Project,
}

impl Tab {
    pub const ALL: [Tab; 13] = [
        Tab::Editor,
        Tab::Viewer,
        Tab::Objects,
        Tab::Measurements,
        Tab::Checks,
        Tab::Navigation,
        Tab::Parameters,
        Tab::Variables,
        Tab::Inspector,
        Tab::Console,
        Tab::Traceback,
        Tab::Log,
        Tab::Project,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Editor => "Editor",
            Tab::Viewer => "3D View",
            Tab::Objects => "Objects",
            Tab::Measurements => "Measurements",
            Tab::Checks => "Checks",
            Tab::Navigation => "Navigation",
            Tab::Parameters => "Parameters",
            Tab::Variables => "Variables",
            Tab::Inspector => "CQ Inspector",
            Tab::Console => "Console",
            Tab::Traceback => "Traceback",
            Tab::Log => "Log",
            Tab::Project => "Project",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Tab::Editor => icon::CODE,
            Tab::Viewer => icon::CUBE,
            Tab::Objects => icon::TREE_STRUCTURE,
            Tab::Measurements => icon::RULER,
            Tab::Checks => icon::CHECKS,
            Tab::Navigation => icon::TREE_STRUCTURE,
            Tab::Parameters => icon::SLIDERS_HORIZONTAL,
            Tab::Variables => icon::BRACKETS_CURLY,
            Tab::Inspector => icon::STACK,
            Tab::Console => icon::TERMINAL_WINDOW,
            Tab::Traceback => icon::WARNING_CIRCLE,
            Tab::Log => icon::LIST_BULLETS,
            Tab::Project => icon::FOLDER_SIMPLE,
        }
    }
}

/// Editor on the left, 3D view in the middle, objects and variables on the
/// right and console, traceback and log at the bottom, like the original.
/// The Project panel joins it, left of the editor, once a folder is open.
pub fn default_layout() -> DockState<Tab> {
    let mut dock = DockState::new(vec![Tab::Viewer]);
    let tree = dock.main_surface_mut();
    // Fractions give the share of the left (or top) child of each split.
    let [top, bottom] = tree.split_below(NodeIndex::root(), 0.74, vec![Tab::Console]);
    tree.split_right(bottom, 0.45, vec![Tab::Checks, Tab::Traceback, Tab::Log]);
    let [viewer, _editor] = tree.split_left(top, 0.36, vec![Tab::Editor]);
    let [_viewer, objects] = tree.split_right(viewer, 0.72, vec![Tab::Objects]);
    tree.split_below(
        objects,
        0.6,
        vec![
            Tab::Variables,
            Tab::Parameters,
            Tab::Inspector,
            Tab::Measurements,
        ],
    );
    dock
}

/// Shows the panels; holds what they need from the application.
pub struct TabContext<'a> {
    pub state: &'a mut AppState,
    pub frame: &'a eframe::Frame,
    /// One panel fills the window (see `Action::ToggleMaximize`).
    pub maximized: bool,
}

impl TabViewer for TabContext<'_> {
    type Tab = Tab;

    fn id(&mut self, tab: &mut Tab) -> egui::Id {
        egui::Id::new(*tab)
    }

    fn title(&mut self, tab: &mut Tab) -> egui::WidgetText {
        let mut title = format!("{}  {}", tab.icon(), tab.title());
        match tab {
            // With several files open their names are on the tab strip.
            Tab::Editor if self.state.tabs().len() > 1 => {
                if self.state.any_modified() {
                    title.push_str(" •");
                }
            }
            Tab::Editor => {
                title = format!("{}  {}", tab.icon(), self.state.document.display_name());
                if self.state.document.is_modified() {
                    title.push_str(" •");
                }
            }
            Tab::Traceback if self.state.traceback.is_some() => title.push_str(" •"),
            Tab::Checks if self.state.cad_checks.iter().any(|check| !check.ok) => {
                title.push_str(" •");
            }
            Tab::Project => {
                if let Some(project) = &self.state.project {
                    title = format!("{}  {}", tab.icon(), project.name());
                }
            }
            _ => {}
        }
        title.into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Tab) {
        constrain_to_tab(ui);
        match tab {
            Tab::Editor => self.state.editor_ui(ui),
            Tab::Viewer => self.state.viewer_ui(ui, self.frame),
            Tab::Objects => objects::ui(ui, self.state),
            Tab::Measurements => measurements::ui(ui, self.state),
            Tab::Checks => checks::ui(ui, self.state),
            Tab::Navigation => navigation::ui(ui, self.state),
            Tab::Parameters => parameters::ui(ui, self.state),
            Tab::Variables => variables::ui(ui, self.state),
            Tab::Inspector => inspector::ui(ui, self.state),
            Tab::Console => console::ui(ui, self.state),
            Tab::Traceback => traceback::ui(ui, self.state),
            Tab::Log => log::ui(ui, self.state),
            Tab::Project => project::ui(ui, self.state),
        }
    }

    fn on_tab_button(&mut self, tab: &mut Tab, response: &egui::Response) {
        if response.double_clicked() {
            self.state.queue([Action::ToggleMaximize(Some(*tab))]);
        }
    }

    /// Added above the dock's own Float and Close items.
    fn context_menu(&mut self, ui: &mut egui::Ui, tab: &mut Tab, _path: NodePath) {
        let (label, glyph) = if self.maximized {
            ("Restore Layout", icon::ARROWS_IN)
        } else {
            ("Maximize", icon::ARROWS_OUT)
        };
        let mut button = egui::Button::new(format!("{glyph}  {label}"));
        if let Some(shortcut) = crate::commands::shortcut(&Action::ToggleMaximize(None)) {
            button = button.shortcut_text(ui.ctx().format_shortcut(&shortcut));
        }
        if ui
            .add(button)
            .on_hover_text("Double-clicking the tab does the same")
            .clicked()
        {
            self.state.queue([Action::ToggleMaximize(Some(*tab))]);
            ui.close();
        }
        if ui
            .button(format!("{}  Reset Layout", icon::LAYOUT))
            .clicked()
        {
            self.state.queue([Action::ResetLayout]);
            ui.close();
        }
        ui.separator();
    }

    fn is_closeable(&self, tab: &Tab) -> bool {
        *tab != Tab::Viewer
    }

    fn scroll_bars(&self, _tab: &Tab) -> [bool; 2] {
        // Every panel scrolls its own content.
        [false, false]
    }
}

/// Keep every panel's paint inside the body assigned by `egui_dock`.
///
/// The dock creates tab bodies as fresh [`egui::Ui`]s. Their maximum rectangle
/// is the tab body, but their inherited clip rectangle can be as large as the
/// whole dock area. Most containers narrow it again; widgets that do not can
/// otherwise paint text over a split separator or a neighbouring tab bar.
fn constrain_to_tab(ui: &mut egui::Ui) {
    ui.set_clip_rect(ui.clip_rect().intersect(ui.max_rect()));
}

/// Centered, dimmed text for empty panels.
///
/// Laid out with centered rows rather than a justified layout, which
/// stretched the spaces of wrapped lines unevenly.
pub(crate) fn empty_state(ui: &mut egui::Ui, icon: &str, text: &str) {
    let rect = ui.available_rect_before_wrap();
    ui.allocate_rect(rect, egui::Sense::hover());
    let color = ui.visuals().weak_text_color();
    let job = empty_state_job(ui.style(), &format!("{icon}  {text}"), color, rect.width());
    let galley = ui.painter().layout_job(job);
    // Centered rows are laid out around x = 0.
    let top = (rect.center().y - galley.size().y / 2.0).max(rect.top());
    ui.painter()
        .galley(egui::pos2(rect.center().x, top), galley, color);
}

fn empty_state_job(
    style: &egui::Style,
    text: &str,
    color: egui::Color32,
    width: f32,
) -> egui::text::LayoutJob {
    let font = egui::TextStyle::Body.resolve(style);
    let mut job =
        egui::text::LayoutJob::simple(text.to_owned(), font, color, (width - 24.0).max(40.0));
    job.halign = egui::Align::Center;
    job
}

#[cfg(test)]
#[path = "../../tests/rust/panels/test_empty_state.rs"]
mod tests;
