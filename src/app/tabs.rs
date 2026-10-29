//! The editor tabs: the documents open besides the one shown, switching
//! between them, closing them, the session they are restored from, and the
//! script F5 runs.
//!
//! The shown tab's document and editor are `AppState::document` and
//! `AppState::editor`, so everything that works on "the document" works on
//! the one the user sees. The other tabs wait in [`EditorTabs`], each with
//! its own editor (caret, scrolling, undo history, breakpoints), until they
//! are shown again.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::{AppState, StatusKind};
use crate::dialogs::Intent;
use crate::document::{Document, is_not_utf8};
use crate::editor::CodeEditor;
use crate::project;
use crate::worker::protocol::DebugAction;

/// A document open in a tab that is not shown, with its editor.
struct Parked {
    document: Document,
    editor: CodeEditor,
    generation: u64,
}

/// The open tabs other than the shown one, and the order of all of them.
pub struct EditorTabs {
    /// The generation of every open document, in tab order.
    order: Vec<u64>,
    parked: Vec<Parked>,
    next_generation: u64,
}

impl EditorTabs {
    /// The tabs of a session that starts with the document `generation`.
    pub fn new(generation: u64) -> Self {
        Self {
            order: vec![generation],
            parked: Vec::new(),
            next_generation: generation + 1,
        }
    }
}

/// A tab, as the tab strip and the menus show it.
#[derive(Clone, Debug, PartialEq)]
pub struct TabInfo {
    pub generation: u64,
    /// The file name, with its folder when another tab has the same name.
    pub name: String,
    pub path: Option<PathBuf>,
    pub modified: bool,
    pub active: bool,
    /// The project's main script, which F5 runs.
    pub main: bool,
}

/// The egui id of the editor of the document `generation`.
fn editor_id(generation: u64) -> egui::Id {
    egui::Id::new(("code_editor_text", generation))
}

impl AppState {
    /// The open tabs, in order.
    pub fn tabs(&self) -> Vec<TabInfo> {
        let main = self.project.as_ref().and_then(|p| p.main.as_deref());
        let mut tabs: Vec<TabInfo> = self
            .documents_in_order()
            .into_iter()
            .map(|(generation, document)| TabInfo {
                generation,
                name: document.display_name(),
                path: document.path().map(Path::to_path_buf),
                modified: document.is_modified(),
                active: generation == self.document_generation,
                main: main.is_some() && document.path() == main,
            })
            .collect();
        let names: Vec<String> = tabs.iter().map(|tab| tab.name.clone()).collect();
        for tab in &mut tabs {
            let twins = names.iter().filter(|name| **name == tab.name).count();
            if twins > 1
                && let Some(folder) = tab.path.as_deref().and_then(Path::parent)
                && let Some(folder) = folder.file_name()
            {
                tab.name = format!("{} — {}", tab.name, folder.to_string_lossy());
            }
        }
        tabs
    }

    /// Every open document, the shown one first.
    fn open_documents(&self) -> impl Iterator<Item = (u64, &Document)> {
        std::iter::once((self.document_generation, &self.document)).chain(
            self.tabs
                .parked
                .iter()
                .map(|parked| (parked.generation, &parked.document)),
        )
    }

    /// Every open document, in tab order.
    pub(super) fn documents_in_order(&self) -> Vec<(u64, &Document)> {
        let mut documents: Vec<_> = self.open_documents().collect();
        let position = |generation: u64| {
            self.tabs
                .order
                .iter()
                .position(|&g| g == generation)
                .unwrap_or(usize::MAX)
        };
        documents.sort_by_key(|(generation, _)| position(*generation));
        documents
    }

    pub fn document_of(&self, generation: u64) -> Option<&Document> {
        self.open_documents()
            .find(|(g, _)| *g == generation)
            .map(|(_, document)| document)
    }

    /// The document `generation` and its editor, shown or not.
    pub(super) fn open_mut(&mut self, generation: u64) -> Option<(&mut Document, &mut CodeEditor)> {
        if generation == self.document_generation {
            return Some((&mut self.document, &mut self.editor));
        }
        self.tabs
            .parked
            .iter_mut()
            .find(|parked| parked.generation == generation)
            .map(|parked| (&mut parked.document, &mut parked.editor))
    }

