//! Modal and floating windows: preferences, upload, about, unsaved changes,
//! the Project panel's prompts and messages.

pub mod capture;
pub mod environment;
pub mod preferences;
pub mod upload;

use std::path::{Path, PathBuf};

use egui::{Align2, RichText};
use egui_phosphor::regular as icon;

use crate::app::AppState;
use crate::commands::Action;
use crate::document::{DiskVersion, Encoding};
use crate::recovery::RecoverySnapshot;
use crate::settings::Units;
use crate::worker::Problem;
use crate::worker::protocol::{ExportFormat, GeometryReport, InterferenceReport};

/// Something that would discard unsaved changes, waiting for confirmation.
#[derive(Clone, Debug, PartialEq)]
pub enum Intent {
    /// Close the tabs of these documents (by generation).
    Close(Vec<u64>),
    Quit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnsavedChoice {
    Save,
    Discard,
    Cancel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalChangeChoice {
    Reload,
    /// Keep the buffer; a save that asked goes ahead over the disk version.
    KeepLocal,
    SaveCopy,
    /// A save that asked is given up; nothing changes.
    Cancel,
}

/// An on-disk edit that conflicts with the editor contents.
pub struct ExternalChange {
    /// The open document (generation) in conflict, whichever tab is shown.
    pub document: u64,
    pub path: PathBuf,
    pub disk: DiskVersion,
    pub show_comparison: bool,
    pub rerun_after_resolution: bool,
    /// Found by a save, which waits for the answer, rather than by the
    /// file watcher.
    pub on_save: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RecoverySessionChoice {
    Restore,
    Discard,
}

/// The unsaved documents of an interrupted session, waiting for a decision.
pub struct RecoverySession {
    pub documents: Vec<RecoverySnapshot>,
    /// The one previewed.
    pub selected: usize,
}

/// A question of the Project panel: a name for a new or renamed file or
/// folder, or the confirmation of a deletion.
pub enum FilePrompt {
    NewFile {
        folder: PathBuf,
        name: String,
        error: Option<String>,
    },
    NewFolder {
        folder: PathBuf,
        name: String,
        error: Option<String>,
    },
    Rename {
        path: PathBuf,
        name: String,
        error: Option<String>,
    },
    Delete {
        path: PathBuf,
        /// Open files below it with unsaved changes, which would be lost.
        unsaved: Vec<String>,
        error: Option<String>,
    },
}

impl FilePrompt {
    pub fn rename(path: &Path) -> Self {
        FilePrompt::Rename {
            path: path.to_path_buf(),
            name: path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            error: None,
        }
    }
}

pub struct HistoryDialog {
    /// The tab whose history was opened, independent of the shown tab.
    pub document: u64,
    pub versions: Vec<RecoverySnapshot>,
    pub selected: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportScope {
    Selection,
    Visible,
    Everything,
}

#[derive(Clone, Debug)]
pub struct ExportDialog {
    pub format: ExportFormat,
    pub scope: ExportScope,
    pub step_fused: bool,
    pub step_ap242: bool,
    pub stl_ascii: bool,
    pub tolerance: f64,
    pub angular_tolerance: f64,
    pub unit: Units,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeometryChoice {
    Cancel,
    ExportAnyway,
    TryFix,
    SelectPair(usize),
}

pub struct GeometryDialog {
    pub reports: Vec<GeometryReport>,
    pub interferences: Vec<InterferenceReport>,
    pub for_export: bool,
    pub selected_pair: Option<usize>,
}

impl ExportDialog {
    pub fn new(format: ExportFormat, tolerance: f64, angular_tolerance: f64, unit: Units) -> Self {
        Self {
            format,
            scope: ExportScope::Everything,
            step_fused: false,
            step_ap242: true,
            stl_ascii: false,
            tolerance,
            angular_tolerance,
            unit,
        }
    }
}

enum HistoryChoice {
    Restore(RecoverySnapshot),
    Close,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageKind {
    Info,
    Error,
}

pub struct Message {
    pub title: String,
    pub text: String,
    pub kind: MessageKind,
}

#[derive(Default)]
pub struct Dialogs {
    pub preferences: bool,
    pub preferences_page: preferences::Page,
    pub about: bool,
    pub upload: Option<upload::UploadDialog>,
    pub capture: capture::CaptureDialog,
    pub environment: Option<environment::EnvironmentDialog>,
    pub unsaved: Option<Intent>,
    pub external_change: Option<ExternalChange>,
    /// Conflicts in other tabs, asked about once the shown one is resolved.
    pub external_queue: Vec<ExternalChange>,
    pub recovery_session: Option<RecoverySession>,
    pub file_prompt: Option<FilePrompt>,
    /// The command palette, while it is open.
    pub palette: Option<crate::ui::command_palette::Palette>,
    pub history: Option<HistoryDialog>,
    pub export: Option<ExportDialog>,
    pub geometry: Option<GeometryDialog>,
    /// A file that is not UTF-8, waiting for permission to read it as
    /// Windows-1252.
    pub legacy_encoding: Option<PathBuf>,
    pub message: Option<Message>,
    /// Interpreters found on this computer, looked for once Python failed.
    pub python_candidates: Option<Vec<PathBuf>>,
}

impl Dialogs {
    /// Whether a modal dialog is shown. Global shortcuts wait until it is
    /// answered, so nothing changes behind the question it asks.
    pub fn modal_open(&self) -> bool {
        self.about
            || self.unsaved.is_some()
            || self.external_change.is_some()
            || self.recovery_session.is_some()
            || self.file_prompt.is_some()
            || self.palette.is_some()
            || self.history.is_some()
            || self.export.is_some()
            || self.geometry.is_some()
            || self.legacy_encoding.is_some()
            || self.message.is_some()
    }

    pub fn info(&mut self, title: &str, text: impl Into<String>) {
        self.message = Some(Message {
            title: title.to_owned(),
            text: text.into(),
            kind: MessageKind::Info,
        });
    }

    pub fn error(&mut self, title: &str, text: impl Into<String>) {
        self.message = Some(Message {
            title: title.to_owned(),
            text: text.into(),
            kind: MessageKind::Error,
        });
    }
}

pub fn show(ctx: &egui::Context, state: &mut AppState) {
    let mut actions = Vec::new();

    if state.dialogs.preferences {
        let dialogs = &mut state.dialogs;
        preferences::window(
            ctx,
            &mut dialogs.preferences,
            &mut dialogs.preferences_page,
            &mut state.settings,
            &mut actions,
        );
    }

    if let Some(upload) = &mut state.dialogs.upload
        && !upload.window(ctx, &mut actions)
    {
        state.dialogs.upload = None;
    }

    if let Some(dialog) = &mut state.dialogs.environment
        && !dialog.window(ctx, &mut actions)
    {
        state.dialogs.environment = None;
    }

    if state.dialogs.capture.open {
        capture::window(
            ctx,
            &mut state.dialogs.capture,
            state.viewer.view_px(),
            state.viewer.turntable_progress(),
            &mut actions,
        );
    }

    if state.dialogs.about {
        about(ctx, state);
    }

    let recovery_was_open = state.dialogs.recovery_session.is_some();
    let recovery_choice = state
        .dialogs
        .recovery_session
        .as_mut()
        .and_then(|session| recovery_session(ctx, session));
    if let Some(choice) = recovery_choice
        && let Some(session) = state.dialogs.recovery_session.take()
    {
        state.resolve_recovery_session(session, choice == RecoverySessionChoice::Restore);
    }
    if recovery_was_open {
        state.queue(actions);
        return;
    }

    if state.dialogs.palette.is_some() {
        let commands = crate::ui::command_palette::commands(state);
        let palette = state.dialogs.palette.as_mut().expect("open");
        if let Some(chosen) =
            crate::ui::command_palette::window(ctx, palette, commands, &mut state.settings)
        {
            state.dialogs.palette = None;
            actions.extend(chosen);
        }
    }

    if let Some(intent) = state.dialogs.unsaved.clone() {
        let names = state.names_of(&state.unsaved_of(&intent));
        // Saved meanwhile: nothing is left to ask about.
        let choice = if names.is_empty() {
            Some(UnsavedChoice::Discard)
        } else {
            unsaved_changes(ctx, &names, intent == Intent::Quit)
        };
        if let Some(choice) = choice {
            state.dialogs.unsaved = None;
            state.resolve_unsaved(intent, choice);
        }
    }

    if let Some(prompt) = &mut state.dialogs.file_prompt
        && let Some(confirmed) = file_prompt(ctx, prompt)
    {
        if confirmed {
            let prompt = state.dialogs.file_prompt.take().expect("shown");
            if let Some(error) = state.confirm_file_prompt(&prompt) {
                state.dialogs.file_prompt = Some(prompt.with_error(error));
            }
        } else {
            state.dialogs.file_prompt = None;
        }
    }

    if let Some(path) = state.dialogs.legacy_encoding.clone()
        && let Some(open) = legacy_encoding(ctx, &path)
    {
        state.dialogs.legacy_encoding = None;
        if open {
            state.open_path_with(&path, Encoding::Windows1252);
        }
    }

    // About the document in conflict, not the tab shown.
    let conflicted = state
        .dialogs
        .external_change
        .as_ref()
        .map(|change| state.document_of(change.document))
        .map(|document| document.map(|d| (d.display_name(), d.text.clone())));
    let external_choice = match (conflicted, state.dialogs.external_change.as_mut()) {
        (Some(Some((name, text))), Some(change)) => external_change(ctx, &name, &text, change),
        // Its tab was closed meanwhile: nothing is left to decide.
        (Some(None), Some(_)) => Some(ExternalChangeChoice::KeepLocal),
        _ => None,
    };
    if let Some(choice) = external_choice
        && let Some(change) = state.dialogs.external_change.take()
    {
        state.resolve_external_change(change, choice);
    }

    let history_choice = state
        .dialogs
        .history
        .as_mut()
        .and_then(|history| local_history(ctx, history));
    match history_choice {
        Some(HistoryChoice::Restore(snapshot)) => {
            if let Some(history) = state.dialogs.history.take() {
                state.restore_history(history.document, snapshot);
            }
        }
        Some(HistoryChoice::Close) => state.dialogs.history = None,
        None => {}
    }

    let selected = state.scene.selected_roots().len();
    let visible = state
        .scene
        .visible_nodes()
        .iter()
        .filter(|node| !node.parts.is_empty())
        .count();
    let everything = state.scene.top_level_keys().len();
    let export_choice = state
        .dialogs
        .export
        .as_mut()
        .and_then(|dialog| export_options(ctx, dialog, selected, visible, everything));
    if let Some(confirmed) = export_choice
        && let Some(options) = state.dialogs.export.take()
        && confirmed
    {
        state.export_confirmed(options);
    }
    let geometry_choice = state
        .dialogs
        .geometry
        .as_ref()
        .and_then(|dialog| geometry_review(ctx, dialog));
    if let Some(choice) = geometry_choice {
        if let GeometryChoice::SelectPair(index) = choice {
            if let Some(pair) = state.dialogs.geometry.as_mut().and_then(|dialog| {
                dialog.selected_pair = Some(index);
                dialog.interferences.get(index).cloned()
            }) {
                let first = pair.first.key.clone();
                let second = pair.second.key.clone();
                state.scene.select_only(&first);
                if first != second {
                    state.scene.toggle_selected(&second);
                }
                state
                    .viewer
                    .fit_selection(&state.scene, true, ctx.input(|input| input.time));
                state.show_interference(&pair);
            }
        } else if let Some(dialog) = state.dialogs.geometry.take()
            && dialog.for_export
        {
            state.resolve_geometry_export(choice);
        }
    }

    if let Some(message) = &state.dialogs.message
        && message_box(ctx, message)
    {
        state.dialogs.message = None;
    }

    state.queue(actions);
}

fn modal_frame(ctx: &egui::Context) -> egui::Frame {
    egui::Frame::popup(&ctx.style_of(ctx.theme())).inner_margin(egui::Margin::same(18))
}

fn export_options(
    ctx: &egui::Context,
    options: &mut ExportDialog,
    selected: usize,
    visible: usize,
    everything: usize,
) -> Option<bool> {
    let mut choice = None;
    let response = egui::Modal::new(egui::Id::new("export_options"))
        .frame(modal_frame(ctx))
        .show(ctx, |ui| {
            ui.set_width(390.0);
            ui.heading("Export model");
            egui::ComboBox::from_label("Format")
                .selected_text(options.format.label())
                .show_ui(ui, |ui| {
                    for format in ExportFormat::ALL {
                        ui.selectable_value(&mut options.format, format, format.label());
                    }
                });
            ui.separator();
            egui::ComboBox::from_label("Model unit")
                .selected_text(options.unit.label())
                .show_ui(ui, |ui| {
                    for unit in Units::ALL.into_iter().filter(|unit| *unit != Units::None) {
                        ui.selectable_value(&mut options.unit, unit, unit.label());
                    }
                });
            if options.unit == Units::None {
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    "Choose the unit represented by one model unit.",
                );
            }
            if matches!(options.format, ExportFormat::Stl | ExportFormat::Dxf) {
                ui.weak(
                    "This file format has no unit metadata. Tell the recipient the model unit.",
                );
            }
            ui.separator();
            ui.label("Objects");
            ui.add_enabled_ui(selected > 0, |ui| {
                ui.radio_value(
                    &mut options.scope,
                    ExportScope::Selection,
                    format!("Selection ({selected})"),
                );
            });
            ui.add_enabled_ui(visible > 0, |ui| {
                ui.radio_value(
                    &mut options.scope,
                    ExportScope::Visible,
                    format!("Visible parts ({visible})"),
                );
            });
            ui.radio_value(
                &mut options.scope,
                ExportScope::Everything,
                format!("Everything ({everything})"),
            );
            if options.format == ExportFormat::Step {
                ui.separator();
                ui.checkbox(&mut options.step_fused, "Fuse STEP assembly");
                ui.checkbox(&mut options.step_ap242, "STEP AP242 (uncheck for AP214)");
            }
            if options.format == ExportFormat::Stl {
                ui.separator();
                ui.checkbox(&mut options.stl_ascii, "ASCII STL (larger file)");
            }
            if matches!(
                options.format,
                ExportFormat::Stl
                    | ExportFormat::ThreeMf
                    | ExportFormat::Gltf
                    | ExportFormat::Glb
                    | ExportFormat::Amf
                    | ExportFormat::Vrml
                    | ExportFormat::Vtkjs
            ) {
                ui.horizontal(|ui| {
                    ui.label("Linear tolerance");
                    ui.add(
                        egui::DragValue::new(&mut options.tolerance)
                            .speed(0.01)
                            .range(0.0001..=100.0),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Angular tolerance (rad)");
                    ui.add(
                        egui::DragValue::new(&mut options.angular_tolerance)
                            .speed(0.01)
                            .range(0.0001..=std::f64::consts::PI),
                    );
                });
            }
            let note = match options.format {
                ExportFormat::Step | ExportFormat::Gltf | ExportFormat::Glb | ExportFormat::Xbf => {
                    "Keeps object hierarchy, names and colors."
                }
                ExportFormat::ThreeMf => "Keeps part names and colors for 3D printing.",
                ExportFormat::Stl
                | ExportFormat::Brep
                | ExportFormat::Amf
                | ExportFormat::Vrml
                | ExportFormat::Vtkjs => "This format may not keep hierarchy or display colors.",
                ExportFormat::Svg => "Exports a projected hidden-line drawing.",
                ExportFormat::Dxf => "Requires planar faces or a 2D section.",
            };
            ui.weak(note);
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    choice = Some(false);
                }
                if ui
                    .add_enabled(
                        options.unit != Units::None,
                        egui::Button::new("Choose file…"),
                    )
                    .clicked()
                {
                    choice = Some(true);
                }
            });
        });
    if response.should_close() {
        choice = Some(false);
    }
    choice
}

fn geometry_review(ctx: &egui::Context, dialog: &GeometryDialog) -> Option<GeometryChoice> {
    let mut choice = None;
    let response = egui::Modal::new(egui::Id::new("geometry_review"))
        .frame(modal_frame(ctx))
        .show(ctx, |ui| {
            ui.set_width(470.0);
            ui.heading(if dialog.for_export {
                "Geometry needs review"
            } else {
                "Geometry check"
            });
            let invalid: Vec<_> = dialog
                .reports
                .iter()
                .filter(|report| !report.valid)
                .collect();
            let collisions = dialog
                .interferences
                .iter()
                .filter(|pair| pair.kind == "interference" && pair.allowance_reason.is_none())
                .count();
            let accepted = dialog
                .interferences
                .iter()
                .filter(|pair| pair.allowance_reason.is_some())
                .count();
            let uncertain = dialog
                .interferences
                .iter()
                .filter(|pair| pair.kind == "inconclusive")
                .count();
            if invalid.is_empty() {
                ui.label(format!(
                    "{} object(s) have valid topology.",
                    dialog.reports.len()
                ));
            } else {
                ui.label(format!(
                    "{} object(s) have geometry problems:",
                    invalid.len()
                ));
                egui::ScrollArea::vertical()
                    .max_height(250.0)
                    .show(ui, |ui| {
                        for report in &invalid {
                            ui.label(RichText::new(&report.name).strong());
                            ui.weak(report.issues.join(" · "));
                            ui.small(format!(
                                "{} solid(s) · {} open shell(s) · {} non-manifold edge(s)",
                                report.solids, report.open_shells, report.non_manifold_edges
                            ));
                        }
                    });
            }
            if !dialog.interferences.is_empty() {
                ui.label(format!(
                    "{collisions} interfering pair(s); {accepted} accepted pair(s); {uncertain} inconclusive pair(s)."
                ));
                egui::ScrollArea::vertical()
                    .max_height(250.0)
                    .show(ui, |ui| {
                        for (index, pair) in dialog.interferences.iter().enumerate() {
                            ui.horizontal(|ui| {
                                if ui.button("Select pair").clicked() {
                                    choice = Some(GeometryChoice::SelectPair(index));
                                }
                                ui.label(format!(
                                    "{} [part {}, solid {}] / {} [part {}, solid {}]: {}",
                                    pair.first.name,
                                    pair.first.part + 1,
                                    pair.first.solid + 1,
                                    pair.second.name,
                                    pair.second.part + 1,
                                    pair.second.solid + 1,
                                    pair.kind
                                ));
                            });
                            if let Some(volume) = pair.overlap_volume
                                && pair.kind == "interference"
                            {
                                ui.small(format!("Shared volume: {volume:.6} model units³"));
                            }
                            if let Some(distance) = pair.distance
                                && pair.kind == "clearance"
                            {
                                ui.small(format!("Clearance: {distance:.6} model units"));
                            }
                            if let Some(reason) = &pair.reason {
                                ui.weak(reason);
                            }
                            if let Some(reason) = &pair.allowance_reason {
                                ui.small(format!("Accepted in script: {reason}"));
                            }
                            if let Some([x, y, z]) = pair.point {
                                ui.small(format!("Overlap center: ({x:.3}, {y:.3}, {z:.3})"));
                            }
                        }
                    });
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui
                    .button(if dialog.for_export {
                        "Cancel export"
                    } else {
                        "Close"
                    })
                    .clicked()
                {
                    choice = Some(GeometryChoice::Cancel);
                }
                if dialog.for_export {
                    if !invalid.is_empty() && ui.button("Try ShapeFix").clicked() {
                        choice = Some(GeometryChoice::TryFix);
                    }
                    if ui.button("Export anyway").clicked() {
                        choice = Some(GeometryChoice::ExportAnyway);
                    }
                }
            });
        });
    if response.should_close() {
        choice = Some(GeometryChoice::Cancel);
    }
    choice
}

fn about(ctx: &egui::Context, state: &mut AppState) {
    let response = egui::Modal::new(egui::Id::new("about"))
        .frame(modal_frame(ctx))
        .show(ctx, |ui| {
            ui.set_width(380.0);
            ui.vertical_centered(|ui| {
                if let Some(logo) = state.logo(ctx) {
                    ui.add(egui::Image::new(&logo).fit_to_exact_size(egui::Vec2::splat(72.0)));
                }
                ui.add_space(6.0);
                ui.label(RichText::new("CQ-editor").size(22.0).strong());
                ui.weak(format!("Version {}", env!("CARGO_PKG_VERSION")));
                ui.add_space(8.0);
                ui.label("CadQuery script editor and 3D viewer.");
                ui.weak("Rust interface (egui + wgpu) with a Python worker running CadQuery.");
                ui.add_space(8.0);
                if let Some(info) = state.worker.info() {
                    ui.weak(format!(
                        "Python {} · CadQuery {} · OCP {}",
                        info.python, info.cadquery, info.ocp
                    ));
                }
                ui.hyperlink_to(
                    format!("{} Source code", icon::GITHUB_LOGO),
                    "https://github.com/igors93/CQ-editor",
                );
                ui.add_space(10.0);
                if ui.button("Close").clicked() {
                    ui.close();
                }
            });
        });
    if response.should_close() {
        state.dialogs.about = false;
    }
}

/// Returns the user's choice once made about the unsaved changes to the
/// files `names`.
fn unsaved_changes(ctx: &egui::Context, names: &[String], quitting: bool) -> Option<UnsavedChoice> {
    let mut choice = None;
    let response = egui::Modal::new(egui::Id::new("unsaved_changes"))
        .frame(modal_frame(ctx))
        .show(ctx, |ui| {
            ui.set_width(380.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(icon::WARNING)
                        .size(28.0)
                        .color(ui.visuals().warn_fg_color),
                );
                ui.vertical(|ui| {
                    ui.label(RichText::new("Unsaved changes").strong().size(16.0));
                    match names {
                        [name] => {
                            ui.label(format!("Save the changes to {name} first?"));
                        }
                        _ => {
                            ui.label(format!("Save the changes to {} files first?", names.len()));
                            egui::ScrollArea::vertical()
                                .max_height(140.0)
                                .show(ui, |ui| {
                                    for name in names {
                                        ui.label(format!("{}  {name}", icon::FILE_PY));
                                    }
                                });
                        }
                    }
                    if quitting {
                        ui.weak("Unsaved changes are lost when CQ-editor closes without saving.");
                    }
                });
            });
            ui.add_space(12.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Cancel").clicked() {
                    choice = Some(UnsavedChoice::Cancel);
                }
                if ui.button("Don't save").clicked() {
                    choice = Some(UnsavedChoice::Discard);
                }
                let save = if names.len() > 1 { "Save all" } else { "Save" };
                if ui
                    .add(
                        egui::Button::new(RichText::new(save).strong())
                            .fill(ui.visuals().selection.bg_fill),
                    )
                    .clicked()
                {
                    choice = Some(UnsavedChoice::Save);
                }
            });
        });
    if response.should_close() && choice.is_none() {
        choice = Some(UnsavedChoice::Cancel);
    }
    choice
}

