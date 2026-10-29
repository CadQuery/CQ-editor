//! The documents: opening, saving, reloading after external changes,
//! crash-recovery checkpoints and local history.

use std::path::Path;
use std::time::Instant;

use egui::ViewportCommand;

use super::{AppState, StatusKind};
use crate::dialogs::{
    ExternalChange, ExternalChangeChoice, HistoryDialog, Intent, RecoverySession, UnsavedChoice,
};
use crate::document::{DiskVersion, Document, Encoding, is_not_utf8};
use crate::logging::{self, Level};
use crate::recovery::{CHECKPOINT_INTERVAL, RecoverySnapshot};

impl AppState {
    /// Performs `intent`, or asks first when it would close documents with
    /// unsaved changes.
    pub(super) fn guard_unsaved(&mut self, intent: Intent) {
        let unsaved = self.unsaved_of(&intent);
        match unsaved.first() {
            None => self.perform(intent),
            Some(&first) => {
                // A single file is shown while it is asked about.
                if unsaved.len() == 1 {
                    self.activate(first);
                }
                self.dialogs.unsaved = Some(intent);
            }
        }
    }

    pub fn resolve_unsaved(&mut self, intent: Intent, choice: UnsavedChoice) {
        match choice {
            UnsavedChoice::Save => {
                let unsaved = self.unsaved_of(&intent);
                if self.save_documents(&unsaved) {
                    self.perform(intent);
                }
            }
            UnsavedChoice::Discard => self.perform(intent),
            UnsavedChoice::Cancel => {}
        }
    }

    pub fn resolve_recovery_session(&mut self, session: RecoverySession, restore: bool) {
        if let Err(error) = self.recovery.clear_session() {
            self.dialogs.error(
                "Cannot update recovery state",
                format!("Your recovery was kept on disk.\n\n{error}"),
            );
            self.dialogs.recovery_session = Some(session);
            return;
        }
        if restore {
            let names: Vec<String> = session
                .documents
                .iter()
                .map(RecoverySnapshot::display_name)
                .collect();
            for snapshot in session.documents {
                self.restore_recovered(snapshot.into_document());
            }
            self.checkpoint_recovery();
            self.set_status(
                StatusKind::Success,
                format!("Recovered unsaved work for {}", names.join(", ")),
            );
        } else {
            self.set_status(
                StatusKind::Info,
                "Discarded the interrupted recovery session",
            );
        }
    }

    /// Puts a recovered buffer back in the tab of its file, or in a new
    /// tab.
    fn restore_recovered(&mut self, document: Document) {
        match document.path().and_then(|path| self.find_open(path)) {
            Some(open) => {
                self.activate(open);
                self.replace_document(document);
            }
            None => self.add_tab(document),
        }
    }

    pub fn restore_history(&mut self, generation: u64, snapshot: RecoverySnapshot) {
        if !self.activate(generation) {
            return;
        }
        if self.document.path() != snapshot.path.as_deref() {
            self.dialogs.error(
                "Cannot restore local history",
                "This history belongs to another document.",
            );
            return;
        }
        self.checkpoint_recovery();
        let name = snapshot.display_name();
        // Another version of the same file, like a reload: the model stays.
        self.replace_document(snapshot.into_document());
        self.checkpoint_recovery();
        self.set_status(
            StatusKind::Success,
            format!("Restored a local history version of {name}"),
        );
    }

    /// Carries out the choice about a conflict on the document it concerns,
    /// which is shown for that, whichever tab was shown meanwhile. A
    /// document closed or moved since is left alone.
    pub fn resolve_external_change(
        &mut self,
        change: ExternalChange,
        choice: ExternalChangeChoice,
    ) {
        if !self.activate(change.document) || self.document.path() != Some(change.path.as_path()) {
            self.rerun_after_external_change(change.rerun_after_resolution);
            return;
        }
        match choice {
            ExternalChangeChoice::KeepLocal => {
                self.keep_local_version(&change);
                if change.on_save {
                    self.save();
                } else {
                    self.set_status(
                        StatusKind::Info,
                        format!("Kept unsaved changes to {}", self.document.display_name()),
                    );
                }
                self.rerun_after_external_change(change.rerun_after_resolution);
            }
            ExternalChangeChoice::Cancel => {
                self.set_status(
                    StatusKind::Info,
                    format!("{} was not saved", self.document.display_name()),
                );
                self.rerun_after_external_change(change.rerun_after_resolution);
            }
            ExternalChangeChoice::Reload => self.reload_external_change(change),
            ExternalChangeChoice::SaveCopy => self.save_copy_then_reload(change),
        }
    }