    fn editors_mut(&mut self) -> impl Iterator<Item = &mut CodeEditor> {
        std::iter::once(&mut self.editor)
            .chain(self.tabs.parked.iter_mut().map(|parked| &mut parked.editor))
    }

    /// Current breakpoints of the run target and every open project module.
    /// Each editor syncs markers before reporting line numbers.
    pub(super) fn open_breakpoints(
        &mut self,
        target: u64,
    ) -> (Vec<usize>, BTreeMap<String, Vec<usize>>) {
        let Some((document, editor)) = self.open_mut(target) else {
            return (Vec::new(), BTreeMap::new());
        };
        editor.sync_markers(&document.text);
        let main_path = document.path().map(Path::to_path_buf);
        let main = editor.breakpoints.iter().copied().collect();
        let mut modules = BTreeMap::new();
        let mut add = |document: &Document, editor: &mut CodeEditor| {
            let Some(path) = document.path() else { return };
            if main_path.as_deref() == Some(path) {
                return;
            }
            editor.sync_markers(&document.text);
            modules.insert(
                path.display().to_string(),
                editor.breakpoints.iter().copied().collect(),
            );
        };
        if target != self.document_generation {
            add(&self.document, &mut self.editor);
        }
        for parked in &mut self.tabs.parked {
            if parked.generation != target {
                add(&parked.document, &mut parked.editor);
            }
        }
        (main, modules)
    }

    pub(super) fn clear_line_profiles(&mut self) {
        for editor in self.editors_mut() {
            editor.line_profile.clear();
        }
    }

    pub fn any_modified(&self) -> bool {
        self.open_documents()
            .any(|(_, document)| document.is_modified())
    }

    /// The tab of the file at `path`, written exactly so.
    fn find_open_exactly(&self, path: &Path) -> Option<u64> {
        self.open_documents()
            .find(|(_, document)| document.path() == Some(path))
            .map(|(generation, _)| generation)
    }

    /// The tab of the file at `path`, however the path is written.
    pub(super) fn find_open(&self, path: &Path) -> Option<u64> {
        self.find_open_exactly(path).or_else(|| {
            let wanted = path.canonicalize().ok()?;
            self.open_documents()
                .find(|(_, document)| {
                    document
                        .path()
                        .and_then(|open| open.canonicalize().ok())
                        .is_some_and(|open| open == wanted)
                })
                .map(|(generation, _)| generation)
        })
    }

    /// Shows the tab of the document `generation`. Returns whether it is
    /// open.
    pub(super) fn activate(&mut self, generation: u64) -> bool {
        if generation == self.document_generation {
            return true;
        }
        let Some(index) = self
            .tabs
            .parked
            .iter()
            .position(|parked| parked.generation == generation)
        else {
            return false;
        };
        self.remember_views();
        let shown = self.tabs.parked.swap_remove(index);
        self.editor.close_popups();
        let hidden = Parked {
            document: std::mem::replace(&mut self.document, shown.document),
            editor: std::mem::replace(&mut self.editor, shown.editor),
            generation: std::mem::replace(&mut self.document_generation, shown.generation),
        };
        self.tabs.parked.push(hidden);
        self.after_tab_change();
        true
    }

    /// What follows a change of the shown document.
    fn after_tab_change(&mut self) {
        self.restore_views();
        // Local history is about the document that was shown.
        self.dialogs.history = None;
        self.editor.focus();
    }

    /// Shows the next tab, or the previous one, wrapping around.
    pub(super) fn cycle_tab(&mut self, forward: bool) {
        let order = &self.tabs.order;
        let Some(position) = order.iter().position(|&g| g == self.document_generation) else {
            return;
        };
        let count = order.len();
        let next = if forward {
            (position + 1) % count
        } else {
            (position + count - 1) % count
        };
        self.activate(order[next]);
    }

    /// A new generation and an editor showing the breakpoints `document`
    /// had when it was last open.
    fn editor_for(&mut self, document: &Document) -> (u64, CodeEditor) {
        let generation = self.tabs.next_generation;
        self.tabs.next_generation += 1;
        let mut editor = CodeEditor::with_id(editor_id(generation));
        if let Some(path) = document.path() {
            editor.restore_breakpoints(&document.text, self.settings.breakpoints_of(path));
        }
        (generation, editor)
    }