impl FilePrompt {
    fn with_error(mut self, message: String) -> Self {
        match &mut self {
            FilePrompt::NewFile { error, .. }
            | FilePrompt::NewFolder { error, .. }
            | FilePrompt::Rename { error, .. }
            | FilePrompt::Delete { error, .. } => *error = Some(message),
        }
        self
    }
}

/// Asks for a name, or to confirm a deletion. Returns `Some(true)` once
/// confirmed and `Some(false)` once cancelled.
fn file_prompt(ctx: &egui::Context, prompt: &mut FilePrompt) -> Option<bool> {
    let mut choice = None;
    let display = |path: &Path| {
        path.file_name().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        )
    };
    let (title, text, confirm) = match prompt {
        FilePrompt::NewFile { folder, .. } => (
            "New file",
            format!(
                "A new file in {}. Without an extension, it is a Python script.",
                display(folder)
            ),
            "Create",
        ),
        FilePrompt::NewFolder { folder, .. } => (
            "New folder",
            format!("A new folder in {}.", display(folder)),
            "Create",
        ),
        FilePrompt::Rename { path, .. } => (
            "Rename",
            format!("A new name for {}.", display(path)),
            "Rename",
        ),
        FilePrompt::Delete { path, .. } => {
            let what = if path.is_dir() {
                format!("Delete the folder {} and everything in it?", display(path))
            } else {
                format!("Delete {}?", display(path))
            };
            ("Delete", what, "Delete")
        }
    };
    let response = egui::Modal::new(egui::Id::new("file_prompt"))
        .frame(modal_frame(ctx))
        .show(ctx, |ui| {
            ui.set_width(400.0);
            ui.label(RichText::new(title).strong().size(16.0));
            ui.label(text);
            ui.add_space(6.0);
            let (name, error) = match prompt {
                FilePrompt::NewFile { name, error, .. }
                | FilePrompt::NewFolder { name, error, .. }
                | FilePrompt::Rename { name, error, .. } => (Some(name), error),
                FilePrompt::Delete {
                    unsaved,
                    error,
                    path,
                } => {
                    if !unsaved.is_empty() {
                        ui.colored_label(
                            ui.visuals().warn_fg_color,
                            format!(
                                "{} {} unsaved changes, which will be lost.",
                                unsaved.join(", "),
                                if unsaved.len() == 1 { "has" } else { "have" }
                            ),
                        );
                    }
                    if path.is_file() {
                        ui.weak("Its last version stays in the local history.");
                    } else {
                        ui.weak("This cannot be undone.");
                    }
                    (None, error)
                }
            };
            if let Some(name) = name {
                let field = ui.add(egui::TextEdit::singleline(name).desired_width(f32::INFINITY));
                // The only text field: it keeps the keyboard, even from the
                // code editor below the prompt.
                if !field.has_focus() {
                    field.request_focus();
                }
                if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    choice = Some(true);
                }
                if field.changed() {
                    *error = None;
                }
            }
            if let Some(error) = error {
                ui.colored_label(ui.visuals().error_fg_color, error.as_str());
            }
            ui.add_space(10.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Cancel").clicked() {
                    choice = Some(false);
                }
                let destructive = matches!(prompt, FilePrompt::Delete { .. });
                let fill = if destructive {
                    ui.visuals().error_fg_color.gamma_multiply(0.35)
                } else {
                    ui.visuals().selection.bg_fill
                };
                if ui
                    .add(egui::Button::new(RichText::new(confirm).strong()).fill(fill))
                    .clicked()
                {
                    choice = Some(true);
                }
            });
        });
    if response.should_close() && choice.is_none() {
        choice = Some(false);
    }
    choice
}

