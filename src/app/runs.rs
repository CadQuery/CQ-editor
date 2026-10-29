//! Rendering and debugging the script. [`Execution`](super::execution)
//! decides; these methods send the requests and apply the results.
//!
//! A run executes the script F5 runs ([`AppState::run_target`]): the shown
//! document, or the project's main script. Its error line, debug line and
//! profile go to that document's editor, shown or not.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use super::execution::{RunEnd, RunRequest, StopPlan};
use super::{AppState, StatusKind};
use crate::editor::text_ops::{self, Located};
use crate::editor::{LineTiming, format_seconds};
use crate::logging::{self, Level};
use crate::parameters;
use crate::scene::{Overlay, RunScript, SceneNode};
use crate::worker::protocol::{
    DebugAction, Request, RequestId, RunFinished, RunProfile, SceneNode as NodeMessage,
};

/// How long after the last parameter change the script renders again, in
/// seconds: a drag renders once it pauses, not on every pixel.
pub(super) const PARAMETER_RENDER_DELAY: f64 = 0.3;

impl AppState {
    /// Writes a parameter's new value into the script, as an undoable
    /// edit, and renders again shortly after.
    pub(super) fn set_parameter(&mut self, name: &str, line: usize, value: f64) {
        if self.is_debugging() {
            self.set_status(
                StatusKind::Warning,
                "Parameters can be changed once debugging ends",
            );
            return;
        }
        let now = self.now();
        let target = self.run_target();
        let Some((document, editor)) = self.open_mut(target) else {
            return;
        };
        let Some(parameter) = parameters::parameters(&document.text)
            .into_iter()
            .find(|parameter| parameter.line == line && parameter.name == name)
        else {
            return;
        };
        let literal = parameter.literal_for(value);
        editor.adjust_text(document, parameter.literal, &literal, now);
        self.parameter_render_at = Some(now + PARAMETER_RENDER_DELAY);
    }

    /// Restores all modified parameters in the run target script to their
    /// reference values in a single undo step and schedules a render.
    pub(super) fn reset_parameters(&mut self) {
        if self.is_debugging() {
            self.set_status(
                StatusKind::Warning,
                "Parameters can be changed once debugging ends",
            );
            return;
        }
        let now = self.now();
        let target = self.run_target();
        let Some((text, saved_text)) = self
            .document_of(target)
            .map(|document| (document.text.clone(), document.saved_text().to_owned()))
        else {
            return;
        };
        let current = parameters::parameters(&text);
        self.parameters.update_references(&saved_text, &current);
        let mut replacements = Vec::new();
        for parameter in &current {
            if self.parameters.is_modified(parameter)
                && let Some(reference) = self.parameters.reference_value(&parameter.name)
            {
                let literal = parameter.literal_for(reference);
                if text.get(parameter.literal.clone()) != Some(literal.as_str()) {
                    replacements.push((parameter.literal.clone(), literal));
                }
            }
        }
        if replacements.is_empty() {
            return;
        }
        if let Some((document, editor)) = self.open_mut(target) {
            editor.replace_literals(document, &replacements, now);
            self.parameter_render_at = Some(now + PARAMETER_RENDER_DELAY);
        }
    }

    /// Renders once the parameters stopped changing for a moment.
    pub(super) fn poll_parameter_render(&mut self, ctx: &egui::Context) {
        let Some(at) = self.parameter_render_at else {
            return;
        };
        let now = ctx.input(|input| input.time);
        if now < at {
            if let Ok(wait) = std::time::Duration::try_from_secs_f64(at - now) {
                ctx.request_repaint_after(wait);
            }
            return;
        }
        self.parameter_render_at = None;
        if !self.is_debugging() {
            self.start_run(false);
        }
    }

    pub(super) fn start_run(&mut self, debug: bool) {
        self.start_run_with(debug, false);
    }