    /// An untitled buffer that was never edited: opening a file replaces
    /// it rather than adding a tab next to it.
    fn shows_pristine_buffer(&self) -> bool {
        self.document.path().is_none()
            && self.document.example_title().is_none()
            && !self.document.is_modified()
    }

    /// Opens `document` in a new tab after the shown one and shows it. A
    /// shown untitled buffer that was never edited is replaced instead, as
    /// the startup script is by the first file opened.
    pub(super) fn add_tab(&mut self, document: Document) {
        let replace = self.shows_pristine_buffer();
        let previous = self.document_generation;
        if replace {
            self.forget_document(previous);
        }
        let (generation, editor) = self.editor_for(&document);
        self.remember_views();
        self.editor.close_popups();
        let hidden = Parked {
            document: std::mem::replace(&mut self.document, document),
            editor: std::mem::replace(&mut self.editor, editor),
            generation: std::mem::replace(&mut self.document_generation, generation),
        };
        let position = self.tabs.order.iter().position(|&g| g == previous);
        match (replace, position) {
            (true, Some(position)) => self.tabs.order[position] = generation,
            (false, Some(position)) => {
                self.tabs.order.insert(position + 1, generation);
                self.tabs.parked.push(hidden);
            }
            (true, None) => self.tabs.order.push(generation),
            (false, None) => {
                self.tabs.order.push(generation);
                self.tabs.parked.push(hidden);
            }
        }
        self.after_tab_change();
    }

    /// Opens `document` in a tab after the shown one, without showing it.
    fn add_background_tab(&mut self, document: Document) -> u64 {
        let (generation, editor) = self.editor_for(&document);
        let position = self
            .tabs
            .order
            .iter()
            .position(|&g| g == self.document_generation)
            .map_or(self.tabs.order.len(), |position| position + 1);
        self.tabs.order.insert(position, generation);
        self.tabs.parked.push(Parked {
            document,
            editor,
            generation,
        });
        generation
    }

    /// Closes the tabs of `documents`, asking about unsaved changes first.
    pub(super) fn close_tabs(&mut self, documents: Vec<u64>) {
        self.guard_unsaved(Intent::Close(documents));
    }

    /// Closes a tab without asking. Closing the last one leaves an empty
    /// untitled buffer: there is always a document.
    pub(super) fn close_now(&mut self, generation: u64) {
        if self.document_of(generation).is_none() {
            return;
        }
        self.forget_document(generation);
        if generation == self.document_generation {
            let order = &self.tabs.order;
            let neighbor = order
                .iter()
                .position(|&g| g == generation)
                .and_then(|position| {
                    order
                        .get(position + 1)
                        .or_else(|| position.checked_sub(1).and_then(|left| order.get(left)))
                })
                .copied();
            match neighbor {
                Some(neighbor) => {
                    self.activate(neighbor);
                }
                None => self.add_tab(Document::untitled("")),
            }
        }
        self.tabs
            .parked
            .retain(|parked| parked.generation != generation);
        self.tabs.order.retain(|&g| g != generation);
        // Its edits were saved or discarded.
        self.checkpoint_recovery();
    }

    /// Before a document goes away: its breakpoints are remembered for the
    /// next time it opens, and a debugging session of it ends.
    fn forget_document(&mut self, generation: u64) {
        if let Some((document, editor)) = self.open_mut(generation)
            && let Some(path) = document.path().map(Path::to_path_buf)
        {
            let saved = editor.saved_breakpoints(&document.text);
            self.settings.remember_breakpoints(&path, saved);
        }
        let paused_here = self
            .open_mut(generation)
            .is_some_and(|(_, editor)| editor.debug_line.is_some());
        if self.is_debugging()
            && (self.running_document() == Some(generation) || paused_here)
            && self.execution.abandon()
        {
            self.debug_command(DebugAction::Stop);
        }
    }