/// Offers to read a file that is not UTF-8 as Windows-1252. Returns whether
/// to open it once the user decided.
fn legacy_encoding(ctx: &egui::Context, path: &Path) -> Option<bool> {
    let mut choice = None;
    let name = path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let response = egui::Modal::new(egui::Id::new("legacy_encoding"))
        .frame(modal_frame(ctx))
        .show(ctx, |ui| {
            ui.set_width(440.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(icon::WARNING)
                        .size(28.0)
                        .color(ui.visuals().warn_fg_color),
                );
                ui.vertical(|ui| {
                    ui.label(RichText::new("Not a UTF-8 file").strong().size(16.0));
                    ui.label(format!("{name} is not valid UTF-8 text."))
                        .on_hover_text(path.display().to_string());
                    ui.label(
                        "It can be opened as Windows-1252 (Western European), the usual \
                         encoding of older Windows editors. Saving then converts it to \
                         UTF-8, the encoding Python expects.",
                    );
                    ui.weak("Opening does not change the file.");
                });
            });
            ui.add_space(12.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Cancel").clicked() {
                    choice = Some(false);
                }
                if ui
                    .add(
                        egui::Button::new(RichText::new("Open as Windows-1252").strong())
                            .fill(ui.visuals().selection.bg_fill),
                    )
                    .clicked()
                {
                    choice = Some(true);
                }
            });
        });
    if response.should_close() && choice.is_none() {
        choice = Some(false);
    }
    choice
}