    /// Renders with the time of each line measured, shown in the gutter.
    pub(super) fn start_profile(&mut self) {
        if self.is_busy() {
            self.set_status(StatusKind::Warning, "Already running; stop it first");
            return;
        }
        self.start_run_with(false, true);
    }

    fn start_run_with(&mut self, debug: bool, profile: bool) {
        let availability = self.worker.status().into();
        match self.execution.request_run(debug, availability) {
            RunRequest::Start => {}
            RunRequest::WhenReady => {
                self.set_status(StatusKind::Info, "Waiting for Python to start…");
                return;
            }
            RunRequest::Unavailable => {
                self.set_status(
                    StatusKind::Error,
                    "Python is not available — check Preferences > General",
                );
                return;
            }
            RunRequest::Queued => return,
            RunRequest::Busy => {
                self.set_status(StatusKind::Warning, "Already running; stop it first");
                return;
            }
        }

        let target = self.prepare_run_target();
        let Some((document, editor)) = self.open_mut(target) else {
            return;
        };
        // A reload may have changed the text since the editor last drew.
        editor.sync_markers(&document.text);
        let script = document.text.clone();
        let path = document.path().map(|p| p.display().to_string());
        let (breakpoints, module_breakpoints) = self.open_breakpoints(target);
        let project_root = self
            .project
            .as_ref()
            .map(|project| project.root().display().to_string());
        let id = self.worker.next_id();
        // Pinned, so a debugging session may reference them even after it
        // clears the view below.
        let known_geometry = self.scene.known_geometry();
        self.scene.pin_geometry(id, &known_geometry);
        self.run_script = Some((
            id,
            Arc::new(RunScript {
                document: target,
                text: script.clone(),
            }),
        ));
        let request = Request::Run {
            id,
            script,
            path,
            debug,
            breakpoints,
            project_root,
            module_breakpoints,
            settings: self.settings.run_settings(),
            scene: self.settings.scene_settings(),
            known_geometry,
            profile,
        };
        if !self.worker.send(&request) {
            self.set_status(StatusKind::Error, "Cannot reach Python");
            return;
        }
        self.execution.started(id, debug);
        self.selector_preview.clear();
        self.scene.clear_selector_preview();
        self.cad_checks.clear();
        self.cad_checks_document = Some(target);
        self.scene.clear_preview();
        // The times of an earlier profile no longer describe the script.
        self.clear_line_profiles();
        if profile {
            self.set_status(StatusKind::Info, "Profiling…");
        }
        if debug {
            // Like the original: the view starts empty and follows the
            // locals of each paused frame.
            let removed = self.scene.clear();
            self.geometry_checks.clear();
            self.release(removed);
            self.measurements.clear();
            self.selector.clear();
            self.set_status(
                StatusKind::Info,
                "Debugging — running to the first breakpoint",
            );
        }
    }

    /// The document the run `id` executes: the one it started with, or the
    /// shown one.
    pub(super) fn run_document(&self, id: RequestId) -> u64 {
        self.run_script
            .as_ref()
            .filter(|(run, _)| *run == id)
            .map_or(self.document_generation, |(_, script)| script.document)
    }

    fn set_error_line(&mut self, document: u64, line: Option<usize>) {
        if let Some((_, editor)) = self.open_mut(document) {
            editor.error_line = line;
        }
    }

