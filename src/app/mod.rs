//! Application state and the frame loop.
//!
//! Worker events, the file watcher and recovery checkpoints are handled in
//! [`eframe::App::logic`], which also runs while the window is minimized or
//! hidden, so runs never stall waiting for a repaint. Each frame then applies
//! settings and collects keyboard shortcuts, builds the UI (menu, toolbar,
//! dock area, status bar, dialogs), which only *queues* [`Action`]s, and
//! finally executes the queued actions. Keeping mutations out of the widget
//! code makes the data flow easy to follow.
//!
//! [`AppState::execute`] dispatches each action. The work behind it lives in
//! one submodule per concern, each an `impl AppState` block:
//!
//! * `documents`: open, save, reload, external changes, recovery, history;
//! * `tabs`: the open documents, switching and closing tabs, the session
//!   they are restored from and the script F5 runs;
//! * `projects`: the project folder, its main script and file operations;
//! * `runs`: rendering, debugging and stopping scripts;
//! * `worker_events`: the worker's lifecycle and the routing of its messages;
//! * `watching`: automatic reload of the script and its imported modules;
//! * `services`: selection, measurement, inspection, export, console, upload.
//!
//! The run and request state is the pure state machine in `execution`.

mod documents;
mod execution;
mod projects;
mod runs;
mod services;
mod tabs;
mod watching;
mod worker_events;

pub use tabs::TabInfo;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use egui::ViewportCommand;
use egui_dock::{DockArea, DockState};

use crate::commands::{self, Action};
use crate::dialogs::{self, Dialogs, FilePrompt, Intent, RecoverySession};
use crate::document::{DEFAULT_SCRIPT, Document};
use crate::editor::{CodeEditor, EditorOptions};
use crate::embedded::{self, Example};
use crate::file_watcher::FileWatcher;
use crate::logging;
use crate::panels::{
    self, ConsoleState, EntryKind, InspectorState, LogView, MeasurementState, NavigationState,
    ParametersState, SelectorPreviewState, SelectorState, SelectorUse, Tab, TabContext,
};
use crate::project::Project;
use crate::recovery::{CHECKPOINT_INTERVAL, RecoveryStore};
use crate::scene::{Overlay, Rgb, RunScript, Scene};
use crate::settings::{self, Projection, Settings};
use crate::theme::{self, Palette};
use crate::updates;
use crate::viewer::{ViewInput, Viewer, ViewerEvent};
use crate::worker::protocol::{CadCheck, NavigationMode, Request, RequestId, Variable};
use crate::worker::{self, Worker};

use execution::{Execution, Pending, Phase};
use services::ExportJob;
use tabs::EditorTabs;
use watching::WatchKey;

const SETTINGS_KEY: &str = "settings";
/// Meshing shows its progress bar only once it has taken this long.
const MESHING_BAR_DELAY: std::time::Duration = std::time::Duration::from_millis(300);
const LAYOUT_KEY: &str = "dock_layout";
/// The CadQuery logo for the window icon and the About dialog: a static, so
/// the binary holds the bytes once.
pub static LOGO_ICO: &[u8] = include_bytes!("../../assets/icons/cadquery_logo_dark.ico");
/// The documentation of this editor, not the wiki of the PyQt one.
pub const DOCUMENTATION_URL: &str =
    concat!(env!("CARGO_PKG_REPOSITORY"), "/blob/master/docs/README.md");
/// How to set up Python with CadQuery for the editor.
pub const INSTALLATION_URL: &str = concat!(
    env!("CARGO_PKG_REPOSITORY"),
    "/blob/master/docs/getting-started/installation.md"
);

/// Command-line options.
#[derive(Debug, Default)]
pub struct StartupOptions {
    pub file: Option<PathBuf>,
    pub python: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusKind {
    Info,
    Success,
    Warning,
    Error,
}

pub struct Status {
    pub kind: StatusKind,
    pub text: String,
}

pub struct AppState {
    pub settings: Settings,
    /// The document of the shown tab.
    pub document: Document,
    /// The editor of the shown tab.
    pub editor: CodeEditor,
    /// The other open tabs.
    tabs: EditorTabs,
    /// The folder of the Project panel.
    pub project: Option<Project>,
    pub scene: Scene,
    pub viewer: Viewer,
    pub worker: Worker,
    pub variables: Vec<Variable>,
    pub traceback: Option<crate::worker::protocol::ScriptError>,
    /// The document whose run raised the traceback.
    pub traceback_document: Option<u64>,
    pub inspector: InspectorState,
    pub measurements: MeasurementState,
    /// The CadQuery selector of the topology selection.
    pub selector: SelectorState,
    pub selector_preview: SelectorPreviewState,
    pub navigation: NavigationState,
    pub parameters: ParametersState,
    /// When to render after a parameter changed (`ctx` time, seconds).
    parameter_render_at: Option<f64>,
    /// Identifies the shown document among those opened or created; a
    /// reload of the same file keeps it, so the lines of its objects still
    /// apply.
    pub(crate) document_generation: u64,
    /// The script the current run executes, for its objects' lines.
    run_script: Option<(RequestId, Arc<RunScript>)>,
    pub console: ConsoleState,
    pub log_view: LogView,
    pub status: Option<Status>,
    pub dialogs: Dialogs,
    pub geometry_checks: HashMap<String, crate::worker::protocol::GeometryReport>,
    pub cad_checks: Vec<CadCheck>,
    pub cad_checks_document: Option<u64>,
    export_job: Option<ExportJob>,
    ctx: egui::Context,
    execution: Execution,
    actions: Vec<Action>,
    watcher: FileWatcher,
    watch_key: Option<WatchKey>,
    focused_last_frame: bool,
    /// The local modules the script F5 runs imports, while they are watched.
    imports: Vec<PathBuf>,
    examples: Vec<Example>,
    palette: Palette,
    cache_dir: PathBuf,
    recovery: RecoveryStore,
    next_recovery: Instant,
    recovery_error_reported: bool,
    python_override: Option<String>,
    worker_was_ready: bool,
    allow_close: bool,
    title: String,
    applied_theme: Option<settings::Theme>,
    applied_scale: f32,
    logo: Option<egui::TextureHandle>,
    /// Whether the layout has the Editor panel, for the empty view's hint.
    editor_panel_open: bool,
}

impl AppState {
    pub fn queue(&mut self, actions: impl IntoIterator<Item = Action>) {
        self.actions.extend(actions);
    }