/// Returns a resolution only after an explicit choice. Comparing keeps the
/// dialog open, and Escape takes the lossless path: keeping the buffer, or
/// giving up the save that asked.
fn external_change(
    ctx: &egui::Context,
    name: &str,
    local_text: &str,
    change: &mut ExternalChange,
) -> Option<ExternalChangeChoice> {
    let mut choice = None;
    let response = egui::Modal::new(egui::Id::new("external_change"))
        .frame(modal_frame(ctx))
        .show(ctx, |ui| {
            ui.set_width(if change.show_comparison { 760.0 } else { 500.0 });
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(icon::WARNING)
                        .size(28.0)
                        .color(ui.visuals().warn_fg_color),
                );
                ui.vertical(|ui| {
                    ui.label(RichText::new("File changed on disk").strong().size(16.0));
                    if change.on_save {
                        ui.label(format!(
                            "{name} was changed by another program since you opened or \
                             saved it. Saving would replace that version."
                        ));
                        ui.weak("Nothing was saved yet.");
                    } else {
                        ui.label(format!(
                            "{name} was changed by another program while you have unsaved \
                             edits."
                        ));
                        ui.weak("Your editor contents have not been replaced.");
                    }
                });
            });

            if change.show_comparison {
                ui.add_space(10.0);
                ui.columns(2, |columns| {
                    let show_version =
                        |column: &mut egui::Ui, title: &str, text: &str, id: &str| {
                            column.label(RichText::new(title).strong());
                            egui::Frame::group(column.style()).show(column, |ui| {
                                egui::ScrollArea::both().id_salt(id).max_height(300.0).show(
                                    ui,
                                    |ui| {
                                        ui.add(
                                            egui::Label::new(RichText::new(text).monospace())
                                                .selectable(true),
                                        );
                                    },
                                );
                            });
                        };
                    show_version(&mut columns[0], "Your version", local_text, "local_change");
                    show_version(
                        &mut columns[1],
                        "Version on disk",
                        &change.disk.text,
                        "disk_change",
                    );
                });
            }

            ui.add_space(12.0);
            ui.weak(
                "Reload creates a recovery backup before replacing the editor contents. \
                 Saving a copy preserves your version separately, then reloads the disk version. \
                 The disk version is kept in the local history before it is overwritten.",
            );
            ui.add_space(8.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let keep = if change.on_save {
                    "Save my version"
                } else {
                    "Keep my version"
                };
                if ui
                    .add(
                        egui::Button::new(RichText::new(keep).strong())
                            .fill(ui.visuals().selection.bg_fill),
                    )
                    .clicked()
                {
                    choice = Some(ExternalChangeChoice::KeepLocal);
                }
                if change.on_save && ui.button("Cancel").clicked() {
                    choice = Some(ExternalChangeChoice::Cancel);
                }
                if ui.button("Reload from disk").clicked() {
                    choice = Some(ExternalChangeChoice::Reload);
                }
                if ui.button("Save a copy…").clicked() {
                    choice = Some(ExternalChangeChoice::SaveCopy);
                }
                let compare = if change.show_comparison {
                    "Hide comparison"
                } else {
                    "Compare changes"
                };
                if ui.button(compare).clicked() {
                    change.show_comparison = !change.show_comparison;
                }
            });
        });
    if response.should_close() && choice.is_none() {
        choice = Some(if change.on_save {
            ExternalChangeChoice::Cancel
        } else {
            ExternalChangeChoice::KeepLocal
        });
    }
    choice
}