    /// The document of the run going on, if any, also while it is paused
    /// in the debugger: closing that document ends the session.
    pub(super) fn running_document(&self) -> Option<u64> {
        let running = self.execution.phase().run_id()?;
        self.run_script
            .as_ref()
            .filter(|(id, _)| *id == running)
            .map(|(_, script)| script.document)
    }

    /// Checkpoints every open document, in tab order, for crash recovery.
    pub(super) fn checkpoint_documents(&mut self) -> std::io::Result<bool> {
        let order = &self.tabs.order;
        let mut documents: Vec<(u64, &Document)> =
            std::iter::once((self.document_generation, &self.document))
                .chain(self.tabs.parked.iter().map(|p| (p.generation, &p.document)))
                .collect();
        documents.sort_by_key(|(generation, _)| {
            order
                .iter()
                .position(|g| g == generation)
                .unwrap_or(usize::MAX)
        });
        let documents: Vec<&Document> = documents.into_iter().map(|(_, d)| d).collect();
        self.recovery.checkpoint_all(&documents)
    }

    /// Keeps the breakpoints of every open file for the next time it opens.
    pub(super) fn remember_all_breakpoints(&mut self) {
        let shown = std::iter::once((&self.document, &mut self.editor));
        let parked = self
            .tabs
            .parked
            .iter_mut()
            .map(|parked| (&parked.document, &mut parked.editor));
        let saved: Vec<_> = shown
            .chain(parked)
            .filter_map(|(document, editor)| {
                let path = document.path()?.to_path_buf();
                Some((path, editor.saved_breakpoints(&document.text)))
            })
            .collect();
        for (path, lines) in saved {
            self.settings.remember_breakpoints(&path, lines);
        }
    }

    /// The modified documents among those `intent` would close.
    pub fn unsaved_of(&self, intent: &Intent) -> Vec<u64> {
        let modified = |generation: &u64| {
            self.document_of(*generation)
                .is_some_and(Document::is_modified)
        };
        match intent {
            Intent::Close(documents) => documents.iter().copied().filter(modified).collect(),
            Intent::Quit => self
                .documents_in_order()
                .into_iter()
                .map(|(generation, _)| generation)
                .filter(modified)
                .collect(),
        }
    }

    /// The names of `documents`, for messages.
    pub fn names_of(&self, documents: &[u64]) -> Vec<String> {
        documents
            .iter()
            .filter_map(|&generation| self.document_of(generation))
            .map(Document::display_name)
            .collect()
    }

    /// Saves each of `documents` that is modified, showing the ones that
    /// need a name first. Returns whether all of them ended up saved.
    pub(super) fn save_documents(&mut self, documents: &[u64]) -> bool {
        let shown = self.document_generation;
        for &generation in documents {
            if !self
                .document_of(generation)
                .is_some_and(Document::is_modified)
            {
                continue;
            }
            self.activate(generation);
            if !self.save() {
                return false;
            }
        }
        self.activate(shown);
        true
    }

    /// Saves every modified document.
    pub(super) fn save_all(&mut self) {
        let modified = self.unsaved_of(&Intent::Quit);
        if modified.is_empty() {
            self.set_status(StatusKind::Info, "Every file is saved");
        } else if self.save_documents(&modified) && modified.len() > 1 {
            self.set_status(
                StatusKind::Success,
                format!("Saved {} files", modified.len()),
            );
        }
    }

    pub(super) fn clear_debug_lines(&mut self) {
        for editor in self.editors_mut() {
            editor.set_debug_line(None);
        }
    }

    // -- the script F5 runs -----------------------------------------------------

    /// The project's main script, while a file of the project is shown.
    pub fn main_script(&self) -> Option<&Path> {
        let project = self.project.as_ref()?;
        let main = project.main.as_deref()?;
        let shown = self.document.path()?;
        project.contains(shown).then_some(main)
    }

    /// The path of the script F5 runs, if it has one.
    pub(super) fn run_target_path(&self) -> Option<PathBuf> {
        self.main_script()
            .or(self.document.path())
            .map(Path::to_path_buf)
    }

    /// The open document F5 runs: the project's main script while a file
    /// of the project is shown and the script is open, else the shown one.
    pub fn run_target(&self) -> u64 {
        self.main_script()
            .and_then(|main| self.find_open_exactly(main))
            .unwrap_or(self.document_generation)
    }