    pub fn palette(&self) -> &Palette {
        &self.palette
    }

    pub fn examples(&self) -> &[Example] {
        &self.examples
    }

    pub fn default_color(&self) -> Rgb {
        self.settings
            .viewer
            .default_color
            .map(|c| f32::from(c) / 255.0)
    }

    pub fn is_paused(&self) -> bool {
        self.execution.is_paused()
    }

    pub fn is_debugging(&self) -> bool {
        self.execution.is_debugging()
    }

    pub fn is_busy(&self) -> bool {
        self.execution.is_busy()
    }

    /// Something Stop can stop: a script, or a console command.
    pub fn can_stop(&self) -> bool {
        self.execution.is_busy() || self.console.busy
    }

    /// The CadQuery logo, loaded on first use.
    pub fn logo(&mut self, ctx: &egui::Context) -> Option<egui::TextureHandle> {
        if self.logo.is_none() {
            let image =
                image::load_from_memory_with_format(LOGO_ICO, image::ImageFormat::Ico).ok()?;
            let rgba = image.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
            self.logo =
                Some(ctx.load_texture("cadquery-logo", color_image, egui::TextureOptions::LINEAR));
        }
        self.logo.clone()
    }

    fn set_status(&mut self, kind: StatusKind, text: impl Into<String>) {
        self.status = Some(Status {
            kind,
            text: text.into(),
        });
    }

    fn now(&self) -> f64 {
        self.ctx.input(|i| i.time)
    }

    // -- per-frame bookkeeping ------------------------------------------------

    /// Work that must go on without painting: eframe calls this before
    /// every frame and also while the window is minimized or hidden,
    /// whenever a repaint was requested (the worker threads request one per
    /// message; the watcher and recovery timers schedule theirs).
    fn logic(&mut self, ctx: &egui::Context) {
        self.process_worker_events();
        self.poll_stop(ctx);
        self.poll_parameter_render(ctx);
        self.poll_selector_preview(ctx);
        let focused = ctx.input(|input| input.focused);
        if focused && !self.focused_last_frame {
            self.check_disk_on_focus();
        }
        self.focused_last_frame = focused;
        self.sync_watcher();
        self.poll_watcher(ctx);
        self.poll_recovery(ctx);
    }

    fn begin_frame(&mut self, ctx: &egui::Context) {
        if self.applied_theme != Some(self.settings.general.theme) {
            theme::apply(ctx, self.settings.general.theme);
            self.applied_theme = Some(self.settings.general.theme);
        }
        // egui's own Ctrl +/- zoom and the preference stay in sync.
        if self.settings.general.ui_scale != self.applied_scale {
            ctx.set_zoom_factor(self.settings.general.ui_scale);
            self.applied_scale = self.settings.general.ui_scale;
        } else if (ctx.zoom_factor() - self.applied_scale).abs() > 1e-3 {
            self.applied_scale = ctx.zoom_factor();
            self.settings.general.ui_scale = self.applied_scale;
        }
        self.palette = Palette::of(ctx.theme());
        // A modal dialog keeps the pointer from the window, but not the
        // keyboard: shortcuts wait until its question is answered, so no
        // tab switches or file opens behind it.
        if !self.dialogs.modal_open() {
            commands::collect_shortcuts(ctx, &mut self.actions);
            commands::collect_selection_shortcuts(
                ctx,
                &self.settings.viewer.selection_shortcuts,
                &mut self.actions,
            );
        }
    }