fn recovery_session(
    ctx: &egui::Context,
    session: &mut RecoverySession,
) -> Option<RecoverySessionChoice> {
    let mut choice = None;
    egui::Modal::new(egui::Id::new("recovery_session"))
        .frame(modal_frame(ctx))
        .show(ctx, |ui| {
            ui.set_width(560.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(icon::LIFEBUOY)
                        .size(28.0)
                        .color(ui.visuals().warn_fg_color),
                );
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Unsaved work can be recovered")
                            .strong()
                            .size(16.0),
                    );
                    match session.documents.as_slice() {
                        [snapshot] => {
                            ui.label(format!(
                                "CQ-editor found an interrupted session for {}.",
                                snapshot.display_name()
                            ));
                        }
                        documents => {
                            ui.label(format!(
                                "CQ-editor found an interrupted session with unsaved changes \
                                 to {} files.",
                                documents.len()
                            ));
                        }
                    }
                    if let Some(snapshot) = session.documents.first() {
                        ui.weak(format!("Recovery checkpoint: {}", snapshot_time(snapshot)));
                    }
                });
            });
            if session.documents.len() > 1 {
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    for (index, snapshot) in session.documents.iter().enumerate() {
                        if ui
                            .selectable_label(session.selected == index, snapshot.display_name())
                            .on_hover_text(snapshot.path.as_ref().map_or_else(
                                || "Untitled".to_owned(),
                                |path| path.display().to_string(),
                            ))
                            .clicked()
                        {
                            session.selected = index;
                        }
                    }
                });
            }
            ui.add_space(10.0);
            if let Some(snapshot) = session.documents.get(session.selected) {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    egui::ScrollArea::both()
                        .id_salt("recovery_preview")
                        .max_height(180.0)
                        .show(ui, |ui| {
                            ui.add(
                                egui::Label::new(RichText::new(&snapshot.text).monospace())
                                    .selectable(true),
                            );
                        });
                });
            }
            ui.add_space(10.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::Button::new(RichText::new("Restore session").strong())
                            .fill(ui.visuals().selection.bg_fill),
                    )
                    .clicked()
                {
                    choice = Some(RecoverySessionChoice::Restore);
                }
                if ui.button("Discard recovery").clicked() {
                    choice = Some(RecoverySessionChoice::Discard);
                }
            });
        });
    choice
}

