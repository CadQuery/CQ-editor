//! Requests on behalf of the editor and the panels: completion, selection,
//! measurement, the CQ inspector, export, screenshots, console and upload.

use std::path::{Path, PathBuf};

use super::execution::Pending;
use super::{AppState, StatusKind};
use crate::editor::{CompletionRequest, HoverRequest};
use crate::panels::{EntryKind, SelectorUse};
use crate::scene::{NodeKey, Overlay, TopologySelection};
use crate::worker::protocol::{
    CompletionTarget, ConsoleSelection, ExportFormat, InterferenceReport, NavigationMode, Request,
    SelectorCandidate, TopologyReference,
};

#[derive(Clone)]
pub(super) struct ExportJob {
    pub keys: Vec<NodeKey>,
    pub format: ExportFormat,
    pub path: String,
    pub tolerance: f64,
    pub angular_tolerance: f64,
    pub step_mode: String,
    pub step_schema: String,
    pub stl_ascii: bool,
    pub flat_parts: bool,
    pub unit: String,
}

impl AppState {
    pub(crate) fn request_outline(&mut self) {
        if !self.worker.is_ready() || self.is_busy() {
            return;
        }
        let source = self.document.text.clone();
        let id = self.worker.next_id();
        if self.worker.send(&Request::Outline {
            id,
            source: source.clone(),
        }) {
            self.execution.track(
                id,
                Pending::Outline {
                    document: self.document_generation,
                },
            );
            self.navigation.document = self.document_generation;
            self.navigation.source = source;
            self.navigation.outline_request = Some(id);
            self.navigation.symbols.clear();
            self.navigation.locations.clear();
            self.navigation.mode = None;
        }
    }

    pub(super) fn request_navigation(&mut self, mode: NavigationMode) {
        if !self.worker.is_ready() || self.is_busy() {
            self.set_status(
                StatusKind::Info,
                "Code navigation is available when Python is ready",
            );
            return;
        }
        let (line, column) = self.editor.cursor_position(&self.document.text);
        let source = self.document.text.clone();
        let path = self.document.path().map(|p| p.display().to_string());
        let project_root = self
            .project
            .as_ref()
            .map(|p| p.root().display().to_string());
        let id = self.worker.next_id();
        if self.worker.send(&Request::Navigate {
            id,
            source,
            line,
            column,
            path,
            project_root,
            mode,
        }) {
            self.execution.track(
                id,
                Pending::Navigate {
                    document: self.document_generation,
                },
            );
            self.navigation.document = self.document_generation;
            self.navigation.mode = Some(mode);
            self.navigation.navigation_request = Some(id);
            self.navigation.navigation_source = self.document.text.clone();
            self.navigation.navigation_document = self.document_generation;
            self.navigation.locations.clear();
            self.navigation.truncated = false;
        }
    }

    /// Schedule a preview only for the unchanged script that produced the scene.
    pub(super) fn update_selector_preview_cursor(&mut self) {
        let run = self.run_script.as_ref().and_then(|(id, script)| {
            (script.document == self.document_generation && script.text == self.document.text)
                .then_some(*id)
        });
        if !self.worker.is_ready()
            || self.scene.is_empty()
            || self.execution.phase() != super::execution::Phase::Idle
        {
            self.selector_preview.clear();
            self.scene.clear_selector_preview();
            return;
        }
        let Some(run_id) = run else {
            self.selector_preview.clear();
            self.scene.clear_selector_preview();
            return;
        };
        let (line, column) = self.editor.cursor_position(&self.document.text);
        let cursor = (run_id, line, column);
        if self.selector_preview.cursor == Some(cursor) {
            return;
        }
        self.selector_preview.clear();
        self.scene.clear_selector_preview();
        self.selector_preview.cursor = Some(cursor);
        self.selector_preview.due_at = Some(self.now() + 0.15);
        self.ctx
            .request_repaint_after(std::time::Duration::from_millis(160));
    }