    pub(super) fn finish_run(&mut self, result: RunFinished) {
        let RunEnd { stale, rerun } = self.execution.ended(result.id);
        let target = self.run_document(result.id);
        self.clear_debug_lines();

        if stale {
            self.scene.clear_preview();
            let keys = result.objects.into_iter().flatten().map(|node| node.key);
            self.release(keys.collect());
            log::info!("Discarded the result of a run of the previous document");
            if rerun {
                self.start_run(false);
            }
            return;
        }

        self.cad_checks = result.checks;
        self.cad_checks_document = Some(target);
        let failed_checks = self.cad_checks.iter().filter(|check| !check.ok).count();

        match &result.error {
            Some(error) if result.interrupted => {
                // Not an error: the user asked for it.
                self.set_error_line(target, None);
                let location = error
                    .line
                    .map(|line| format!(" at line {line}"))
                    .unwrap_or_default();
                self.set_status(StatusKind::Info, format!("Stopped{location}"));
                logging::push(Level::Info, "script", &error.text);
            }
            Some(error) => {
                self.set_error_line(target, error.line);
                let location = error
                    .line
                    .map(|l| format!(" (line {l})"))
                    .unwrap_or_default();
                // Named when it is not the file shown.
                let file = match self.document_of(target) {
                    Some(document) if target != self.document_generation => {
                        format!("{}: ", document.display_name())
                    }
                    _ => String::new(),
                };
                self.set_status(
                    StatusKind::Error,
                    format!("{file}{}: {}{location}", error.type_name, error.message),
                );
                logging::push(Level::Error, "script", &error.text);
            }
            None => self.set_error_line(target, None),
        }
        let ok = result.ok;
        self.traceback = result.error.filter(|_| !result.interrupted);
        self.traceback_document = Some(target);

        if let Some(objects) = result.objects {
            let preserved = self
                .settings
                .objects
                .preserve_properties
                .then(|| self.scene.snapshot_properties());
            let was_empty = self.scene.is_empty();
            let decoding = Instant::now();
            // A scene that cannot be decoded is reported, and the rest of
            // the result (variables, a queued render) still applies.
            if let Some(mut nodes) = self.decode_scene_nodes(Some(result.id), objects) {
                self.scene.clear_preview();
                self.attach_run_script(result.id, &mut nodes);
                if let Some(timings) = &result.timings {
                    log::info!("{}", timings.report(&result.receipt, decoding.elapsed()));
                }
                let count = nodes.len();
                if self.settings.objects.clear_before_run || result.debug {
                    let removed = self.scene.replace(nodes, preserved.as_ref());
                    self.geometry_checks.clear();
                    self.release(removed);
                    self.measurements.clear();
                    self.selector.clear();
                } else {
                    self.scene.append(nodes, preserved.as_ref());
                }
                self.scene.overlay = Overlay::default();
                self.refresh_inspector();
                if self.settings.viewer.fit_automatically || was_empty {
                    let animate = self.settings.viewer.animate && !was_empty;
                    self.viewer.fit(&self.scene, animate, self.now());
                }
                if ok {
                    let plural = if count == 1 { "" } else { "s" };
                    let rendered = format!(
                        "Rendered {count} object{plural} in {:.2} s",
                        result.duration
                    );
                    match self.unsaved_modules(target).as_slice() {
                        [] => self.set_status(StatusKind::Success, rendered),
                        unsaved => self.set_status(
                            StatusKind::Warning,
                            format!(
                                "{rendered}; unsaved changes to {} were not used, save them first",
                                unsaved.join(", ")
                            ),
                        ),
                    }
                }
            }
        }
        self.scene.clear_preview();
        if let Some(variables) = result.variables {
            self.variables = variables;
        }
        if result.stopped {
            self.set_status(StatusKind::Info, "Debugging stopped");
        }
        if result.cancelled {
            self.set_status(
                StatusKind::Info,
                "Meshing cancelled; the model shown before stays",
            );
        }
        if ok && !result.cancelled && failed_checks > 0 {
            self.set_status(
                StatusKind::Warning,
                format!("{failed_checks} CAD requirement(s) failed — see Checks"),
            );
        }
        if let Some(profile) = result.profile {
            self.show_profile(result.id, profile);
        }
        if ok {
            // The script may have started importing other modules.
            self.request_imports();
        }
        if rerun {
            self.start_run(false);
        }
    }

