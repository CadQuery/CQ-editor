//! The command palette (Ctrl+Shift+P): a search over every command, so a
//! command can be found without knowing its menu, and run without the mouse.
//! It lists the commands of the menus, plus the open tabs, recent files and
//! folders, examples, panels and named views. Only the commands that can be
//! used right now are offered.

use egui::{Key, RichText};
use egui_phosphor::regular as icon;

use crate::app::AppState;
use crate::commands::{self, Action};
use crate::panels::Tab;
use crate::viewer::camera::StandardView;
use crate::viewer::{DisplayMode, SelectionMode};
use crate::worker::protocol::{DebugAction, ExportFormat};

/// Most commands listed at once; typing narrows them down.
const MAX_SHOWN: usize = 12;

/// The palette while it is open.
#[derive(Default)]
pub struct Palette {
    pub query: String,
    /// Index into the matches of `query`.
    pub selected: usize,
}

/// A command of the palette.
#[derive(Clone, Debug, PartialEq)]
pub struct Command {
    /// Where it is found otherwise, such as `File` or `Tab`.
    pub category: &'static str,
    pub label: String,
    pub action: Action,
}

impl Command {
    fn new(category: &'static str, label: impl Into<String>, action: Action) -> Self {
        Self {
            category,
            label: label.into(),
            action,
        }
    }

    /// What the query is matched against, and the identifier stored for
    /// favorite commands.
    pub fn id(&self) -> String {
        format!("{}: {}", self.category, self.label)
    }

    fn text(&self) -> String {
        self.id()
    }
}