    /// Debounce caret motion so a rapid series of arrows sends one worker request.
    pub(super) fn poll_selector_preview(&mut self, ctx: &egui::Context) {
        let Some(due) = self.selector_preview.due_at else {
            return;
        };
        if self.now() < due {
            ctx.request_repaint_after(std::time::Duration::from_millis(20));
            return;
        }
        if self.execution.phase() != super::execution::Phase::Idle || !self.worker.is_ready() {
            self.selector_preview.due_at = Some(self.now() + 0.15);
            ctx.request_repaint_after(std::time::Duration::from_millis(160));
            return;
        }
        self.selector_preview.due_at = None;
        let Some((run_id, line, column)) = self.selector_preview.cursor else {
            return;
        };
        let id = self.worker.next_id();
        let preferred_key = self.scene.current().map(|node| node.key.clone());
        if self.worker.send(&Request::SelectorPreview {
            id,
            run_id,
            line,
            column,
            preferred_key,
        }) {
            self.execution.track(id, Pending::SelectorPreview);
            self.selector_preview.request_id = Some(id);
        }
    }
    // -- editor services ----------------------------------------------------------

    pub(super) fn request_completion(&mut self, request: CompletionRequest) {
        if !self.worker.is_ready() {
            return;
        }
        let source = self.document.text.clone();
        let path = self.document.path().map(|p| p.display().to_string());
        let automatic = request.automatic;
        let document = self.document_generation;
        self.send_tracked(
            Pending::Completion {
                automatic,
                document,
            },
            |id| Request::Complete {
                id,
                source,
                line: request.line,
                column: request.column,
                path,
                target: CompletionTarget::Editor,
            },
        );
    }

    /// Asks for the documentation of the name under the editor's pointer.
    pub(super) fn request_description(&mut self, request: HoverRequest) {
        if !self.worker.is_ready() || self.is_busy() {
            return;
        }
        let source = self.document.text.clone();
        let path = self.document.path().map(|p| p.display().to_string());
        let document = self.document_generation;
        self.send_tracked(Pending::Describe { document }, |id| Request::Describe {
            id,
            source,
            line: request.line,
            column: request.column,
            path,
        });
    }

    // -- objects and inspection -------------------------------------------------

    pub(super) fn select(
        &mut self,
        key: Option<NodeKey>,
        topology: Option<TopologySelection>,
        extend: bool,
    ) {
        match (key, topology) {
            (Some(key), Some(selection)) => {
                if extend {
                    self.scene.ensure_selected(&key);
                } else {
                    self.scene.select_only(&key);
                }
                let entity = selection.entity;
                let node = selection.node.clone();
                let part_index = selection.part;
                if self.scene.select_topology(selection, extend) {
                    let count = self.scene.topology_selections.len();
                    if count == 0 {
                        self.set_status(StatusKind::Info, "Entity selection cleared");
                    } else {
                        let name = self
                            .scene
                            .find(&node)
                            .map_or(node.as_str(), |node| node.name.as_str());
                        let entity_label = self
                            .scene
                            .find(&node)
                            .and_then(|scene_node| scene_node.parts.get(part_index))
                            .and_then(|part| {
                                part.face_attributes.iter().find(|face| {
                                    entity.kind() == "Face" && face.id == entity.identifier()
                                })
                            })
                            .and_then(|face| face.name.clone())
                            .unwrap_or_else(|| {
                                format!("{} {}", entity.kind(), entity.identifier())
                            });
                        let plural = if count == 1 { "entity" } else { "entities" };
                        self.set_status(
                            StatusKind::Info,
                            format!(
                                "Selected {entity_label} of {name} · {count} {plural} selected"
                            ),
                        );
                    }
                }
            }
            (Some(key), None) if extend => self.scene.toggle_selected(&key),
            (Some(key), None) => self.scene.select_only(&key),
            (None, _) if !extend => self.scene.clear_selection(),
            (None, _) => {}
        }
        self.request_topology_results();
        self.refresh_inspector();
    }