    fn end_frame(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close && self.any_modified()
        {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.guard_unsaved(Intent::Quit);
        }

        let marker = if self.document.is_modified() { "*" } else { "" };
        let folder = self
            .document
            .path()
            .and_then(Path::parent)
            .and_then(Path::file_name)
            .map(|name| format!(" — {}", name.to_string_lossy()))
            .unwrap_or_default();
        let title = format!(
            "{}{marker}{folder} — CQ-editor",
            self.document.display_name()
        );
        if title != self.title {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    // -- panels ---------------------------------------------------------------

    pub fn editor_ui(&mut self, ui: &mut egui::Ui) {
        let colors = self.palette.editor();
        let options = EditorOptions {
            font_size: self.settings.editor.font_size,
            line_wrap: self.settings.editor.line_wrap,
            ruler: self.settings.editor.max_line_length,
            highlight_current_line: self.settings.editor.highlight_current_line,
            auto_indent: self.settings.editor.auto_indent,
            completion_min_chars: self.settings.editor.completion_min_chars,
            colors: &colors,
        };
        let tabs = self.tabs();
        if tabs.len() > 1 {
            crate::ui::editor_tabs::ui(ui, &tabs, &mut self.actions);
        }
        let output = self.editor.ui(ui, &mut self.document, &options);
        if let Some(request) = output.completion_request {
            self.request_completion(request);
        }
        if let Some(request) = output.hover_request {
            self.request_description(request);
        }
        self.update_selector_preview_cursor();
    }

    pub fn viewer_ui(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame) {
        let colors = self.palette.viewer();
        let busy = match (self.execution.phase(), self.worker.status()) {
            (Phase::Running { debug: false, .. }, _) => Some("Running script…"),
            (Phase::Running { debug: true, .. }, _) => Some("Debugging…"),
            (Phase::Idle, worker::Status::Starting) => Some("Starting Python…"),
            _ => None,
        };
        let busy = if self.execution.is_stopping() {
            Some("Stopping…")
        } else {
            busy
        };
        let hint = self.worker.is_ready().then(|| self.empty_view_hint());
        let events = self.viewer.ui(
            ui,
            frame,
            ViewInput {
                scene: &self.scene,
                settings: &self.settings.viewer,
                rendering: &self.settings.rendering,
                colors: &colors,
                busy,
                empty_hint: hint.as_deref(),
                meshing: self
                    .execution
                    .meshing()
                    .filter(|meshing| meshing.since.elapsed() >= MESHING_BAR_DELAY)
                    .map(|meshing| (meshing.done, meshing.total)),
            },
        );
        for event in events {
            match event {
                ViewerEvent::Picked {
                    key,
                    topology,
                    extend,
                } => self.actions.push(Action::Select {
                    key,
                    topology,
                    extend,
                }),
                ViewerEvent::FocusRequested => self.actions.push(Action::FitSelection),
                ViewerEvent::Command(action) => self.actions.push(action),
                ViewerEvent::Captured(Ok(message)) => self.set_status(StatusKind::Success, message),
                ViewerEvent::Captured(Err(err)) => self.dialogs.error("Capture failed", err),
            }
        }
    }

    /// What an empty view and Objects panel suggest: after a failed run,
    /// pressing F5 again is not the answer, and a closed editor is named.
    pub fn empty_view_hint(&self) -> String {
        let hint = match &self.traceback {
            Some(error) => {
                let location = error
                    .line
                    .map_or_else(String::new, |l| format!(" at line {l}"));
                format!("The last run failed{location}; see the Traceback panel")
            }
            None => "Press F5 to render the script".to_owned(),
        };
        if self.editor_panel_open {
            hint
        } else {
            format!("{hint} · the editor is closed: View > Editor")
        }
    }

    // -- actions ----------------------------------------------------------------

    fn execute(&mut self, action: Action) {
        let animate = self.settings.viewer.animate;
        let now = self.now();
        match action {
            Action::NewFile => self.new_file(),
            Action::OpenFile => self.choose_files(),
            Action::OpenPath(path) => self.open_path(&path),
            Action::OpenExample(index) => self.open_example(index),
            Action::Save => {
                self.save();
            }
            Action::SaveAs => {
                self.save_as();
            }
            Action::SaveAll => self.save_all(),
            Action::CloseTab => self.close_tabs(vec![self.document_generation]),
            Action::CloseDocument(generation) => self.close_tabs(vec![generation]),
            Action::CloseOtherTabs(kept) => {
                let others = self
                    .tabs()
                    .into_iter()
                    .map(|tab| tab.generation)
                    .filter(|&generation| generation != kept)
                    .collect();
                self.activate(kept);
                self.close_tabs(others);
            }
            Action::CloseAllTabs => {
                let all = self.tabs().into_iter().map(|tab| tab.generation).collect();
                self.close_tabs(all);
            }
            Action::ActivateTab(generation) => {
                self.activate(generation);
            }
            Action::NextTab => self.cycle_tab(true),
            Action::PreviousTab => self.cycle_tab(false),
            Action::OpenFolder => self.choose_project_folder(),
            Action::OpenFolderPath(path) => self.open_project_folder(&path),
            Action::CloseFolder => self.close_project(),
            Action::RefreshProject => {
                if let Some(project) = &mut self.project {
                    project.refresh();
                }
            }
            Action::NewFileIn(folder) => {
                self.dialogs.file_prompt = Some(FilePrompt::NewFile {
                    folder,
                    name: String::new(),
                    error: None,
                });
            }
            Action::NewFolderIn(folder) => {
                self.dialogs.file_prompt = Some(FilePrompt::NewFolder {
                    folder,
                    name: String::new(),
                    error: None,
                });
            }
            Action::RenamePath(path) => {
                self.dialogs.file_prompt = Some(FilePrompt::rename(&path));
            }
            Action::DeletePath(path) => {
                let unsaved = self.unsaved_below(&path);
                self.dialogs.file_prompt = Some(FilePrompt::Delete {
                    path,
                    unsaved,
                    error: None,
                });
            }
            Action::SetMainScript(path) => self.set_main_script(path),
            Action::RevealPath(path) => self.reveal_in_file_manager(&path),
            Action::LocalHistory => self.show_local_history(),
            Action::ToggleAutoreload => {
                self.settings.editor.autoreload = !self.settings.editor.autoreload
            }
            Action::Quit => self.guard_unsaved(Intent::Quit),

            Action::Undo => self.editor.undo(&mut self.document),
            Action::Redo => self.editor.redo(&mut self.document),
            Action::ToggleComment => self.editor.toggle_comment(&mut self.document),
            Action::DuplicateLine => self.editor.duplicate_line(&mut self.document),
            Action::MoveLineUp => self.editor.move_lines(&mut self.document, true),
            Action::MoveLineDown => self.editor.move_lines(&mut self.document, false),
            Action::InsertSnippet(index) => {
                if let Some(snippet) = crate::editor::snippets::SNIPPETS.get(index) {
                    self.editor.insert_snippet(&mut self.document, snippet);
                }
            }
            Action::Autocomplete => self.editor.request_completion(),
            Action::Find => self.editor.open_search(&self.document),
            Action::Replace => self.editor.open_replace(&self.document),
            Action::GoToLinePrompt => self.editor.open_go_to_line(),
            Action::ShowOutline => self.request_outline(),
            Action::GoToDefinition => self.request_navigation(NavigationMode::Definition),
            Action::FindReferences => self.request_navigation(NavigationMode::References),
            Action::CopySelector => self.use_selector(SelectorUse::Copy),
            Action::InsertSelector => self.use_selector(SelectorUse::Insert),
            Action::CopyMeasurementCode => self.request_measurement_code(),
            Action::CopyMeasurement(format) => self.copy_measurement(format),
            Action::FindNext | Action::FindPrevious => {
                if self.editor.search_is_open() {
                    self.editor
                        .find_next(&self.document, action == Action::FindNext);
                } else {
                    self.editor.open_search(&self.document);
                }
            }
            Action::Preferences => self.dialogs.preferences = true,

            Action::Render => self.start_run(false),
            Action::Profile => self.start_profile(),
            Action::ClearProfile => self.clear_line_profiles(),
            Action::ToggleDebug => {
                if self.is_debugging() {
                    self.stop_debugging();
                } else {
                    self.start_run(true);
                }
            }
            Action::Debug(action) => self.debug_command(action),
            Action::CancelMeshing => {
                if let Some(id) = self.execution.phase().running_id()
                    && self.execution.meshing().is_some()
                    && self.worker.send(&Request::Cancel { target: id })
                {
                    self.set_status(StatusKind::Info, "Cancelling the meshing…");
                }
            }
            Action::Stop => self.stop(),
            Action::ClearConsole => self.console.clear(),
            Action::ClearLog => logging::clear(),
            Action::RestartPython => self.start_worker(),
            Action::CreateEnvironment => {
                let folder = unused_folder(&self.cache_dir.join("environments").join("cadquery"));
                let base = self.python_in_use();
                self.dialogs.environment = Some(dialogs::environment::EnvironmentDialog::create(
                    folder,
                    base,
                    worker::launch::candidates(),
                ));
            }
            Action::InstallPackages => {
                let python = self.python_in_use();
                self.dialogs.environment = Some(dialogs::environment::EnvironmentDialog::install(
                    python,
                    worker::launch::candidates(),
                ));
            }
            Action::UsePython(path) => {
                self.settings.general.python = path.display().to_string();
                // A --python from the command line would win otherwise.
                self.python_override = None;
                self.start_worker();
            }

            Action::Fit => self.viewer.fit(&self.scene, animate, now),
            Action::FitSelection => self.viewer.fit_selection(&self.scene, animate, now),
            Action::View(view) => self.viewer.set_standard_view(view, animate, now),
            Action::Display(mode) => self.viewer.display_mode = mode,
            Action::SelectionMode(mode) => self.viewer.selection_mode = mode,
            Action::ToggleSection => self.viewer.toggle_section(&self.scene),
            Action::ToggleGroundGrid => {
                self.settings.viewer.ground_grid = !self.settings.viewer.ground_grid;
            }
            Action::ToggleScaleBar => {
                self.settings.viewer.scale_bar = !self.settings.viewer.scale_bar;
            }
            Action::ToggleExplode => self.viewer.toggle_explode(&mut self.scene),
            Action::SetExplode(factor) => self.scene.set_explode(factor),
            Action::ToggleIsolate => {
                if self.scene.is_isolating() {
                    self.scene.end_isolation();
                } else if self.scene.selection.is_empty() {
                    self.set_status(StatusKind::Info, "Select the objects to isolate");
                } else {
                    let keys = self.scene.selection.clone();
                    self.scene.isolate(&keys);
                }
            }
            Action::ShowAll => self.scene.show_all(),
            Action::SaveView(name) => {
                let projection = self.settings.viewer.projection;
                let name = self.viewer.save_view(name.as_deref(), projection);
                self.set_status(StatusKind::Success, format!("Saved the view \"{name}\""));
            }
            Action::GoToView(index) => {
                if let Some(projection) = self.viewer.go_to_view(index, animate, now) {
                    self.settings.viewer.projection = projection;
                } else if index < self.viewer.named_views.len() {
                    self.set_status(StatusKind::Error, "This saved view is damaged");
                }
            }
            Action::UpdateView(index) => {
                let projection = self.settings.viewer.projection;
                if let Some(name) = self.viewer.update_view(index, projection) {
                    let text = format!("Updated the view \"{name}\"");
                    self.set_status(StatusKind::Success, text);
                }
            }
            Action::DeleteView(index) => {
                self.viewer.delete_view(index);
            }
            Action::ToggleProjection => {
                self.settings.viewer.projection = match self.settings.viewer.projection {
                    Projection::Orthographic => Projection::Perspective,
                    _ => Projection::Orthographic,
                };
            }
            // Layout changes are handled by the app, which owns the dock.
            Action::TogglePanel(_) | Action::ResetLayout | Action::ToggleMaximize(_) => {}
            Action::CommandPalette => self.dialogs.palette = Some(Default::default()),

            Action::Export(format) => self.export(format),
            Action::CheckGeometry => self.check_geometry(),
            Action::ToggleInspect => self.toggle_inspect(),
            Action::Screenshot => self.dialogs.capture.open = true,
            Action::SaveImage => self.save_image(),
            Action::CopyImage => self.copy_image(),
            Action::ExportTurntable => self.export_turntable(),
            Action::CancelTurntable => {
                if self.viewer.turntable_progress().is_some() {
                    self.viewer.cancel_turntable();
                    self.set_status(StatusKind::Info, "Turntable export cancelled");
                }
            }
            Action::Upload => {
                self.dialogs.upload.get_or_insert_with(Default::default);
            }

            Action::Documentation => open_url(DOCUMENTATION_URL),
            Action::InstallationGuide => open_url(INSTALLATION_URL),
            Action::CadQueryDocumentation => open_url("https://cadquery.readthedocs.io/en/latest/"),
            Action::About => self.dialogs.about = true,
            Action::DownloadUpdate => {
                if let Some(release) = updates::available(&self.settings.general) {
                    open_url(&release.url);
                }
            }
            Action::CheckUpdates => {
                if self.send_tracked(Pending::Updates { automatic: false }, |id| {
                    Request::CheckUpdates {
                        id,
                        editor: updates::VERSION.to_owned(),
                        cadquery: true,
                    }
                }) {
                    self.set_status(StatusKind::Info, "Checking for CadQuery updates…");
                }
            }

            Action::GoToLine(line) => {
                // A line of the script F5 runs, wherever its tab is.
                self.activate(self.run_target());
                self.editor.go_to_line(&self.document.text, line);
            }
            Action::GoToScriptLine { document, line } => {
                if self.activate(document) {
                    self.editor.go_to_line(&self.document.text, line);
                } else {
                    self.set_status(
                        StatusKind::Info,
                        "The script of this traceback is no longer open",
                    );
                }
            }
            Action::OpenFileAt { path, line } => self.open_path_at(&path, line),
            Action::Select {
                key,
                topology,
                extend,
            } => self.select(key, topology, extend),
            Action::SetVisible { key, visible } => self.scene.set_visible(&key, visible),
            Action::SetParameter { name, line, value } => self.set_parameter(&name, line, value),
            Action::ResetParameters => self.reset_parameters(),
            Action::GoToSource { key, step } => self.go_to_source(&key, step),
            Action::GoToSelectionSource => {
                if let Some(key) = self.scene.current().map(|node| node.key.clone()) {
                    self.go_to_source(&key, None);
                } else {
                    self.set_status(StatusKind::Info, "Select an object to go to its source");
                }
            }
            Action::RemoveSelected => {
                let keys = self.scene.selected_top_level();
                let removed = self.scene.remove_top_level(&keys);
                self.geometry_checks.clear();
                self.release(removed);
                self.request_topology_results();
                self.refresh_inspector();
            }
            Action::ClearObjects => {
                let removed = self.scene.clear();
                self.geometry_checks.clear();
                self.release(removed);
                self.measurements.clear();
                self.selector.clear();
                self.scene.overlay = Overlay::default();
                self.refresh_inspector();
            }
            Action::InspectEntry { frame, item } => {
                self.inspector.selected = Some((frame, item));
                if self.inspector.show_in_view {
                    self.request_inspection_geometry();
                }
            }
            Action::ConsoleSubmit(source) => self.console_submit(source),
            Action::ConsoleComplete => self.console_complete(),
            Action::ResetConsole => {
                if self.send_tracked(Pending::Console, |id| Request::ConsoleReset { id }) {
                    self.console.more = false;
                    self.console.push(EntryKind::Info, "Console restarted\n");
                }
            }
            Action::StartUpload { url, file_type } => self.start_upload(url, file_type),
        }
    }
}

/// Opens a web page in the browser: the menu's links and the links of egui
/// widgets (eframe's own link handling is disabled, see `Cargo.toml`).
///
/// Only http(s) URLs are opened. An upload service's reply supplies links
/// too, and handed to the system, a path or another scheme would open local
/// documents or start programs.
pub fn open_url(url: &str) {
    if !is_web_url(url) {
        log::warn!("Not opening {url}: only http and https links are opened");
        return;
    }
    if let Err(err) = open::that(url) {
        log::warn!("Cannot open {url}: {err}");
    }
}

fn is_web_url(url: &str) -> bool {
    url.split_once("://").is_some_and(|(scheme, rest)| {
        (scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https"))
            && !rest.is_empty()
            && !url.chars().any(char::is_control)
    })
}

/// Takes the links egui widgets asked to open this frame.
fn take_requested_urls(ctx: &egui::Context) -> Vec<String> {
    ctx.output_mut(|output| {
        let mut urls = Vec::new();
        output.commands.retain(|command| match command {
            egui::OutputCommand::OpenUrl(open) => {
                urls.push(open.url.clone());
                false
            }
            _ => true,
        });
        urls
    })
}

pub struct CqEditorApp {
    dock: DockState<Tab>,
    /// The whole layout while one panel is maximized in `dock`.
    maximized: Option<DockState<Tab>>,
    state: AppState,
}

/// Recreates a panel where it belongs in the default workspace. Actions
/// can rely on this even after the user closed the panel or maximized one.
fn ensure_panel(dock: &mut DockState<Tab>, tab: Tab) {
    if dock.find_tab(&tab).is_some() {
        return;
    }
    let tree = dock.main_surface_mut();
    match tab {
        Tab::Editor => match tree.find_tab(&Tab::Viewer) {
            Some((node, _)) => {
                tree.split_left(node, 0.45, vec![tab]);
            }
            None => tree.push_to_focused_leaf(tab),
        },
        Tab::Project => match tree.find_tab(&Tab::Editor) {
            Some((node, _)) => {
                tree.split_left(node, 0.25, vec![tab]);
            }
            None => tree.push_to_focused_leaf(tab),
        },
        Tab::Console | Tab::Traceback | Tab::Log => {
            let sibling = [Tab::Console, Tab::Traceback, Tab::Log]
                .into_iter()
                .find_map(|other| tree.find_tab(&other).map(|(node, _)| node));
            if let Some(node) = sibling {
                tree.set_focused_node(node);
                tree.push_to_focused_leaf(tab);
            } else if let Some((node, _)) = tree.find_tab(&Tab::Viewer) {
                tree.split_below(node, 0.74, vec![tab]);
            } else {
                tree.push_to_focused_leaf(tab);
            }
        }
        _ => tree.push_to_focused_leaf(tab),
    }
}

/// The layout of the last session, when it has the 3D view, with its
/// Editor back if it was closed; else the default layout.
fn restored_layout(saved: Option<DockState<Tab>>) -> DockState<Tab> {
    let mut dock = saved
        .filter(|dock| dock.find_tab(&Tab::Viewer).is_some())
        .unwrap_or_else(panels::default_layout);
    ensure_panel(&mut dock, Tab::Editor);
    dock
}

/// Names of the panel tabs' own menu that a newcomer understands.
fn label_tab_menus(dock: &mut DockState<Tab>) {
    let menu = &mut dock.translations.tab_context_menu;
    menu.eject_button = "Float in a Window".to_owned();
    menu.close_button = "Close Panel".to_owned();
}

impl CqEditorApp {
    pub fn new(cc: &eframe::CreationContext<'_>, options: StartupOptions) -> Self {
        let ctx = cc.egui_ctx.clone();
        ctx.set_fonts(theme::fonts());

        let settings: Settings = cc
            .storage
            .and_then(|storage| eframe::get_value(storage, SETTINGS_KEY))
            .unwrap_or_default();
        let dock = restored_layout(
            cc.storage
                .and_then(|storage| eframe::get_value::<DockState<Tab>>(storage, LAYOUT_KEY)),
        );

        let cache_dir = eframe::storage_dir(crate::APP_ID).unwrap_or_else(std::env::temp_dir);
        let recovery = RecoveryStore::new(cache_dir.join("recovery"));
        let mut startup_errors = Vec::new();
        let recovery_session = match recovery.load_session() {
            Ok(documents) if documents.is_empty() => None,
            Ok(documents) => Some(RecoverySession {
                documents,
                selected: 0,
            }),
            Err(error) => {
                log::warn!("Cannot load the recovery session: {error}");
                startup_errors.push(format!("Cannot load the recovery session:\n{error}"));
                None
            }
        };
        let dialogs = Dialogs {
            recovery_session,
            ..Dialogs::default()
        };

        let mut state = AppState {
            worker: Worker::new(ctx.clone()),
            ctx,
            settings,
            document: Document::untitled(DEFAULT_SCRIPT),
            editor: CodeEditor::default(),
            tabs: EditorTabs::new(0),
            project: None,
            scene: Scene::default(),
            viewer: Viewer::default(),
            variables: Vec::new(),
            traceback: None,
            traceback_document: None,
            inspector: InspectorState::default(),
            measurements: MeasurementState::default(),
            selector: SelectorState::default(),
            selector_preview: SelectorPreviewState::default(),
            navigation: NavigationState::default(),
            parameters: ParametersState::default(),
            parameter_render_at: None,
            document_generation: 0,
            run_script: None,
            console: ConsoleState::default(),
            log_view: LogView::default(),
            status: None,
            dialogs,
            geometry_checks: HashMap::new(),
            cad_checks: Vec::new(),
            cad_checks_document: None,
            export_job: None,
            execution: Execution::default(),
            actions: Vec::new(),
            watcher: FileWatcher::default(),
            watch_key: None,
            focused_last_frame: false,
            imports: Vec::new(),
            examples: embedded::examples(),
            palette: Palette::DARK,
            cache_dir,
            recovery,
            next_recovery: Instant::now() + CHECKPOINT_INTERVAL,
            recovery_error_reported: false,
            python_override: options.python,
            worker_was_ready: false,
            allow_close: false,
            title: String::new(),
            applied_theme: None,
            applied_scale: 1.0,
            logo: None,
            editor_panel_open: true,
        };
        startup_errors.extend(state.restore_session(options.file));
        if !startup_errors.is_empty() {
            state
                .dialogs
                .error("Startup problem", startup_errors.join("\n\n"));
        }
        if let Some(path) = state.document.path().map(Path::to_path_buf) {
            state.settings.add_recent_file(&path);
            state.settings.last_file = Some(path);
        }
        state.restore_views();
        state.start_worker();
        let mut dock = dock;
        label_tab_menus(&mut dock);
        let mut app = Self {
            dock,
            maximized: None,
            state,
        };
        if app.state.project.is_some() {
            app.show_panel(Tab::Project);
        }
        app
    }

    /// Shows `tab` alone in the window (the focused panel when `None`), or
    /// brings the whole layout back.
    fn toggle_maximize(&mut self, tab: Option<Tab>) {
        if self.maximized.is_some() {
            self.restore_layout();
            return;
        }
        let tab = tab.or_else(|| self.dock.find_active_focused().map(|(_, tab)| *tab));
        let Some(tab) = tab else {
            self.state.set_status(
                StatusKind::Info,
                "Click in a panel first, or double-click its tab, to maximize it",
            );
            return;
        };
        let mut alone = DockState::new(vec![tab]);
        label_tab_menus(&mut alone);
        self.maximized = Some(std::mem::replace(&mut self.dock, alone));
        self.state.set_status(
            StatusKind::Info,
            format!(
                "{} is maximized; double-click its tab to bring the layout back",
                tab.title()
            ),
        );
    }

    /// Ends a maximized panel, bringing the whole layout back.
    fn restore_layout(&mut self) {
        if let Some(layout) = self.maximized.take() {
            self.dock = layout;
        }
    }

    /// Shows a panel required by an action, recreating it if closed.
    fn show_panel(&mut self, tab: Tab) {
        if self.dock.find_tab(&tab).is_none() {
            self.restore_layout();
        }
        ensure_panel(&mut self.dock, tab);
        if let Some(path) = self.dock.find_tab(&tab) {
            let _ = self.dock.set_active_tab(path);
        }
    }

    fn execute(&mut self, action: Action) {
        let shown = self.state.document_generation;
        self.execute_action(action);
        // New, Open, examples, tabs, recovered work…: another document is
        // shown, so its editor must be on screen.
        if self.state.document_generation != shown {
            self.show_panel(Tab::Editor);
        }
    }

    fn execute_action(&mut self, action: Action) {
        match action {
            Action::TogglePanel(tab) => {
                self.restore_layout();
                match self.dock.find_tab(&tab) {
                    Some(path) => {
                        self.dock.remove_tab(path);
                    }
                    None => self.dock.push_to_focused_leaf(tab),
                }
            }
            Action::ToggleMaximize(tab) => self.toggle_maximize(tab),
            Action::ResetLayout => {
                self.maximized = None;
                self.dock = panels::default_layout();
                label_tab_menus(&mut self.dock);
                if self.state.project.is_some() {
                    self.show_panel(Tab::Project);
                }
            }
            Action::GoToSource { .. }
            | Action::GoToSelectionSource
            | Action::OpenPath(_)
            | Action::ActivateTab(_)
            | Action::GoToLine(_)
            | Action::GoToScriptLine { .. }
            | Action::OpenFileAt { .. } => {
                // The file or line is shown in the editor, wherever its
                // panel is.
                self.show_panel(Tab::Editor);
                self.state.execute(action);
            }
            Action::ShowOutline | Action::GoToDefinition | Action::FindReferences => {
                self.show_panel(Tab::Navigation);
                self.state.execute(action);
            }
            Action::OpenFolder | Action::OpenFolderPath(_) => {
                self.state.execute(action);
                if self.state.project.is_some() {
                    self.show_panel(Tab::Project);
                }
            }
            Action::ConsoleSubmit(source) => {
                self.show_panel(Tab::Console);
                self.state.execute(Action::ConsoleSubmit(source));
            }
            Action::ResetConsole => {
                self.show_panel(Tab::Console);
                self.state.execute(Action::ResetConsole);
            }
            other => self.state.execute(other),
        }
    }
}

impl eframe::App for CqEditorApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let had_traceback = self.state.traceback.is_some();
        self.state.logic(ctx);
        if !had_traceback && self.state.traceback.is_some() {
            self.show_panel(Tab::Traceback);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.state.begin_frame(&ctx);
        let palette = self.state.palette;

        let layout = self.maximized.as_ref().unwrap_or(&self.dock);
        let open_panels: Vec<Tab> = layout.iter_all_tabs().map(|(_, tab)| *tab).collect();
        self.state.editor_panel_open = open_panels.contains(&Tab::Editor);
        let bar = |margin_y: i8| {
            egui::Frame::NONE
                .fill(palette.window)
                .inner_margin(egui::Margin::symmetric(8, margin_y))
        };
        egui::Panel::top("menu_bar").frame(bar(2)).show(ui, |ui| {
            crate::ui::menu_bar::ui(ui, &mut self.state, &open_panels, self.maximized.is_some())
        });
        egui::Panel::top("toolbar")
            .frame(bar(3))
            .show(ui, |ui| crate::ui::toolbar::ui(ui, &mut self.state));
        egui::Panel::bottom("status_bar")
            .frame(bar(3))
            .show(ui, |ui| crate::ui::status_bar::ui(ui, &self.state));
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(palette.window)
                    .inner_margin(egui::Margin::symmetric(4, 0)),
            )
            .show(ui, |ui| {
                let style = theme::dock_style(ui.style(), &palette);
                DockArea::new(&mut self.dock)
                    .style(style)
                    .show_leaf_collapse_buttons(false)
                    .show_leaf_close_all_buttons(false)
                    .show_inside(
                        ui,
                        &mut TabContext {
                            state: &mut self.state,
                            frame,
                            maximized: self.maximized.is_some(),
                        },
                    );
            });

        // Closing the one panel shown maximized brings the layout back.
        if self.maximized.is_some() && self.dock.iter_all_tabs().next().is_none() {
            self.restore_layout();
        }

        dialogs::show(&ctx, &mut self.state);
        if let worker::Status::Failed(reason) = self.state.worker.status() {
            let problem = dialogs::PythonProblem {
                problem: self.state.worker.problem(),
                reason: reason.clone(),
                python: self.state.worker.config().map(|c| c.python.clone()),
            };
            // Looked for once, not on every frame.
            let candidates = self
                .state
                .dialogs
                .python_candidates
                .get_or_insert_with(worker::launch::candidates);
            let mut actions = Vec::new();
            dialogs::python_problem(&ctx, &problem, candidates, &mut actions);
            self.state.queue(actions);
        }

        for action in std::mem::take(&mut self.state.actions) {
            log::debug!("action {action:?}");
            self.execute(action);
        }
        for url in take_requested_urls(&ctx) {
            open_url(&url);
        }
        self.state.end_frame(&ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.state.checkpoint_recovery();
        self.state.remember_all_breakpoints();
        self.state.remember_views();
        self.state.remember_session();
        eframe::set_value(storage, SETTINGS_KEY, &self.state.settings);
        // The whole layout, not the one panel maximized.
        let layout = self.maximized.as_ref().unwrap_or(&self.dock);
        eframe::set_value(storage, LAYOUT_KEY, layout);
    }

    fn on_exit(&mut self) {
        logging::flush();
    }
}

#[cfg(test)]
#[path = "../../tests/rust/app/test_app.rs"]
pub(crate) mod tests;

/// `preferred`, or `preferred-2`, `-3`… when it exists and is not empty.
fn unused_folder(preferred: &Path) -> PathBuf {
    let free =
        |path: &Path| std::fs::read_dir(path).map_or(true, |mut entries| entries.next().is_none());
    if free(preferred) {
        return preferred.to_path_buf();
    }
    let name = preferred
        .file_name()
        .map_or_else(|| "environment".into(), |name| name.to_string_lossy());
    (2..)
        .map(|n| preferred.with_file_name(format!("{name}-{n}")))
        .find(|path| free(path))
        .expect("a free folder")
}