/// The commands of the menus, whether usable now or not.
pub fn menu_commands() -> Vec<Command> {
    let c = Command::new;
    let mut list = vec![
        c("File", "New", Action::NewFile),
        c("File", "Open…", Action::OpenFile),
        c("File", "Open Folder…", Action::OpenFolder),
        c("File", "Save", Action::Save),
        c("File", "Save As…", Action::SaveAs),
        c("File", "Save All", Action::SaveAll),
        c("File", "Local History…", Action::LocalHistory),
        c("File", "Close Tab", Action::CloseTab),
        c("File", "Close All Tabs", Action::CloseAllTabs),
        c("File", "Close Folder", Action::CloseFolder),
        c("File", "Refresh Project Folder", Action::RefreshProject),
        c(
            "File",
            "Automatic Reload and Preview",
            Action::ToggleAutoreload,
        ),
        c("File", "Quit", Action::Quit),
        c("Tab", "Next Tab", Action::NextTab),
        c("Tab", "Previous Tab", Action::PreviousTab),
        c("Edit", "Undo", Action::Undo),
        c("Edit", "Redo", Action::Redo),
        c("Edit", "Toggle Comment", Action::ToggleComment),
        c("Edit", "Duplicate Line", Action::DuplicateLine),
        c("Edit", "Move Line Up", Action::MoveLineUp),
        c("Edit", "Move Line Down", Action::MoveLineDown),
        c("Edit", "Auto-Complete", Action::Autocomplete),
        c("Edit", "Search", Action::Find),
        c("Edit", "Find Next", Action::FindNext),
        c("Edit", "Find Previous", Action::FindPrevious),
        c("Edit", "Replace", Action::Replace),
        c("Edit", "Go to Line…", Action::GoToLinePrompt),
        c("Edit", "Show Outline", Action::ShowOutline),
        c("Edit", "Go to Definition", Action::GoToDefinition),
        c("Edit", "Find References", Action::FindReferences),
        c("Edit", "Copy Selector", Action::CopySelector),
        c("Edit", "Insert Selector", Action::InsertSelector),
        c(
            "Edit",
            "Copy Measurement as Code",
            Action::CopyMeasurementCode,
        ),
        c(
            "Edit",
            "Copy Measurement as Text",
            Action::CopyMeasurement(commands::MeasurementFormat::Text),
        ),
        c(
            "Edit",
            "Copy Measurement as JSON",
            Action::CopyMeasurement(commands::MeasurementFormat::Json),
        ),
        c(
            "Edit",
            "Copy Measurement as CSV",
            Action::CopyMeasurement(commands::MeasurementFormat::Csv),
        ),
        c("Edit", "Reset Modified Parameters", Action::ResetParameters),
        c("Edit", "Go to Source", Action::GoToSelectionSource),
        c("Edit", "Preferences…", Action::Preferences),
        c("Run", "Render", Action::Render),
        c("Run", "Profile", Action::Profile),
        c("Run", "Clear Profile", Action::ClearProfile),
        c("Run", "Debug / Stop Debugging", Action::ToggleDebug),
        c("Run", "Step", Action::Debug(DebugAction::Step)),
        c("Run", "Step In", Action::Debug(DebugAction::StepIn)),
        c("Run", "Step Out", Action::Debug(DebugAction::StepOut)),
        c("Run", "Continue", Action::Debug(DebugAction::Continue)),
        c("Run", "Stop", Action::Stop),
        c("Run", "Clear Console", Action::ClearConsole),
        c("Tools", "Check Geometry", Action::CheckGeometry),
        c("Run", "Reset Console", Action::ResetConsole),
        c("Run", "Clear Log", Action::ClearLog),
        c("Run", "Restart Python", Action::RestartPython),
        c(
            "Run",
            "Create Python Environment…",
            Action::CreateEnvironment,
        ),
        c("Run", "Install Packages…", Action::InstallPackages),
        c("Tools", "Inspect CQ Object", Action::ToggleInspect),
        c("Tools", "Screenshot…", Action::Screenshot),
        c("Tools", "Copy Screenshot to Clipboard", Action::CopyImage),
        c("Tools", "Upload Model…", Action::Upload),
        c("View", "Fit", Action::Fit),
        c("View", "Zoom to Selection", Action::FitSelection),
        c("View", "Perspective Projection", Action::ToggleProjection),
        c("View", "Section View", Action::ToggleSection),
        c("View", "Exploded View", Action::ToggleExplode),
        c("View", "Ground Grid", Action::ToggleGroundGrid),
        c("View", "Scale Bar", Action::ToggleScaleBar),
        c(
            "View",
            "Isolate Selection / End Isolation",
            Action::ToggleIsolate,
        ),
        c("View", "Show All", Action::ShowAll),
        c("View", "Save Current View", Action::SaveView(None)),
        c(
            "View",
            "Maximize Panel / Restore Layout",
            Action::ToggleMaximize(None),
        ),
        c("View", "Reset Layout", Action::ResetLayout),
        c("View", "Command Palette…", Action::CommandPalette),
        c("Objects", "Clear All Objects", Action::ClearObjects),
        c("Objects", "Remove Selected Objects", Action::RemoveSelected),
        c("Help", "Documentation", Action::Documentation),
        c("Help", "Installation Guide", Action::InstallationGuide),
        c(
            "Help",
            "CadQuery Documentation",
            Action::CadQueryDocumentation,
        ),
        c("Help", "Check for Updates", Action::CheckUpdates),
        c("Help", "Download the New Release…", Action::DownloadUpdate),
        c("Help", "About", Action::About),
    ];
    list.extend(
        StandardView::ALL
            .into_iter()
            .map(|view| Command::new("View", format!("{} View", view.label()), Action::View(view))),
    );
    list.extend(
        DisplayMode::ALL
            .into_iter()
            .map(|mode| Command::new("Display", mode.label(), Action::Display(mode))),
    );
    list.extend(SelectionMode::ALL.into_iter().map(|mode| {
        Command::new(
            "Select",
            format!("Select {}", mode.label()),
            Action::SelectionMode(mode),
        )
    }));
    list.extend(ExportFormat::ALL.into_iter().map(|format| {
        Command::new(
            "Tools",
            format!("Export as {}…", format.label()),
            Action::Export(format),
        )
    }));
    list.extend(
        Tab::ALL
            .into_iter()
            .filter(|tab| *tab != Tab::Viewer)
            .map(|tab| {
                Command::new(
                    "Panel",
                    format!("Show / Hide {}", tab.title()),
                    Action::TogglePanel(tab),
                )
            }),
    );
    list.extend(
        crate::editor::snippets::SNIPPETS
            .iter()
            .enumerate()
            .map(|(index, snippet)| {
                Command::new(
                    "Snippet",
                    format!("Insert {}", snippet.title),
                    Action::InsertSnippet(index),
                )
            }),
    );
    list
}