    /// What the console's `selected()` returns: the picked entities, or
    /// the selected objects.
    pub(super) fn console_selection(&self) -> ConsoleSelection {
        ConsoleSelection {
            entities: self.topology_references(),
            objects: self.scene.selected_roots(),
        }
    }

    /// The selected entities as the worker names them.
    fn topology_references(&self) -> Vec<TopologyReference> {
        self.scene
            .topology_selections
            .iter()
            .map(|selection| TopologyReference {
                key: selection.node.clone(),
                part: selection.part,
                kind: selection.entity.topology_kind(),
                identifier: selection.entity.identifier(),
            })
            .collect()
    }

    /// Measures the topology selection and finds its selector, or clears
    /// both when nothing is selected.
    pub(super) fn request_topology_results(&mut self) {
        self.request_measurement();
        self.request_selector();
    }

    /// What the Measurements panel measures: the picked entities, else the
    /// selected objects.
    pub(super) fn measured(&self) -> (Vec<TopologyReference>, Vec<String>) {
        let selections = self.topology_references();
        let objects = if selections.is_empty() {
            self.scene.selected_roots()
        } else {
            Vec::new()
        };
        (selections, objects)
    }

    fn request_measurement(&mut self) {
        let (selections, objects) = self.measured();
        if (selections.is_empty() && objects.is_empty()) || !self.worker.is_ready() {
            self.measurements.clear();
            return;
        }
        let id = self.worker.next_id();
        if self.worker.send(&Request::Measure {
            id,
            selections,
            objects,
        }) {
            self.execution.track(id, Pending::Measure);
            self.measurements.loading = true;
            self.measurements.error = None;
            self.measurements.request_id = Some(id);
        } else {
            self.measurements.loading = false;
            self.measurements.error = Some("Cannot reach Python".to_owned());
            self.measurements.request_id = None;
        }
    }

    /// Copies the current measurement values to the clipboard as structured
    /// text, JSON or CSV.
    pub(super) fn copy_measurement(&mut self, format: crate::commands::MeasurementFormat) {
        let Some(result) = &self.measurements.result else {
            self.set_status(
                StatusKind::Warning,
                "Select objects, or pick faces, edges or vertices, to measure",
            );
            return;
        };
        let formatted = crate::panels::format_measurement(
            result,
            &self.scene,
            self.settings.viewer.units,
            format,
        );
        self.ctx.copy_text(formatted);
        self.set_status(
            StatusKind::Success,
            format!("Copied the measurement as {}", format.label()),
        );
    }

    /// Asks for code repeating the current measurement; it goes to the
    /// clipboard when it arrives.
    pub(super) fn request_measurement_code(&mut self) {
        let (selections, objects) = self.measured();
        if selections.is_empty() && objects.is_empty() {
            self.set_status(
                StatusKind::Warning,
                "Select objects, or pick faces, edges or vertices, to measure",
            );
            return;
        }
        if self.send_tracked(Pending::MeasurementCode, |id| Request::MeasurementCode {
            id,
            selections,
            objects,
        }) {
            self.set_status(
                StatusKind::Info,
                "Writing and checking the measurement code…",
            );
        }
    }

    /// The code of a measurement arrived: to the clipboard, with a warning
    /// unless the worker reproduced every value with it.
    pub(super) fn measurement_code_arrived(
        &mut self,
        code: String,
        verified: bool,
        problem: Option<String>,
    ) {
        self.ctx.copy_text(code);
        if verified {
            self.set_status(
                StatusKind::Success,
                "Copied the measurement as code (checked: it reproduces every value)",
            );
        } else {
            let why = problem.unwrap_or_else(|| "it could not be checked".to_owned());
            self.set_status(
                StatusKind::Warning,
                format!("Copied the measurement as code, but {why}"),
            );
        }
    }

