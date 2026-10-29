//! The project folder: opening and closing it, its main script, and the
//! file operations of the Project panel, which keep the open tabs and what
//! is remembered of each file in step with the disk.

use std::io;
use std::path::{Path, PathBuf};

use super::{AppState, StatusKind};
use crate::dialogs::FilePrompt;
use crate::document::Document;
use crate::project::{self, Project};
use crate::settings::ProjectState;

impl AppState {
    /// Asks for a folder and opens it as the project.
    pub(super) fn choose_project_folder(&mut self) {
        let mut dialog = rfd::FileDialog::new().set_title("Open Folder");
        let start = self
            .project
            .as_ref()
            .map(|project| project.root().to_path_buf())
            .or_else(|| {
                self.document
                    .path()
                    .and_then(Path::parent)
                    .map(Path::to_path_buf)
            });
        if let Some(start) = start {
            dialog = dialog.set_directory(start);
        }
        if let Some(folder) = dialog.pick_folder() {
            self.open_project_folder(&folder);
        }
    }

    /// Opens `folder` as the project, reporting the outcome.
    pub(super) fn open_project_folder(&mut self, folder: &Path) {
        match self.open_project(folder) {
            Ok(()) => {
                let project = self.project.as_ref().expect("just opened");
                let main = project.main.as_deref().map_or_else(String::new, |main| {
                    format!("; F5 renders {}", project.relative(main).display())
                });
                let text = format!("Opened the folder {}{main}", project.name());
                self.set_status(StatusKind::Info, text);
            }
            Err(error) => self.dialogs.error(
                "Cannot open folder",
                format!("{}\n\n{error}", folder.display()),
            ),
        }
    }

    /// Makes `folder` the project, with the main script and expanded
    /// folders it had last time. A folder opened for the first time gets
    /// the main script [`project::guess_main`] finds.
    pub(super) fn open_project(&mut self, folder: &Path) -> io::Result<()> {
        let mut project = Project::open(folder)?;
        self.remember_project();
        match self.settings.project_state(project.root()) {
            Some(state) => {
                project.main = state.main.clone().filter(|main| main.is_file());
                project.restore_expanded(&state.expanded.clone());
            }
            None => project.main = project::guess_main(project.root()),
        }
        if let Some(path) = self.document.path() {
            project.reveal(path);
        }
        self.settings.project = Some(project.root().to_path_buf());
        self.project = Some(project);
        self.remember_project();
        Ok(())
    }

    pub(super) fn close_project(&mut self) {
        self.remember_project();
        self.project = None;
        self.settings.project = None;
    }

    /// Keeps the project's main script and expanded folders.
    pub(super) fn remember_project(&mut self) {
        if let Some(project) = &self.project {
            self.settings.remember_project(ProjectState {
                root: project.root().to_path_buf(),
                main: project.main.clone(),
                expanded: project.expanded(),
            });
        }
    }

    /// Makes `path` the script F5 runs while a project file is shown, or
    /// lets F5 run the shown file again.
    pub(super) fn set_main_script(&mut self, path: Option<PathBuf>) {
        let Some(project) = &mut self.project else {
            return;
        };
        let text = match &path {
            Some(path) if !project.contains(path) => {
                self.set_status(
                    StatusKind::Warning,
                    "The main script must be a file of the project folder",
                );
                return;
            }
            Some(path) => format!(
                "F5 now renders {} while a file of the project is shown",
                project.relative(path).display()
            ),
            None => "F5 now renders the file shown".to_owned(),
        };
        project.main = path;
        self.remember_project();
        self.set_status(StatusKind::Info, text);
    }

    /// Carries out a confirmed prompt of the Project panel. Returns the
    /// error to show in the prompt, which then stays open.
    pub fn confirm_file_prompt(&mut self, prompt: &FilePrompt) -> Option<String> {
        let result = match prompt {
            FilePrompt::NewFile { folder, name, .. } => self.create_project_file(folder, name),
            FilePrompt::NewFolder { folder, name, .. } => self.create_project_folder(folder, name),
            FilePrompt::Rename { path, name, .. } => self.rename_project_path(path, name),
            FilePrompt::Delete { path, .. } => self.delete_project_path(path),
        };
        result.err()
    }