fn local_history(ctx: &egui::Context, history: &mut HistoryDialog) -> Option<HistoryChoice> {
    let mut choice = None;
    let response = egui::Modal::new(egui::Id::new("local_history"))
        .frame(modal_frame(ctx))
        .show(ctx, |ui| {
            ui.set_width(760.0);
            ui.label(RichText::new("Local history").strong().size(16.0));
            ui.weak("Recovery checkpoints are retained locally; the newest 20 versions are kept per document.");
            ui.add_space(10.0);
            ui.columns(2, |columns| {
                columns[0].set_width(220.0);
                egui::ScrollArea::vertical()
                    .id_salt("history_versions")
                    .max_height(360.0)
                    .show(&mut columns[0], |ui| {
                        for (index, snapshot) in history.versions.iter().enumerate() {
                            if ui
                                .selectable_label(
                                    history.selected == index,
                                    snapshot_time(snapshot),
                                )
                                .clicked()
                            {
                                history.selected = index;
                            }
                        }
                    });
                if let Some(snapshot) = history.versions.get(history.selected) {
                    columns[1].label(RichText::new(snapshot.display_name()).strong());
                    egui::Frame::group(columns[1].style()).show(&mut columns[1], |ui| {
                        egui::ScrollArea::both()
                            .id_salt("history_preview")
                            .max_height(330.0)
                            .show(ui, |ui| {
                                ui.add(
                                    egui::Label::new(RichText::new(&snapshot.text).monospace())
                                        .selectable(true),
                                );
                            });
                    });
                }
            });
            ui.add_space(10.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(
                        !history.versions.is_empty(),
                        egui::Button::new(RichText::new("Restore this version").strong())
                            .fill(ui.visuals().selection.bg_fill),
                    )
                    .clicked()
                    && let Some(snapshot) = history.versions.get(history.selected)
                {
                    choice = Some(HistoryChoice::Restore(snapshot.clone()));
                }
                if ui.button("Close").clicked() {
                    choice = Some(HistoryChoice::Close);
                }
            });
        });
    if response.should_close() && choice.is_none() {
        choice = Some(HistoryChoice::Close);
    }
    choice
}