/// Whether `action` can be used now; the palette hides the others, as the
/// menus disable them.
fn available(state: &AppState, action: &Action) -> bool {
    let picked = !state.scene.topology_selections.is_empty();
    let has_objects = !state.scene.is_empty();
    match action {
        Action::Undo => state.editor.can_undo(),
        Action::Redo => state.editor.can_redo(),
        Action::SaveAll => state.any_modified(),
        Action::CloseFolder | Action::RefreshProject => state.project.is_some(),
        Action::CopySelector | Action::InsertSelector | Action::CopyMeasurementCode => picked,
        Action::CopyMeasurement(_) => state.measurements.result.is_some(),
        Action::GoToSelectionSource => state.scene.current().is_some_and(|n| n.line.is_some()),
        Action::ClearProfile => !state.editor.line_profile.is_empty(),
        Action::Debug(_) => state.is_paused(),
        Action::Stop => state.can_stop(),
        Action::Export(_) | Action::ClearObjects | Action::FitSelection => has_objects,
        Action::RemoveSelected => !state.scene.selection.is_empty(),
        Action::ToggleIsolate => state.scene.is_isolating() || !state.scene.selection.is_empty(),
        Action::DownloadUpdate => crate::updates::available(&state.settings.general).is_some(),
        Action::CommandPalette => false,
        _ => true,
    }
}