    /// Creates a file in `folder` and opens it. Errors are for the prompt.
    pub fn create_project_file(&mut self, folder: &Path, name: &str) -> Result<(), String> {
        let path = project::create_file(folder, name).map_err(|error| error.to_string())?;
        if let Some(project) = &mut self.project {
            project.reveal(&path);
            project.refresh();
        }
        match Document::open(&path) {
            Ok(document) => {
                self.add_tab(document);
                self.settings.add_recent_file(&path);
                self.set_status(StatusKind::Success, format!("Created {}", path.display()));
            }
            Err(error) => self.set_status(
                StatusKind::Warning,
                format!(
                    "Created {}, which cannot be opened: {error}",
                    path.display()
                ),
            ),
        }
        Ok(())
    }

    pub fn create_project_folder(&mut self, folder: &Path, name: &str) -> Result<(), String> {
        let path = project::create_folder(folder, name).map_err(|error| error.to_string())?;
        if let Some(project) = &mut self.project {
            project.reveal(&path);
            if !project.is_expanded(&path) {
                project.toggle(&path);
            }
            project.refresh();
        }
        self.set_status(StatusKind::Success, format!("Created {}", path.display()));
        Ok(())
    }

    /// Renames a file or folder. Open tabs, the main script, breakpoints,
    /// named views and recent files follow it; unsaved edits stay unsaved.
    pub fn rename_project_path(&mut self, path: &Path, name: &str) -> Result<(), String> {
        let target = project::rename(path, name).map_err(|error| error.to_string())?;
        // The views of the shown file are in the viewer, not yet remembered.
        self.remember_views();
        self.remember_all_breakpoints();
        for generation in self
            .documents_in_order()
            .into_iter()
            .map(|(g, _)| g)
            .collect::<Vec<_>>()
        {
            if let Some((document, _)) = self.open_mut(generation)
                && let Some(moved) = document
                    .path()
                    .and_then(|open| project::relocated(open, path, &target))
            {
                document.set_path(moved);
            }
        }
        self.settings.relocate(path, &target);
        if let Some(project) = &mut self.project {
            project.relocate(path, &target);
        }
        self.remember_project();
        self.set_status(
            StatusKind::Success,
            format!("Renamed {} to {}", path.display(), target.display()),
        );
        Ok(())
    }

    /// Open documents in or at `path` with unsaved changes.
    pub fn unsaved_below(&self, path: &Path) -> Vec<String> {
        self.documents_in_order()
            .into_iter()
            .filter(|(_, document)| {
                document.is_modified() && document.path().is_some_and(|open| open.starts_with(path))
            })
            .map(|(_, document)| document.display_name())
            .collect()
    }

    /// Deletes a file, or a folder and everything in it, after the prompt
    /// confirmed it. The open tabs of what was deleted close; their last
    /// contents, and a deleted script, stay in the local history.
    pub fn delete_project_path(&mut self, path: &Path) -> Result<(), String> {
        if self
            .project
            .as_ref()
            .is_some_and(|project| project.root() == path)
        {
            return Err("The project folder itself cannot be deleted here".to_owned());
        }
        let below: Vec<u64> = self
            .documents_in_order()
            .into_iter()
            .filter(|(_, document)| document.path().is_some_and(|open| open.starts_with(path)))
            .map(|(generation, _)| generation)
            .collect();
        let copies: Vec<Document> = below
            .iter()
            .filter_map(|&generation| self.document_of(generation).cloned())
            .collect();
        for document in &copies {
            if let Err(error) = self.recovery.archive(document) {
                log::warn!(
                    "Cannot keep {} in local history: {error}",
                    document.display_name()
                );
            }
        }
        if path.is_file()
            && let Err(error) = self.recovery.archive_path(path)
        {
            log::warn!("Cannot keep {} in local history: {error}", path.display());
        }
        project::delete(path).map_err(|error| error.to_string())?;
        for generation in below {
            self.close_now(generation);
        }
        if let Some(project) = &mut self.project {
            project.forget(path);
        }
        self.remember_project();
        self.set_status(StatusKind::Info, format!("Deleted {}", path.display()));
        Ok(())
    }

    /// Opens the system's file manager on `path`'s folder.
    pub(super) fn reveal_in_file_manager(&mut self, path: &Path) {
        let folder = if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or(path)
        };
        if let Err(error) = open::that(folder) {
            self.set_status(
                StatusKind::Error,
                format!("Cannot open the file manager: {error}"),
            );
        }
    }
}