    /// A run of the worker failed before it could finish.
    pub(super) fn run_failed(&mut self, id: RequestId, message: String) {
        let RunEnd { stale, rerun } = self.execution.ended(id);
        self.scene.clear_preview();
        self.clear_debug_lines();
        if stale {
            log::warn!("A run of the previous document failed: {message}");
        } else {
            self.set_status(StatusKind::Error, message);
        }
        if rerun {
            self.start_run(false);
        }
    }

    pub(super) fn debug_command(&mut self, action: DebugAction) {
        let Some(id) = self.execution.resume() else {
            return;
        };
        let target = self.run_document(id);
        let (breakpoints, module_breakpoints) = self.open_breakpoints(target);
        let sent = self.worker.send(&Request::Debug {
            action,
            breakpoints,
            module_breakpoints,
        });
        self.clear_debug_lines();
        if !sent {
            self.execution.ended(id);
            self.set_status(
                StatusKind::Warning,
                "The debugging session ended when Python disconnected",
            );
        }
    }

    pub(super) fn stop_debugging(&mut self) {
        if self.is_paused() {
            self.debug_command(DebugAction::Stop);
        } else {
            self.stop();
        }
    }

    /// Aborts a running script by restarting the worker.
    /// Stops the running script or console command gently: Python raises
    /// `KeyboardInterrupt` in it, so the console and the caches survive.
    /// Python is restarted when Stop is pressed again, or when the script
    /// does not stop within `STOP_GRACE` (native code that never returns
    /// to Python). A paused debugging session ends through the debugger.
    pub(super) fn stop(&mut self) {
        if self.is_paused() {
            self.debug_command(DebugAction::Stop);
            return;
        }
        match self.execution.stop(Instant::now()) {
            StopPlan::Nothing => {}
            StopPlan::Interrupt(target) => {
                if self.worker.send(&Request::Interrupt { target }) {
                    self.set_status(StatusKind::Info, "Stopping… (Stop again to restart Python)");
                } else {
                    self.restart_to_stop("Execution stopped; Python was restarted");
                }
            }
            StopPlan::Restart => self.restart_to_stop("Execution stopped; Python was restarted"),
        }
    }

    /// Restarts Python once an interrupted script has had `STOP_GRACE` to
    /// stop, and wakes the UI for that moment.
    pub(super) fn poll_stop(&mut self, ctx: &egui::Context) {
        let Some(deadline) = self.execution.stop_deadline() else {
            return;
        };
        let now = Instant::now();
        if now >= deadline {
            self.restart_to_stop("The script did not stop; Python was restarted");
        } else {
            ctx.request_repaint_after(deadline - now);
        }
    }

    fn restart_to_stop(&mut self, message: &str) {
        self.start_worker();
        self.set_status(StatusKind::Warning, message);
    }

    /// Puts a profiled run's times in the gutter, on the lines of the
    /// document as it is now.
    fn show_profile(&mut self, id: RequestId, profile: RunProfile) {
        let Some((target, run_text)) = self
            .run_script
            .as_ref()
            .filter(|(run, _)| *run == id)
            .map(|(_, script)| (script.document, script.text.clone()))
        else {
            return;
        };
        let mut by_file: BTreeMap<Option<String>, BTreeMap<usize, LineTiming>> = BTreeMap::new();
        for line in profile.lines {
            by_file.entry(line.file).or_default().insert(
                line.line,
                LineTiming {
                    seconds: line.seconds,
                    hits: line.hits,
                    operations: Vec::new(),
                },
            );
        }
        for operation in profile.operations {
            if let Some(timing) = by_file
                .get_mut(&operation.file)
                .and_then(|lines| lines.get_mut(&operation.line))
            {
                timing
                    .operations
                    .push((operation.name, operation.seconds, operation.calls));
            }
        }
        let mut slowest: Option<(String, usize, f64)> = None;
        for (file, lines) in by_file {
            let generation = match file.as_deref() {
                Some(path) => self.find_open(std::path::Path::new(path)),
                None => Some(target),
            };
            let Some(generation) = generation else {
                continue;
            };
            let source = match file.as_deref() {
                Some(path) => profile
                    .sources
                    .get(path)
                    .cloned()
                    .or_else(|| std::fs::read_to_string(path).ok()),
                None => Some(run_text.clone()),
            };
            let Some((document, editor)) = self.open_mut(generation) else {
                continue;
            };
            let source = source.unwrap_or_else(|| document.text.clone());
            editor.set_line_profile(lines, &source, &document.text);
            if let Some((line, timing)) = editor
                .line_profile
                .iter()
                .max_by(|a, b| a.1.seconds.total_cmp(&b.1.seconds))
                && slowest
                    .as_ref()
                    .is_none_or(|(_, _, seconds)| timing.seconds > *seconds)
            {
                let label = file
                    .as_deref()
                    .and_then(|path| std::path::Path::new(path).file_name())
                    .map_or_else(
                        || document.display_name(),
                        |name| name.to_string_lossy().into_owned(),
                    );
                slowest = Some((label, *line, timing.seconds));
            }
        }
        if let Some((file, line, seconds)) = slowest {
            let text = format!(
                "Profiled: the slowest line is {file}:{line} ({}); times are in the gutters",
                format_seconds(seconds)
            );
            self.set_status(StatusKind::Success, text);
        }
    }