    /// The user keeps the buffer over the version another program wrote:
    /// that version goes to the local history, since the next save replaces
    /// it, and becomes the baseline the save no longer asks about.
    fn keep_local_version(&mut self, change: &ExternalChange) {
        if let Err(error) = self.recovery.archive_path(&change.path) {
            log::warn!(
                "Cannot keep the disk version of {} in local history: {error}",
                change.path.display()
            );
        }
        self.document.adopt_disk_baseline(&change.disk);
    }

    /// Saving would replace a version of the file the editor has not seen:
    /// the user is asked first, with the dialog of the file watcher.
    fn ask_before_overwriting(&mut self, disk: DiskVersion) {
        let Some(path) = self.document.path().map(Path::to_path_buf) else {
            return;
        };
        let change = ExternalChange {
            document: self.document_generation,
            path,
            disk,
            show_comparison: false,
            rerun_after_resolution: false,
            on_save: true,
        };
        if self.dialogs.external_change.is_none() {
            self.dialogs.external_change = Some(change);
        } else {
            self.dialogs.external_queue.insert(0, change);
        }
        self.set_status(
            StatusKind::Warning,
            format!(
                "{} was changed by another program; nothing was saved yet",
                self.document.display_name()
            ),
        );
    }

    fn perform(&mut self, intent: Intent) {
        match intent {
            Intent::Close(documents) => {
                for generation in documents {
                    self.close_now(generation);
                }
            }
            Intent::Quit => {
                self.clear_recovery_session();
                self.allow_close = true;
                self.ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }
    }

    pub(super) fn new_file(&mut self) {
        self.add_tab(Document::untitled(""));
    }

    /// Asks for scripts and opens each in a tab.
    pub(super) fn choose_files(&mut self) {
        let mut dialog = rfd::FileDialog::new().add_filter("Python script", &["py"]);
        let folder = self
            .document
            .path()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .or_else(|| self.project.as_ref().map(|p| p.root().to_path_buf()));
        if let Some(folder) = folder {
            dialog = dialog.set_directory(folder);
        }
        for path in dialog.pick_files().unwrap_or_default() {
            self.open_path(&path);
        }
    }

    pub(super) fn open_example(&mut self, index: usize) {
        if let Some(example) = self.examples.get(index) {
            let document = Document::example(&example.title, example.source);
            self.add_tab(document);
            self.set_status(
                StatusKind::Info,
                "Examples are read-only; saving asks for a new location",
            );
        }
    }

    /// Shows the file at `path`: its tab when it is open, else a new one.
    pub(super) fn open_path(&mut self, path: &Path) {
        match self.find_open(path) {
            Some(open) => {
                self.activate(open);
            }
            None => self.open_path_with(path, Encoding::Utf8),
        }
    }

    /// Shows the file at `path` with the caret on `line`, from the
    /// traceback of a module the script imported.
    pub(super) fn open_path_at(&mut self, path: &Path, line: usize) {
        self.open_path(path);
        if self.find_open(path) == Some(self.document_generation) {
            self.editor.go_to_line(&self.document.text, line);
        }
    }

    /// Opens a file in a new tab; one that is not UTF-8 is opened after
    /// the user agreed to read it as `encoding` (and to convert it on save).
    pub fn open_path_with(&mut self, path: &Path, encoding: Encoding) {
        match Document::open_with(path, encoding) {
            Ok(document) => {
                self.add_tab(document);
                self.settings.add_recent_file(path);
                self.settings.last_file = Some(path.to_path_buf());
                if let Some(project) = &mut self.project {
                    project.reveal(path);
                }
                let status = match encoding {
                    Encoding::Utf8 => format!("Opened {}", path.display()),
                    _ => format!(
                        "Opened {} as {}; saving converts it to UTF-8",
                        path.display(),
                        encoding.label()
                    ),
                };
                self.set_status(StatusKind::Info, status);
            }
            Err(err) if is_not_utf8(&err) => {
                self.dialogs.legacy_encoding = Some(path.to_path_buf());
            }
            Err(err) => self
                .dialogs
                .error("Cannot open file", format!("{}\n\n{err}", path.display())),
        }
    }

    /// Replaces the shown document with another version of the same file
    /// (recovered, or from the local history), like a reload: the model
    /// stays, and the editor starts afresh.
    fn replace_document(&mut self, document: Document) {
        self.remember_breakpoints();
        self.remember_views();
        self.document = document;
        self.dialogs.external_change = None;
        self.dialogs.history = None;
        self.next_recovery = Instant::now() + CHECKPOINT_INTERVAL;
        self.editor.reset();
        self.restore_breakpoints();
        self.restore_views();
        if self.traceback_document == Some(self.document_generation) {
            self.traceback = None;
        }
    }

    /// Returns whether the document ended up saved. A file that another
    /// program changed since it was loaded or saved is not replaced without
    /// asking.
    pub(super) fn save(&mut self) -> bool {
        self.checkpoint_recovery();
        match self.document.unseen_disk_version() {
            Ok(None) => {}
            Ok(Some(disk)) => {
                self.ask_before_overwriting(disk);
                return false;
            }
            Err(error) => {
                let path = self
                    .document
                    .path()
                    .map_or_else(String::new, |path| path.display().to_string());
                self.dialogs.error(
                    "Cannot save",
                    format!(
                        "{path} could not be read to check it before saving:\n{error}\n\n\
                         Nothing was saved. Use Save As to save your edits elsewhere."
                    ),
                );
                return false;
            }
        }
        match self.document.save() {
            Ok(true) => {
                self.after_save();
                true
            }
            Ok(false) => {
                if self.document.example_title().is_some() {
                    self.set_status(
                        StatusKind::Info,
                        "Examples are read-only — choose a new location to save your changes",
                    );
                }
                self.save_as()
            }
            Err(err) => {
                self.dialogs.error("Cannot save", err.to_string());
                false
            }
        }
    }

    pub(super) fn save_as(&mut self) -> bool {
        self.checkpoint_recovery();
        let mut dialog = rfd::FileDialog::new()
            .add_filter("Python script", &["py"])
            .set_file_name(self.document.path().and_then(Path::file_name).map_or_else(
                || "script.py".to_owned(),
                |name| name.to_string_lossy().into_owned(),
            ));
        if let Some(dir) = self.document.path().and_then(Path::parent) {
            dialog = dialog.set_directory(dir);
        }
        let Some(mut path) = dialog.save_file() else {
            return false;
        };
        if path.extension().is_none() {
            path.set_extension("py");
        }
        if let Err(error) = self.recovery.archive_path(&path) {
            log::warn!("Cannot archive the previous Save As target: {error}");
        }
        match self.document.save_as(&path) {
            Ok(()) => {
                self.after_save();
                true
            }
            Err(err) => {
                self.dialogs
                    .error("Cannot save", format!("{}\n\n{err}", path.display()));
                false
            }
        }
    }

    fn save_copy_then_reload(&mut self, change: ExternalChange) {
        let stem = change
            .path
            .file_stem()
            .map_or_else(|| "script".into(), |name| name.to_string_lossy());
        let mut dialog = rfd::FileDialog::new()
            .add_filter("Python script", &["py"])
            .set_file_name(format!("{stem}.local-copy.py"));
        if let Some(directory) = change.path.parent() {
            dialog = dialog.set_directory(directory);
        }
        let Some(mut path) = dialog.save_file() else {
            self.dialogs.external_change = Some(change);
            return;
        };
        if path.extension().is_none() {
            path.set_extension("py");
        }
        match self.document.save_copy(&path) {
            Ok(()) => self.reload_external_change(change),
            Err(error) => {
                self.dialogs.error(
                    "Cannot save a copy",
                    format!("{}\n\n{error}", path.display()),
                );
                self.dialogs.external_change = Some(change);
            }
        }
    }

    fn reload_external_change(&mut self, change: ExternalChange) {
        self.checkpoint_recovery();
        let backup = if self.document.is_modified() {
            match self
                .document
                .create_recovery_backup(&self.cache_dir.join("recovery"))
            {
                Ok(path) => Some(path),
                Err(error) => {
                    self.dialogs.error(
                        "Cannot create recovery backup",
                        format!(
                            "The file was not reloaded and your edits are unchanged.\n\n{error}"
                        ),
                    );
                    self.dialogs.external_change = Some(change);
                    return;
                }
            }
        } else {
            None
        };

        match self.document.apply_disk_version(change.disk) {
            Ok(_) => {
                if let Some(path) = &backup {
                    logging::push(
                        Level::Info,
                        "recovery",
                        &format!("Saved unsaved edits to {}", path.display()),
                    );
                }
                let message = backup.map_or_else(
                    || format!("Reloaded {}", self.document.display_name()),
                    |path| {
                        format!(
                            "Reloaded {}; recovery backup: {}",
                            self.document.display_name(),
                            path.display()
                        )
                    },
                );
                self.set_status(StatusKind::Info, message);
                self.checkpoint_recovery();
                self.rerun_after_external_change(change.rerun_after_resolution);
            }
            Err(error) => self
                .dialogs
                .error("Cannot reload the script", error.to_string()),
        }
    }

    /// After a conflict was resolved: the next waiting one is asked about,
    /// with its file shown, or the script renders if a change asked for it.
    fn rerun_after_external_change(&mut self, rerun: bool) {
        while !self.dialogs.external_queue.is_empty() {
            let mut next = self.dialogs.external_queue.remove(0);
            if !self.activate(next.document) {
                continue;
            }
            next.rerun_after_resolution |= rerun;
            self.dialogs.external_change = Some(next);
            return;
        }
        if rerun {
            self.start_run(false);
        }
    }

    /// Shows the breakpoints the document had when it was last open.
    pub(super) fn restore_breakpoints(&mut self) {
        if let Some(path) = self.document.path() {
            let saved = self.settings.breakpoints_of(path).to_vec();
            self.editor.restore_breakpoints(&self.document.text, &saved);
        }
    }

    /// Keeps the breakpoints of the document for the next time it opens.
    pub(super) fn remember_breakpoints(&mut self) {
        if let Some(path) = self.document.path().map(Path::to_path_buf) {
            let saved = self.editor.saved_breakpoints(&self.document.text);
            self.settings.remember_breakpoints(&path, saved);
        }
    }

    /// Shows the named views the document had when it was last open.
    pub(super) fn restore_views(&mut self) {
        self.viewer.named_views = self
            .document
            .path()
            .map(|path| self.settings.views_of(path).to_vec())
            .unwrap_or_default();
    }

    /// Keeps the named views of the document for the next time it opens.
    pub(super) fn remember_views(&mut self) {
        if let Some(path) = self.document.path().map(Path::to_path_buf) {
            let views = self.viewer.named_views.clone();
            self.settings.remember_views(&path, views);
        }
    }

    fn after_save(&mut self) {
        // Under the new name after a Save As.
        self.remember_breakpoints();
        self.remember_views();
        if let Err(error) = self.recovery.archive(&self.document) {
            log::warn!("Cannot add the saved document to local history: {error}");
        }
        // Other tabs may still have unsaved edits.
        self.checkpoint_recovery();
        if let Some(path) = self.document.path().map(Path::to_path_buf) {
            self.settings.add_recent_file(&path);
            self.settings.last_file = Some(path.clone());
            self.set_status(StatusKind::Success, format!("Saved {}", path.display()));
        }
        // The saved script may import other modules now.
        self.request_imports();
    }

    pub(super) fn show_local_history(&mut self) {
        self.checkpoint_recovery();
        match self.recovery.history(self.document.path()) {
            Ok(versions) if versions.is_empty() => self.dialogs.info(
                "Local history",
                "No recovery versions are available for this document yet.",
            ),
            Ok(versions) => {
                self.dialogs.history = Some(HistoryDialog {
                    document: self.document_generation,
                    versions,
                    selected: 0,
                });
            }
            Err(error) => self
                .dialogs
                .error("Cannot read local history", error.to_string()),
        }
    }

    pub(super) fn checkpoint_recovery(&mut self) {
        // The startup documents are separate clean buffers while the
        // recovered session awaits a decision. They must never clear that
        // pending session.
        // When quitting was confirmed, the edits were saved or deliberately
        // discarded ("Don't save"); eframe's final `save` must not bring
        // them back as an interrupted session on the next start.
        if self.dialogs.recovery_session.is_some() || self.allow_close {
            return;
        }
        match self.checkpoint_documents() {
            Ok(_) => self.recovery_error_reported = false,
            Err(error) => {
                log::warn!("Cannot create recovery checkpoint: {error}");
                if !self.recovery_error_reported {
                    self.set_status(
                        StatusKind::Warning,
                        "Automatic recovery is unavailable; see the Log panel",
                    );
                    self.recovery_error_reported = true;
                }
            }
        }
    }

    pub(super) fn clear_recovery_session(&mut self) {
        if let Err(error) = self.recovery.clear_session() {
            log::warn!("Cannot clear recovery session: {error}");
        }
    }

    pub(super) fn poll_recovery(&mut self, ctx: &egui::Context) {
        if self.dialogs.recovery_session.is_some() {
            return;
        }
        if !self.any_modified() {
            self.clear_recovery_session();
            self.next_recovery = Instant::now() + CHECKPOINT_INTERVAL;
            return;
        }

        let now = Instant::now();
        if now >= self.next_recovery {
            self.checkpoint_recovery();
            self.next_recovery = now + CHECKPOINT_INTERVAL;
        }
        ctx.request_repaint_after(self.next_recovery.saturating_duration_since(now));
    }
}