/// Every command usable now: those of the menus, then the open tabs,
/// named views, recent files and folders and the examples.
pub fn commands(state: &AppState) -> Vec<Command> {
    let mut list: Vec<Command> = menu_commands()
        .into_iter()
        .filter(|command| available(state, &command.action))
        .collect();
    for tab in state.tabs().into_iter().filter(|tab| !tab.active) {
        list.push(Command::new(
            "Tab",
            tab.name,
            Action::ActivateTab(tab.generation),
        ));
    }
    for (index, view) in state.viewer.named_views.iter().enumerate() {
        list.push(Command::new(
            "Named View",
            &view.name,
            Action::GoToView(index),
        ));
    }
    for path in &state.settings.recent_files {
        let name = path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        list.push(Command::new(
            "Open Recent",
            name,
            Action::OpenPath(path.clone()),
        ));
    }
    for project in &state.settings.projects {
        let name = project.root.file_name().map_or_else(
            || project.root.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        list.push(Command::new(
            "Open Recent Folder",
            name,
            Action::OpenFolderPath(project.root.clone()),
        ));
    }
    for (index, example) in state.examples().iter().enumerate() {
        list.push(Command::new(
            "Example",
            &example.title,
            Action::OpenExample(index),
        ));
    }
    list
}

/// How well `text` matches `query`: every character of the query, in
/// order and regardless of case, with more points for characters at the
/// start of a word or right after the previous match, and for the whole
/// query in one piece. `None` when it does not match.
pub fn score(query: &str, text: &str) -> Option<i32> {
    let query: Vec<char> = query
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    if query.is_empty() {
        return Some(0);
    }
    let text: Vec<char> = text.chars().flat_map(char::to_lowercase).collect();
    let mut score = 0;
    let mut next = 0;
    let mut previous: Option<usize> = None;
    for wanted in &query {
        let found = (next..text.len()).find(|&i| text[i] == *wanted)?;
        score += 1;
        if found == 0 || !text[found - 1].is_alphanumeric() {
            score += 8;
        }
        match previous {
            Some(previous) if previous + 1 == found => score += 5,
            Some(previous) => score -= (found - previous - 1).min(5) as i32,
            None => {}
        }
        previous = Some(found);
        next = found + 1;
    }
    let joined: String = query.iter().collect();
    if text.iter().collect::<String>().contains(&joined) {
        score += 20;
    }
    Some(score)
}

/// The commands matching `query`, best first; equally good ones keep their
/// order.
#[cfg(test)]
pub fn matches(commands: Vec<Command>, query: &str) -> Vec<Command> {
    matches_with_favorites(commands, query, &[])
}

/// The commands matching `query`, prioritizing `favorites` at the top when
/// the query is empty (and breaking score ties in favor of favorites).
pub fn matches_with_favorites(
    commands: Vec<Command>,
    query: &str,
    favorites: &[String],
) -> Vec<Command> {
    let is_fav = |cmd: &Command| favorites.iter().any(|fav| fav == &cmd.id());
    if query.trim().is_empty() {
        let mut favs: Vec<Command> = Vec::new();
        let mut rest: Vec<Command> = Vec::new();
        for cmd in commands {
            if is_fav(&cmd) {
                favs.push(cmd);
            } else {
                rest.push(cmd);
            }
        }
        // Keep favorites in the order they were pinned.
        favs.sort_by_key(|cmd| {
            favorites
                .iter()
                .position(|fav| fav == &cmd.id())
                .unwrap_or(usize::MAX)
        });
        favs.extend(rest);
        return favs;
    }

    let mut scored: Vec<((i32, bool), Command)> = commands
        .into_iter()
        .filter_map(|command| {
            let s = score(query, &command.text())?;
            let fav = is_fav(&command);
            Some(((s, fav), command))
        })
        .collect();
    scored.sort_by_key(|(key, _)| std::cmp::Reverse(*key));
    scored.into_iter().map(|(_, command)| command).collect()
}

/// Shows the palette at the top of the window. Returns the command chosen,
/// or `Some(None)` once it was dismissed.
pub fn window(
    ctx: &egui::Context,
    palette: &mut Palette,
    commands: Vec<Command>,
    settings: &mut crate::settings::Settings,
) -> Option<Option<Action>> {
    let found = matches_with_favorites(commands, &palette.query, &settings.favorite_commands);
    palette.selected = palette.selected.min(found.len().saturating_sub(1));
    let mut chosen = None;
    let (up, down, enter, toggle_fav) = ctx.input_mut(|input| {
        (
            input.consume_key(egui::Modifiers::NONE, Key::ArrowUp),
            input.consume_key(egui::Modifiers::NONE, Key::ArrowDown),
            input.consume_key(egui::Modifiers::NONE, Key::Enter),
            input.consume_key(egui::Modifiers::COMMAND, Key::D),
        )
    });
    if up {
        palette.selected = palette.selected.saturating_sub(1);
    }
    if down {
        palette.selected = (palette.selected + 1).min(found.len().saturating_sub(1));
    }
    if toggle_fav && let Some(command) = found.get(palette.selected) {
        settings.toggle_favorite_command(&command.id());
    }
    if enter && let Some(command) = found.get(palette.selected) {
        chosen = Some(command.action.clone());
    }

    let area = egui::Modal::default_area(egui::Id::new("command_palette"))
        .anchor(egui::Align2::CENTER_TOP, [0.0, 60.0]);
    let response = egui::Modal::new(egui::Id::new("command_palette"))
        .area(area)
        .frame(egui::Frame::popup(&ctx.style_of(ctx.theme())).inner_margin(10))
        .show(ctx, |ui| {
            ui.set_width(520.0);
            let field = ui.add(
                egui::TextEdit::singleline(&mut palette.query)
                    .hint_text("Type a command, a file, a snippet or a panel")
                    .desired_width(f32::INFINITY),
            );
            if !field.has_focus() {
                field.request_focus();
            }
            if field.changed() {
                palette.selected = 0;
            }
            ui.add_space(4.0);
            if found.is_empty() {
                ui.weak("No command matches");
            }
            let empty_query = palette.query.trim().is_empty();
            // The page of results around the selected one.
            let first = palette.selected.saturating_sub(MAX_SHOWN - 1);
            let mut prev_was_favorite = false;
            for (index, command) in found.iter().enumerate().skip(first).take(MAX_SHOWN) {
                let id = command.id();
                let is_favorite = settings.is_favorite_command(&id);
                if empty_query && index > first && prev_was_favorite && !is_favorite {
                    ui.separator();
                }
                prev_was_favorite = is_favorite;
                let selected = index == palette.selected;
                let mut star_clicked = false;
                let row = ui
                    .horizontal(|ui| {
                        ui.set_min_width(ui.available_width());
                        let star_text = if is_favorite {
                            RichText::new("★").color(egui::Color32::from_rgb(235, 180, 40))
                        } else {
                            RichText::new("☆").weak()
                        };
                        let star_hint = if is_favorite {
                            "Remove from Favorite Commands (Ctrl+D)"
                        } else {
                            "Add to Favorite Commands (Ctrl+D)"
                        };
                        if ui
                            .add(egui::Button::new(star_text).frame(false))
                            .on_hover_text(star_hint)
                            .clicked()
                        {
                            star_clicked = true;
                        }
                        let label = ui.selectable_label(selected, &command.label);
                        ui.weak(command.category);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if let Some(shortcut) = commands::shortcut(&command.action) {
                                ui.weak(ctx.format_shortcut(&shortcut));
                            }
                        });
                        label
                    })
                    .inner;
                if star_clicked {
                    settings.toggle_favorite_command(&id);
                } else if row.clicked() {
                    chosen = Some(command.action.clone());
                }
                if row.hovered() && ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO) {
                    palette.selected = index;
                }
            }
            ui.add_space(4.0);
            ui.label(
                RichText::new(format!(
                    "{}  ↑↓ choose · Enter runs · Ctrl+D ★ favorite · Esc closes",
                    icon::KEYBOARD
                ))
                .small()
                .weak(),
            );
        });
    if chosen.is_some() {
        return Some(chosen);
    }
    response.should_close().then_some(None)
}

#[cfg(test)]
#[path = "../../tests/rust/app/test_command_palette.rs"]
mod tests;