fn snapshot_time(snapshot: &RecoverySnapshot) -> String {
    let time = std::time::UNIX_EPOCH
        .checked_add(std::time::Duration::from_millis(snapshot.timestamp_ms))
        .unwrap_or(std::time::UNIX_EPOCH);
    let local: chrono::DateTime<chrono::Local> = time.into();
    local.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Returns `true` once dismissed.
fn message_box(ctx: &egui::Context, message: &Message) -> bool {
    let mut dismissed = false;
    let response = egui::Modal::new(egui::Id::new("message_box"))
        .frame(modal_frame(ctx))
        .show(ctx, |ui| {
            ui.set_max_width(460.0);
            let (glyph, color) = match message.kind {
                MessageKind::Info => (icon::INFO, ui.visuals().hyperlink_color),
                MessageKind::Error => (icon::X_CIRCLE, ui.visuals().error_fg_color),
            };
            ui.horizontal(|ui| {
                ui.label(RichText::new(glyph).size(26.0).color(color));
                ui.label(RichText::new(&message.title).strong().size(16.0));
            });
            ui.add_space(6.0);
            egui::ScrollArea::vertical()
                .max_height(260.0)
                .show(ui, |ui| {
                    ui.add(egui::Label::new(&message.text).selectable(true));
                });
            ui.add_space(10.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("OK").clicked() {
                    dismissed = true;
                }
            });
        });
    dismissed || response.should_close()
}

