//! The Project panel: the project folder's file tree. A click opens a
//! script in a tab or folds a folder; the context menus create, rename and
//! delete files and folders and choose the main script, which F5 runs.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use egui::{Align2, FontId, RichText, Sense, Vec2};
use egui_phosphor::regular as icon;

use crate::app::AppState;
use crate::commands::Action;
use crate::project::{self, Row};

/// Indentation of each level of the tree, in points.
const INDENT: f32 = 14.0;

/// How an open file shows in the tree.
#[derive(Clone, Copy)]
struct OpenFile {
    active: bool,
    modified: bool,
}

pub fn ui(ui: &mut egui::Ui, state: &mut AppState) {
    let mut actions = Vec::new();
    let open: HashMap<PathBuf, OpenFile> = state
        .tabs()
        .into_iter()
        .filter_map(|tab| {
            let file = OpenFile {
                active: tab.active,
                modified: tab.modified,
            };
            Some((tab.path?, file))
        })
        .collect();
    let Some(project) = &mut state.project else {
        no_folder(ui, &mut actions);
        state.queue(actions);
        return;
    };
    let root = project.root().to_path_buf();
    let main = project.main.clone();

    egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(6, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let tool = |ui: &mut egui::Ui, glyph: &str, tip: &str| {
                    ui.add(egui::Button::new(glyph).frame(false))
                        .on_hover_text(tip)
                        .clicked()
                };
                if tool(ui, icon::FILE_PLUS, "New file") {
                    actions.push(Action::NewFileIn(root.clone()));
                }
                if tool(ui, icon::FOLDER_PLUS, "New folder") {
                    actions.push(Action::NewFolderIn(root.clone()));
                }
                if tool(ui, icon::ARROWS_CLOCKWISE, "Refresh") {
                    actions.push(Action::RefreshProject);
                }
                if tool(ui, icon::ARROWS_IN_LINE_VERTICAL, "Collapse all folders") {
                    project.collapse_all();
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if tool(ui, icon::X, "Close the folder") {
                        actions.push(Action::CloseFolder);
                    }
                });
            });
            let main_text = match &main {
                Some(main) => format!(
                    "{}  F5 renders {}",
                    icon::PLAY,
                    project.relative(main).display()
                ),
                None => format!("{}  No main script: F5 renders the file shown", icon::PLAY),
            };
            ui.add(egui::Label::new(RichText::new(main_text).small().weak()).truncate())
                .on_hover_text(
                    "While a file of this folder is shown, F5 renders the main script. \
                     Right-click a script to choose it.",
                );
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let has_filter = !project.filter.is_empty();
                let width = (ui.available_width() - if has_filter { 22.0 } else { 0.0 }).max(40.0);
                ui.add(
                    egui::TextEdit::singleline(&mut project.filter)
                        .hint_text(format!("{} Filter files…", icon::MAGNIFYING_GLASS))
                        .desired_width(width),
                );
                if has_filter
                    && ui
                        .add(egui::Button::new(icon::X).frame(false))
                        .on_hover_text("Clear file search")
                        .clicked()
                {
                    project.filter.clear();
                }
            });
        });
    ui.separator();

    let searching = !project.filter.trim().is_empty();
    let rows = if searching {
        project.search_rows(&project.filter.clone())
    } else {
        project.rows()
    };
    if searching && rows.is_empty() {
        egui::Frame::NONE
            .inner_margin(egui::Margin::symmetric(8, 6))
            .show(ui, |ui| {
                ui.weak("No matching files");
            });
    }
    let row_height = ui.text_style_height(&egui::TextStyle::Body) + 6.0;
    let mut toggled = None;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show_rows(ui, row_height, rows.len(), |ui, range| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for row in &rows[range] {
                let file = open.get(&row.entry.path).copied();
                let is_main = main.as_deref() == Some(row.entry.path.as_path());
                let display_name = if searching {
                    project
                        .relative(&row.entry.path)
                        .to_string_lossy()
                        .replace('\\', "/")
                } else {
                    row.entry.name.clone()
                };
                let response = row_ui(ui, row, &display_name, row_height, file, is_main);
                if response.clicked() {
                    if row.entry.is_dir {
                        toggled = Some(row.entry.path.clone());
                    } else if project::is_editable(&row.entry.path) {
                        actions.push(Action::OpenPath(row.entry.path.clone()));
                    }
                }
                response.context_menu(|ui| {
                    context_menu(ui, row, &root, is_main, &mut actions);
                });
            }
        });
    if let Some(folder) = toggled {
        project.toggle(&folder);
    }

    // The empty space below the tree has the folder's own menu.
    let rest = ui.available_rect_before_wrap();
    if rest.height() > 0.0 {
        ui.allocate_rect(rest, Sense::click())
            .context_menu(|ui| folder_items(ui, &root, &mut actions));
    }
    state.queue(actions);
}

fn no_folder(ui: &mut egui::Ui, actions: &mut Vec<Action>) {
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() / 3.0);
        ui.label(
            RichText::new("Open a folder to see its scripts here")
                .color(ui.visuals().weak_text_color()),
        );
        ui.add_space(6.0);
        if ui
            .button(format!("{}  Open Folder…", icon::FOLDER_OPEN))
            .clicked()
        {
            actions.push(Action::OpenFolder);
        }
    });
}