    fn request_selector(&mut self) {
        // A Copy or Insert asked for the previous selection does not apply
        // to this one.
        self.selector.clear();
        if self.scene.topology_selections.is_empty() || !self.worker.is_ready() {
            return;
        }
        let selections = self.topology_references();
        let id = self.worker.next_id();
        if self.worker.send(&Request::Selector { id, selections }) {
            self.execution.track(id, Pending::Selector);
            self.selector.loading = true;
            self.selector.request_id = Some(id);
        } else {
            self.selector.error = Some("Cannot reach Python".to_owned());
        }
    }

    /// Copies or inserts the selector of the topology selection, once it is
    /// known.
    pub(super) fn use_selector(&mut self, usage: SelectorUse) {
        if self.scene.topology_selections.is_empty() {
            self.set_status(
                StatusKind::Warning,
                "Pick faces, edges or vertices first (Face, Edge or Vertex selection mode)",
            );
            return;
        }
        if let Some(candidate) = self.selector.candidate().cloned() {
            self.apply_selector(usage, &candidate);
        } else if self.selector.loading {
            self.selector.then = Some(usage);
            self.set_status(StatusKind::Info, "Finding a selector…");
        } else if let Some(error) = self.selector.error.clone() {
            self.set_status(StatusKind::Warning, error);
        } else {
            self.request_selector();
            if self.selector.loading {
                self.selector.then = Some(usage);
            }
        }
    }

    pub(super) fn apply_selector(&mut self, usage: SelectorUse, candidate: &SelectorCandidate) {
        let expression = &candidate.expression;
        let caution = if candidate.index_based {
            " (by index: it may select another entity once the model changes)"
        } else {
            ""
        };
        match usage {
            SelectorUse::Copy => {
                self.ctx.copy_text(expression.clone());
                self.set_status(StatusKind::Success, format!("Copied {expression}{caution}"));
            }
            SelectorUse::Insert => {
                self.editor.insert_text(&mut self.document, expression);
                self.set_status(
                    StatusKind::Success,
                    format!("Inserted {expression}{caution}"),
                );
            }
        }
    }

    /// Inspects the current object when it is a workplane.
    pub(super) fn refresh_inspector(&mut self) {
        let target = self
            .scene
            .current()
            .filter(|node| node.inspectable)
            .map(|node| (node.key.clone(), node.name.clone()));
        match target {
            Some((key, name)) => {
                if self.inspector.key.as_deref() == Some(key.as_str()) {
                    return;
                }
                self.inspector.clear();
                self.scene.overlay = Overlay::default();
                self.inspector.key = Some(key.clone());
                self.inspector.name = name;
                self.inspector.loading =
                    self.send_tracked(Pending::Inspect, |id| Request::Inspect { id, key });
            }
            None => {
                let gone = self
                    .inspector
                    .key
                    .as_ref()
                    .is_some_and(|key| self.scene.find(key).is_none());
                if gone {
                    self.inspector.clear();
                    self.scene.overlay = Overlay::default();
                }
            }
        }
    }

    pub(super) fn toggle_inspect(&mut self) {
        self.inspector.show_in_view = !self.inspector.show_in_view;
        if self.inspector.show_in_view {
            self.request_inspection_geometry();
        } else {
            self.scene.overlay = Overlay::default();
        }
    }

    pub(super) fn request_inspection_geometry(&mut self) {
        let (Some(key), Some(entry)) = (self.inspector.key.clone(), self.inspector.selected) else {
            return;
        };
        let scene = self.settings.scene_settings();
        let (frame, item) = entry;
        let request_key = key.clone();
        self.send_tracked(Pending::InspectGeometry { key, entry }, |id| {
            Request::InspectGeometry {
                id,
                key: request_key,
                frame,
                item,
                scene,
            }
        });
    }

    /// The selection, or everything when nothing is selected.
    fn objects_to_process(&self) -> Vec<NodeKey> {
        let selected = self.scene.selected_roots();
        if selected.is_empty() {
            self.scene.top_level_keys()
        } else {
            selected
        }
    }