    /// Gives the nodes of a run the script text their lines refer to.
    pub(super) fn attach_run_script(&self, id: RequestId, nodes: &mut [SceneNode]) {
        if let Some((run, script)) = &self.run_script
            && *run == id
        {
            for node in nodes {
                node.attach_script(script);
            }
        }
    }

    /// Moves the editor to the script line that created an object, or one
    /// step of its construction stack in the CQ Inspector, following the
    /// edits made since the run.
    pub(super) fn go_to_source(&mut self, key: &str, step: Option<(usize, Option<usize>)>) {
        let Some(node) = self.scene.find(key) else {
            return;
        };
        let line = match step {
            None => node.line,
            Some(_) if self.inspector.key.as_deref() != Some(key) => None,
            Some((frame, item)) => {
                let frame = self.inspector.frames.get(frame);
                match item {
                    None => frame.and_then(|frame| frame.line),
                    Some(item) => frame
                        .and_then(|frame| frame.items.get(item))
                        .and_then(|item| item.line),
                }
            }
        };
        let (Some(line), Some(script)) = (line, node.script.clone()) else {
            self.set_status(
                StatusKind::Info,
                "No script line is known for this object (objects shown from the console have none)",
            );
            return;
        };
        // Its script is shown, in whichever tab it is.
        if !self.activate(script.document) {
            self.set_status(
                StatusKind::Warning,
                "This object comes from another script, which is no longer open; render again \
                 to go to its source",
            );
            return;
        }
        let Some(located) = text_ops::locate_line(&script.text, line, &self.document.text) else {
            self.set_status(
                StatusKind::Warning,
                format!("Line {line} was deleted since the last run"),
            );
            return;
        };
        self.editor.go_to_line(&self.document.text, located.line());
        match located {
            Located::Unchanged(at) => self.set_status(StatusKind::Info, format!("Line {at}")),
            Located::Moved(at) => self.set_status(
                StatusKind::Info,
                format!("Line {at} (line {line} when the script ran)"),
            ),
            Located::Changed(at) => self.set_status(
                StatusKind::Warning,
                format!("Line {at}: edited since the last run"),
            ),
        }
    }

    pub(super) fn decode_scene_nodes(
        &mut self,
        request: Option<RequestId>,
        objects: Vec<NodeMessage>,
    ) -> Option<Vec<SceneNode>> {
        match self.scene.decode_nodes(request, objects) {
            Ok(nodes) => Some(nodes),
            Err(error) => {
                log::error!("Cannot decode worker scene: {error}");
                self.set_status(StatusKind::Error, "Cannot update the 3D scene");
                None
            }
        }
    }
}