    /// The document to run now, opening the main script in a tab (without
    /// showing it) when it is not open yet.
    pub(super) fn prepare_run_target(&mut self) -> u64 {
        let Some(main) = self.main_script().map(Path::to_path_buf) else {
            return self.document_generation;
        };
        if let Some(open) = self.find_open(&main) {
            return open;
        }
        match Document::open(&main) {
            Ok(document) => self.add_background_tab(document),
            Err(error) => {
                let message = format!(
                    "Cannot open the main script {}: {error}; rendering {} instead",
                    main.display(),
                    self.document.display_name()
                );
                log::warn!("{message}");
                self.set_status(StatusKind::Warning, message);
                self.document_generation
            }
        }
    }

    /// Open scripts next to the one `target` runs that have unsaved
    /// changes: Python imports their saved version.
    pub(super) fn unsaved_modules(&self, target: u64) -> Vec<String> {
        let Some(folder) = self
            .document_of(target)
            .and_then(Document::path)
            .and_then(Path::parent)
        else {
            return Vec::new();
        };
        self.documents_in_order()
            .into_iter()
            .filter(|(generation, document)| {
                *generation != target
                    && document.is_modified()
                    && document
                        .path()
                        .is_some_and(|path| path.starts_with(folder) && project::is_script(path))
            })
            .map(|(_, document)| document.display_name())
            .collect()
    }

    // -- sessions -----------------------------------------------------------

    /// Remembers the open tabs and the project for the next session.
    pub(super) fn remember_session(&mut self) {
        self.settings.open_files = self
            .documents_in_order()
            .into_iter()
            .filter_map(|(_, document)| document.path().map(Path::to_path_buf))
            .collect();
        if let Some(path) = self.document.path() {
            self.settings.last_file = Some(path.to_path_buf());
        }
        self.settings.project = self.project.as_ref().map(|p| p.root().to_path_buf());
        self.remember_project();
    }

    /// Opens the project and the tabs of the last session, then the file
    /// given on the command line, which is shown. Returns the problems to
    /// report; a file that is not UTF-8 is asked about like a file opened
    /// later.
    pub(super) fn restore_session(&mut self, startup: Option<PathBuf>) -> Vec<String> {
        let mut problems = Vec::new();
        if let Some(root) = self.settings.project.clone()
            && root.is_dir()
            && let Err(error) = self.open_project(&root)
        {
            log::warn!("Cannot open the project {}: {error}", root.display());
        }

        let mut files = self.settings.open_files.clone();
        if files.is_empty() {
            // Settings of a release without tabs.
            files.extend(self.settings.last_file.clone());
        }
        for path in files.into_iter().filter(|path| path.is_file()) {
            if self.find_open_exactly(&path).is_some() {
                continue;
            }
            match Document::open(&path) {
                Ok(document) => self.add_tab(document),
                Err(error) if is_not_utf8(&error) => {
                    log::info!("Not reopening {}: {error}", path.display());
                }
                Err(error) => {
                    log::warn!("Cannot open {}: {error}", path.display());
                    problems.push(format!("Cannot reopen {}:\n{error}", path.display()));
                }
            }
        }

        // Kept in the session, where a relative path would not survive a
        // start from another folder.
        let startup = startup.map(|path| std::path::absolute(&path).unwrap_or(path));
        match startup {
            Some(path) => match self.find_open(&path) {
                Some(open) => {
                    self.activate(open);
                }
                None => match Document::open(&path) {
                    Ok(document) => self.add_tab(document),
                    Err(error) if is_not_utf8(&error) => {
                        self.dialogs.legacy_encoding = Some(path);
                    }
                    Err(error) => {
                        log::warn!("Cannot open {}: {error}", path.display());
                        problems.push(format!(
                            "Cannot open the startup file {}:\n{error}",
                            path.display()
                        ));
                    }
                },
            },
            None => {
                if let Some(open) = self
                    .settings
                    .last_file
                    .clone()
                    .and_then(|last| self.find_open_exactly(&last))
                {
                    self.activate(open);
                }
            }
        }
        problems
    }
}

#[cfg(test)]
#[path = "../../tests/rust/app/test_tabs.rs"]
mod tests;