    pub(super) fn export(&mut self, format: ExportFormat) {
        if self.scene.top_level_keys().is_empty() {
            self.set_status(StatusKind::Warning, "Nothing to export");
            return;
        }
        let mut options = crate::dialogs::ExportDialog::new(
            format,
            self.settings.objects.stl_tolerance,
            self.settings.objects.stl_angular_tolerance,
            self.settings.viewer.units,
        );
        if !self.scene.selected_roots().is_empty() {
            options.scope = crate::dialogs::ExportScope::Selection;
        }
        self.dialogs.export = Some(options);
    }

    pub(crate) fn export_confirmed(&mut self, options: crate::dialogs::ExportDialog) {
        use crate::dialogs::ExportScope;
        let keys = match options.scope {
            ExportScope::Selection => self.scene.selected_roots(),
            ExportScope::Visible => self
                .scene
                .visible_nodes()
                .into_iter()
                .filter(|node| !node.parts.is_empty())
                .map(|node| node.key.clone())
                .collect(),
            ExportScope::Everything => self.scene.top_level_keys(),
        };
        if keys.is_empty() {
            self.set_status(StatusKind::Warning, "Nothing to export in the chosen scope");
            return;
        }
        let format = options.format;
        // Named after the script that made the model.
        let stem = self
            .run_target_path()
            .as_deref()
            .and_then(Path::file_stem)
            .map_or_else(|| "model".to_owned(), |s| s.to_string_lossy().into_owned());
        let Some(mut path) = rfd::FileDialog::new()
            .add_filter(format.label(), &[format.extension()])
            .set_file_name(format!("{stem}.{}", format.extension()))
            .save_file()
        else {
            return;
        };
        if path.extension().is_none() {
            path.set_extension(format.extension());
        }
        let (tolerance, angular_tolerance) = (options.tolerance, options.angular_tolerance);
        if !tolerance.is_finite()
            || tolerance <= 0.0
            || !angular_tolerance.is_finite()
            || angular_tolerance <= 0.0
        {
            self.dialogs.error(
                "Cannot export",
                "Tolerances must be finite positive numbers.",
            );
            return;
        }
        self.export_job = Some(ExportJob {
            keys,
            format,
            path: path.display().to_string(),
            tolerance,
            angular_tolerance,
            step_mode: if options.step_fused {
                "fused"
            } else {
                "default"
            }
            .into(),
            step_schema: if options.step_ap242 { "ap242" } else { "ap214" }.into(),
            stl_ascii: options.stl_ascii,
            flat_parts: options.scope == ExportScope::Visible,
            unit: options.unit.export_code().to_owned(),
        });
        if self.send_export("check") {
            self.set_status(StatusKind::Info, format!("Exporting {}…", path.display()));
        }
    }

    fn send_export(&mut self, on_invalid: &str) -> bool {
        let Some(job) = self.export_job.clone() else {
            return false;
        };
        let on_invalid = on_invalid.to_owned();
        self.send_tracked(Pending::Export, |id| Request::Export {
            id,
            keys: job.keys,
            format: job.format,
            path: job.path,
            tolerance: job.tolerance,
            angular_tolerance: job.angular_tolerance,
            step_mode: job.step_mode,
            step_schema: job.step_schema,
            stl_ascii: job.stl_ascii,
            flat_parts: job.flat_parts,
            unit: job.unit,
            on_interference: if on_invalid == "ignore" {
                "ignore"
            } else {
                "check"
            }
            .to_owned(),
            on_invalid,
        })
    }

    pub(crate) fn resolve_geometry_export(&mut self, choice: crate::dialogs::GeometryChoice) {
        match choice {
            crate::dialogs::GeometryChoice::Cancel => self.export_job = None,
            crate::dialogs::GeometryChoice::ExportAnyway => {
                self.send_export("ignore");
            }
            crate::dialogs::GeometryChoice::TryFix => {
                self.send_export("fix");
            }
            crate::dialogs::GeometryChoice::SelectPair(_) => {}
        }
    }