/// Why Python is not available, for [`python_problem`].
pub struct PythonProblem {
    pub problem: Option<Problem>,
    pub reason: String,
    /// The interpreter that failed.
    pub python: Option<PathBuf>,
}

impl PythonProblem {
    /// Title and advice.
    pub fn explanation(&self) -> (&'static str, &'static str) {
        match self.problem {
            Some(Problem::NoPython) => (
                "Python was not found",
                "CQ-editor runs your scripts with a Python that has CadQuery. Install one \
                 (see the installation guide), or choose an interpreter.",
            ),
            Some(Problem::NoCadQuery) => (
                "CadQuery is not available",
                "Install CadQuery in that environment (python -m pip install cadquery), \
                 or choose another interpreter.",
            ),
            Some(Problem::Exited) | None => (
                "Python stopped before it was ready",
                "The Log panel shows what it printed. Choose another interpreter, or retry.",
            ),
        }
    }
}

/// Most detected interpreters listed at once.
const MAX_LISTED_INTERPRETERS: usize = 8;

/// Floating hint shown while the worker could not start, with ways out:
/// interpreters found on this computer, picking one, the installation
/// guide, the preferences and a retry.
pub fn python_problem(
    ctx: &egui::Context,
    problem: &PythonProblem,
    candidates: &[PathBuf],
    actions: &mut Vec<Action>,
) {
    let (title, advice) = problem.explanation();
    let others: Vec<&PathBuf> = candidates
        .iter()
        .filter(|path| Some(*path) != problem.python.as_ref())
        .take(MAX_LISTED_INTERPRETERS)
        .collect();
    egui::Area::new(egui::Id::new("python_problem"))
        .anchor(Align2::CENTER_BOTTOM, [0.0, -40.0])
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .inner_margin(egui::Margin::same(12))
                .show(ui, |ui| {
                    ui.set_max_width(560.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(icon::WARNING_OCTAGON)
                                .size(22.0)
                                .color(ui.visuals().error_fg_color),
                        );
                        ui.vertical(|ui| {
                            ui.label(RichText::new(title).strong());
                            ui.label(&problem.reason);
                            if let Some(python) = &problem.python {
                                ui.weak(format!("Interpreter: {}", python.display()));
                            }
                            ui.weak(advice);
                        });
                    });
                    if !others.is_empty() {
                        ui.add_space(4.0);
                        ui.label("Interpreters found on this computer:");
                        for path in others {
                            if ui
                                .link(format!("{}  {}", icon::FILE_PY, path.display()))
                                .on_hover_text("Restart Python with this interpreter")
                                .clicked()
                            {
                                actions.push(Action::UsePython(path.clone()));
                            }
                        }
                    }
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .button(format!("{} Choose interpreter…", icon::FOLDER_OPEN))
                            .clicked()
                            && let Some(path) = rfd::FileDialog::new()
                                .set_title("Python interpreter with CadQuery")
                                .pick_file()
                        {
                            actions.push(Action::UsePython(path));
                        }
                        if ui
                            .button(format!("{} Installation guide", icon::BOOK_OPEN_TEXT))
                            .clicked()
                        {
                            actions.push(Action::InstallationGuide);
                        }
                        if ui
                            .button(format!("{} Preferences…", icon::GEAR_SIX))
                            .clicked()
                        {
                            actions.push(Action::Preferences);
                        }
                        if ui
                            .button(format!("{} Retry", icon::ARROW_CLOCKWISE))
                            .clicked()
                        {
                            actions.push(Action::RestartPython);
                        }
                    });
                });
        });
}
