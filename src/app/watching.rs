//! Automatic reload: the script F5 runs, the local modules it imports and
//! the files open in tabs are watched. A changed tab is reloaded (or, with
//! unsaved edits, asked about), and a change to the script or its modules
//! renders it again.

use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::execution::Pending;
use super::{AppState, StatusKind};
use crate::dialogs::ExternalChange;
use crate::document::ExternalUpdate;
use crate::worker::protocol::Request;

/// What the watcher follows; it is told again when this changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct WatchKey {
    autoreload: bool,
    watch_imports: bool,
    /// The script F5 runs, whose imports are asked for.
    script: Option<PathBuf>,
    files: Vec<PathBuf>,
}

impl AppState {
    /// Inspect all open files when the window regains focus. This runs even
    /// when automatic preview is disabled: external edits must not remain
    /// invisible until the next save.
    pub(super) fn check_disk_on_focus(&mut self) {
        let documents: Vec<_> = self
            .documents_in_order()
            .into_iter()
            .filter_map(|(generation, document)| {
                document.path().map(|path| (generation, path.to_path_buf()))
            })
            .collect();
        let mut conflicts = Vec::new();
        for (generation, path) in documents {
            let snapshot = self
                .document_of(generation)
                .and_then(|document| document.disk_version().transpose())
                .transpose();
            match snapshot {
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    if let Some((document, _)) = self.open_mut(generation) {
                        document.set_missing_on_disk(true);
                    }
                    self.set_status(
                        StatusKind::Warning,
                        format!(
                            "{} was deleted or moved on disk; the editor buffer is preserved",
                            path.display()
                        ),
                    );
                }
                Err(error) => {
                    self.dialogs.error(
                        "Cannot check a file on disk",
                        format!("{}\n\n{error}", path.display()),
                    );
                }
                Ok(Some(version)) => {
                    if let Some((document, _)) = self.open_mut(generation) {
                        document.set_missing_on_disk(false);
                    }
                    let update = self
                        .document_of(generation)
                        .map(|document| document.classify_external_update(&version));
                    match update {
                        Some(ExternalUpdate::Conflict) => {
                            let already_queued = self
                                .dialogs
                                .external_change
                                .as_ref()
                                .is_some_and(|change| change.document == generation)
                                || self
                                    .dialogs
                                    .external_queue
                                    .iter()
                                    .any(|change| change.document == generation);
                            if !already_queued {
                                conflicts.push(ExternalChange {
                                    document: generation,
                                    path,
                                    disk: version,
                                    show_comparison: false,
                                    rerun_after_resolution: false,
                                    on_save: false,
                                });
                            }
                        }
                        Some(ExternalUpdate::Reload | ExternalUpdate::MatchesBuffer) => {
                            self.reload_tab(generation);
                        }
                        _ => {}
                    }
                }
                Ok(None) => {}
            }
        }
        if !conflicts.is_empty() {
            self.dialogs.external_queue.extend(conflicts);
            if self.dialogs.external_change.is_none() {
                let next = self.dialogs.external_queue.remove(0);
                self.activate(next.document);
                self.dialogs.external_change = Some(next);
            }
        }
    }

    pub(super) fn sync_watcher(&mut self) {
        let editor = &self.settings.editor;
        let (autoreload, watch_imports) = (editor.autoreload, editor.watch_imported_modules);
        let script = self.run_target_path();
        let mut files = Vec::new();
        if autoreload {
            files.extend(script.clone());
            if watch_imports {
                files.extend(self.imports.iter().cloned());
            }
            files.extend(
                self.documents_in_order()
                    .into_iter()
                    .filter_map(|(_, document)| document.path().map(Path::to_path_buf)),
            );
            files.sort();
            files.dedup();
        }
        let key = WatchKey {
            autoreload,
            watch_imports,
            script,
            files,
        };
        if self.watch_key.as_ref() == Some(&key) {
            return;
        }
        let ask = self.watch_key.as_ref().is_none_or(|previous| {
            (
                previous.autoreload,
                previous.watch_imports,
                &previous.script,
            ) != (key.autoreload, key.watch_imports, &key.script)
        });
        self.start_watching(&key, ask);
        self.watch_key = Some(key);
    }

    fn start_watching(&mut self, key: &WatchKey, ask_imports: bool) {
        if key.files.is_empty() {
            self.watcher.clear();
        } else {
            self.watcher.set_files(key.files.clone());
        }
        if ask_imports {
            // Imports of another script, or of none, are of no use now.
            self.imports.clear();
            self.request_imports();
        }
    }

    /// The script whose local imports are watched, if any.
    pub(super) fn watched_imports_of(&self) -> Option<PathBuf> {
        let editor = &self.settings.editor;
        (editor.autoreload && editor.watch_imported_modules)
            .then(|| self.run_target_path())
            .flatten()
    }

    /// Asks which local modules the script imports, so the watcher follows
    /// them. Called whenever watching starts and again after each save and
    /// successful run, as the imports change with the script.
    pub(super) fn request_imports(&mut self) {
        let Some(script) = self.watched_imports_of() else {
            return;
        };
        if !self.worker.is_ready() {
            return; // asked again after the first run
        }
        let path = script.display().to_string();
        self.send_tracked(Pending::Imports(script), |id| Request::Imports { id, path });
    }

    pub(super) fn poll_watcher(&mut self, ctx: &egui::Context) {
        if !self.watcher.is_watching() {
            return;
        }
        ctx.request_repaint_after(self.watcher.next_poll_in());
        let delay = Duration::from_millis(self.settings.editor.autoreload_delay_ms);
        let changed = self.watcher.poll(delay);
        if changed.is_empty() {
            return;
        }

        let script = self.run_target_path();
        let imported_changed = changed.iter().any(|path| self.imports.contains(path));
        // A script that is not open always counts; an open one only when
        // its contents changed.
        let mut script_changed = script
            .as_ref()
            .is_some_and(|script| changed.contains(script) && self.find_open(script).is_none());
        let mut conflicts = Vec::new();
        for path in &changed {
            let Some(generation) = self.find_open(path) else {
                continue;
            };
            let is_script = script.as_ref() == Some(path);
            match self.reload_tab(generation) {
                Some(ExternalUpdate::Unchanged) | None => {}
                Some(ExternalUpdate::Conflict) => {
                    let Some(disk) = self
                        .document_of(generation)
                        .and_then(|document| document.disk_version().ok().flatten())
                    else {
                        continue;
                    };
                    conflicts.push(ExternalChange {
                        document: generation,
                        path: path.clone(),
                        disk,
                        show_comparison: false,
                        rerun_after_resolution: is_script,
                        on_save: false,
                    });
                }
                Some(ExternalUpdate::MatchesBuffer | ExternalUpdate::Reload) => {
                    script_changed |= is_script;
                }
            }
        }

        if !conflicts.is_empty() {
            self.dialogs.external_queue.extend(conflicts);
            if self.dialogs.external_change.is_none() {
                let next = self.dialogs.external_queue.remove(0);
                self.activate(next.document);
                self.dialogs.external_change = Some(next);
            }
            self.set_status(
                StatusKind::Warning,
                "File changed on disk; waiting for your decision",
            );
        }
        if script_changed || imported_changed {
            match &mut self.dialogs.external_change {
                Some(conflict) => conflict.rerun_after_resolution = true,
                None => self.start_run(false),
            }
        }
    }

    /// Reloads the open document `generation` from disk when that loses
    /// nothing. Returns how the disk version relates to the buffer;
    /// a conflict is left for the user to decide.
    pub(super) fn reload_tab(&mut self, generation: u64) -> Option<ExternalUpdate> {
        let (document, _) = self.open_mut(generation)?;
        let version = match document.disk_version() {
            Ok(version) => version?,
            Err(error) => {
                log::warn!("Cannot read the changed file: {error}");
                self.dialogs
                    .error("Cannot read the changed file", error.to_string());
                return None;
            }
        };
        let update = document.classify_external_update(&version);
        let applied = match update {
            // A timestamp-only rewrite must neither erase local edits nor
            // trigger a needless rebuild.
            ExternalUpdate::Unchanged | ExternalUpdate::Conflict => return Some(update),
            ExternalUpdate::MatchesBuffer => document.apply_disk_version(version),
            ExternalUpdate::Reload => document.reload(version),
        };
        match applied {
            Ok(_) => {
                let name = document.display_name();
                self.set_status(StatusKind::Info, format!("Reloaded {name}"));
                if update == ExternalUpdate::MatchesBuffer {
                    self.checkpoint_recovery();
                }
                Some(update)
            }
            Err(error) => {
                log::warn!("Cannot reload the file: {error}");
                None
            }
        }
    }
}