    pub(crate) fn show_interference(&mut self, pair: &InterferenceReport) {
        self.scene.overlay = Overlay::default();
        if pair.kind != "interference" {
            return;
        }
        let first = pair.first.clone();
        let second = pair.second.clone();
        let scene = self.settings.scene_settings();
        self.send_tracked(
            Pending::InterferenceGeometry {
                first: first.clone(),
                second: second.clone(),
            },
            |id| Request::InterferenceGeometry {
                id,
                first,
                second,
                scene,
            },
        );
    }

    pub(super) fn check_geometry(&mut self) {
        let keys = self.objects_to_process();
        if keys.is_empty() {
            self.set_status(StatusKind::Warning, "Nothing to check");
            return;
        }
        if self.send_tracked(Pending::Check, |id| Request::Check {
            id,
            keys,
            flat_parts: false,
        }) {
            self.set_status(StatusKind::Info, "Checking geometry…");
        }
    }

    pub(super) fn save_image(&mut self) {
        if self.viewer.turntable_progress().is_some() {
            self.set_status(StatusKind::Info, "Wait for the turntable export to finish");
            return;
        }
        if let Some(path) = ask_png_path("screenshot.png") {
            self.viewer
                .request_image(path, self.dialogs.capture.options);
        }
    }

    pub(super) fn copy_image(&mut self) {
        if self.viewer.turntable_progress().is_some() {
            self.set_status(StatusKind::Info, "Wait for the turntable export to finish");
            return;
        }
        self.viewer
            .request_clipboard_image(self.dialogs.capture.options);
    }

    pub(super) fn export_turntable(&mut self) {
        let Some(path) = ask_png_path("turntable.png") else {
            return;
        };
        let (options, turntable) = (self.dialogs.capture.options, self.dialogs.capture.turntable);
        match self.viewer.start_turntable(path, options, turntable) {
            Ok(()) => self.set_status(StatusKind::Info, "Exporting the turntable…"),
            Err(error) => self.dialogs.error("Capture failed", error),
        }
    }

    pub(super) fn console_submit(&mut self, source: String) {
        if !self.worker.is_ready() {
            self.console
                .push(EntryKind::Error, "Python is not running\n");
            return;
        }
        self.console.record_input(&source);
        let scene = self.settings.scene_settings();
        let known_geometry = self.scene.known_geometry();
        let selection = self.console_selection();
        let mut sent = None;
        if self.send_tracked(Pending::Console, |id| {
            sent = Some(id);
            Request::Console {
                id,
                source,
                scene,
                known_geometry: known_geometry.clone(),
                selection,
            }
        }) {
            if let Some(id) = sent {
                self.scene.pin_geometry(id, &known_geometry);
            }
        } else {
            self.console.busy = false;
        }
    }

    pub(super) fn console_complete(&mut self) {
        let source = self.console.input.clone();
        let line = source.split('\n').count();
        let column = source
            .rsplit('\n')
            .next()
            .map_or(0, |last| last.chars().count());
        self.send_tracked(Pending::ConsoleCompletion, |id| Request::Complete {
            id,
            source,
            line,
            column,
            path: None,
            target: CompletionTarget::Console,
        });
    }

    pub(super) fn start_upload(&mut self, url: String, file_type: String) {
        let keys = self.objects_to_process();
        let source = self
            .document_of(self.run_target())
            .map_or_else(String::new, |document| document.text.clone());
        if !self.send_tracked(Pending::Upload, |id| Request::Upload {
            id,
            url,
            file_type,
            keys,
            source,
        }) && let Some(upload) = &mut self.dialogs.upload
        {
            upload.failed("Python is not ready");
        }
    }
}

/// Where to save a PNG, from the save dialog (`.png` added when missing).
fn ask_png_path(file_name: &str) -> Option<PathBuf> {
    let mut path = rfd::FileDialog::new()
        .add_filter("PNG image", &["png"])
        .set_file_name(file_name)
        .save_file()?;
    if path.extension().is_none() {
        path.set_extension("png");
    }
    Some(path)
}