/// One line of the tree, painted by hand so the whole width is a target.
fn row_ui(
    ui: &mut egui::Ui,
    row: &Row,
    display_name: &str,
    height: f32,
    file: Option<OpenFile>,
    is_main: bool,
) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::click());
    let visuals = ui.visuals();
    let active = file.is_some_and(|file| file.active);
    if active {
        ui.painter()
            .rect_filled(rect, 2.0, visuals.selection.bg_fill.gamma_multiply(0.6));
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect, 2.0, visuals.widgets.hovered.weak_bg_fill);
    }

    let entry = &row.entry;
    let caret = match (entry.is_dir, row.expanded) {
        (true, true) => icon::CARET_DOWN,
        (true, false) => icon::CARET_RIGHT,
        (false, _) => " ",
    };
    let glyph = if entry.is_dir {
        if row.expanded {
            icon::FOLDER_OPEN
        } else {
            icon::FOLDER
        }
    } else if is_main {
        icon::PLAY_CIRCLE
    } else if project::is_script(&entry.path) {
        icon::FILE_PY
    } else if project::is_editable(&entry.path) {
        icon::FILE_TEXT
    } else {
        icon::FILE
    };
    let editable = entry.is_dir || project::is_editable(&entry.path);
    let color = if is_main {
        visuals.strong_text_color()
    } else if editable {
        visuals.text_color()
    } else {
        visuals.weak_text_color()
    };
    let modified = if file.is_some_and(|file| file.modified) {
        " •"
    } else {
        ""
    };
    let font = FontId::proportional(ui.text_style_height(&egui::TextStyle::Body) * 0.95);
    let left = rect.left() + 4.0 + row.depth as f32 * INDENT;
    let painter = ui.painter_at(rect);
    painter.text(
        egui::pos2(left, rect.center().y),
        Align2::LEFT_CENTER,
        format!("{caret} {glyph}  {display_name}{modified}"),
        font,
        color,
    );

    let hint = if entry.is_dir {
        entry.path.display().to_string()
    } else if is_main {
        format!("{}\nThe main script: F5 renders it", entry.path.display())
    } else if editable {
        entry.path.display().to_string()
    } else {
        format!("{}\nNot a script or text file", entry.path.display())
    };
    response.on_hover_text_at_pointer(hint)
}

fn context_menu(
    ui: &mut egui::Ui,
    row: &Row,
    root: &Path,
    is_main: bool,
    actions: &mut Vec<Action>,
) {
    let path = &row.entry.path;
    let mut item = |ui: &mut egui::Ui, glyph: &str, label: &str, action: Action| {
        if ui.button(format!("{glyph}  {label}")).clicked() {
            actions.push(action);
            ui.close();
        }
    };
    if row.entry.is_dir {
        folder_items(ui, path, actions);
    } else {
        if project::is_editable(path) {
            item(
                ui,
                icon::FILE_ARROW_UP,
                "Open",
                Action::OpenPath(path.clone()),
            );
        }
        if project::is_script(path) {
            if is_main {
                item(
                    ui,
                    icon::PLAY_CIRCLE,
                    "Clear Main Script",
                    Action::SetMainScript(None),
                );
            } else {
                item(
                    ui,
                    icon::PLAY_CIRCLE,
                    "Set as Main Script",
                    Action::SetMainScript(Some(path.clone())),
                );
            }
        }
        ui.separator();
        let folder = path.parent().unwrap_or(root).to_path_buf();
        item(ui, icon::FILE_PLUS, "New File…", Action::NewFileIn(folder));
    }
    if path != root {
        ui.separator();
        let mut item = |ui: &mut egui::Ui, glyph: &str, label: &str, action: Action| {
            if ui.button(format!("{glyph}  {label}")).clicked() {
                actions.push(action);
                ui.close();
            }
        };
        item(
            ui,
            icon::PENCIL_SIMPLE,
            "Rename…",
            Action::RenamePath(path.clone()),
        );
        item(ui, icon::TRASH, "Delete…", Action::DeletePath(path.clone()));
    }
    ui.separator();
    if ui.button(format!("{}  Copy Path", icon::COPY)).clicked() {
        ui.ctx().copy_text(path.display().to_string());
        ui.close();
    }
    if ui
        .button(format!("{}  Show in File Manager", icon::FOLDER_OPEN))
        .clicked()
    {
        actions.push(Action::RevealPath(path.clone()));
        ui.close();
    }
}

/// What can be done in a folder.
fn folder_items(ui: &mut egui::Ui, folder: &Path, actions: &mut Vec<Action>) {
    if ui
        .button(format!("{}  New File…", icon::FILE_PLUS))
        .clicked()
    {
        actions.push(Action::NewFileIn(folder.to_path_buf()));
        ui.close();
    }
    if ui
        .button(format!("{}  New Folder…", icon::FOLDER_PLUS))
        .clicked()
    {
        actions.push(Action::NewFolderIn(folder.to_path_buf()));
        ui.close();
    }
}
