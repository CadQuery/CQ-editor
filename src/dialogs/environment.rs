//! Creating a Python environment with CadQuery, or installing packages into
//! the interpreter in use, from Preferences > General (FEAT-E3).

use std::path::{Path, PathBuf};
use std::time::Duration;

use egui_phosphor::regular as icon;

use crate::commands::Action;
use crate::environments::{
    DEFAULT_PACKAGES, Job, JobState, create_steps, install_steps, parse_packages,
};
use crate::worker::launch::venv_python;

/// What the dialog does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    /// A new virtual environment with CadQuery and jedi.
    Create,
    /// Packages into an existing interpreter.
    Install,
}

pub struct EnvironmentDialog {
    mode: Mode,
    /// Create: the environment's folder and the Python that makes it.
    folder: String,
    base: String,
    /// Install: the interpreter that gets the packages.
    target: String,
    packages: String,
    /// Interpreters found on this computer, to pick from.
    candidates: Vec<PathBuf>,
    job: Option<Job>,
    problem: Option<String>,
}

impl EnvironmentDialog {
    /// Creating an environment in `folder` with the interpreter `base`.
    pub fn create(folder: PathBuf, base: PathBuf, candidates: Vec<PathBuf>) -> Self {
        Self {
            mode: Mode::Create,
            folder: folder.display().to_string(),
            base: base.display().to_string(),
            target: String::new(),
            packages: DEFAULT_PACKAGES.to_owned(),
            candidates,
            job: None,
            problem: None,
        }
    }

    /// Installing packages into the interpreter `python`.
    pub fn install(python: PathBuf, candidates: Vec<PathBuf>) -> Self {
        Self {
            mode: Mode::Install,
            folder: String::new(),
            base: String::new(),
            target: python.display().to_string(),
            packages: String::new(),
            candidates,
            job: None,
            problem: None,
        }
    }

    fn running(&self) -> bool {
        self.job
            .as_ref()
            .is_some_and(|job| *job.state() == JobState::Running)
    }

    /// Checks the fields and starts the commands.
    fn start(&mut self) {
        self.problem = None;
        let packages = match parse_packages(&self.packages) {
            Ok(packages) => packages,
            Err(problem) => {
                self.problem = Some(problem);
                return;
            }
        };
        let steps = match self.mode {
            Mode::Create => {
                let folder = PathBuf::from(self.folder.trim());
                if self.folder.trim().is_empty() {
                    self.problem = Some("Choose a folder for the environment".to_owned());
                    return;
                }
                if std::fs::read_dir(&folder).is_ok_and(|mut entries| entries.next().is_some()) {
                    self.problem = Some(format!(
                        "{} is not empty; choose a new or empty folder",
                        folder.display()
                    ));
                    return;
                }
                create_steps(Path::new(self.base.trim()), &folder, &packages)
            }
            Mode::Install => install_steps(Path::new(self.target.trim()), &packages),
        };
        self.job = Some(Job::start(steps));
    }

    /// Returns `false` once the window was closed (which stops a running
    /// job).
    pub fn window(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) -> bool {
        if let Some(job) = &mut self.job
            && *job.poll() == JobState::Running
        {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        let title = match self.mode {
            Mode::Create => format!("{} Create Python Environment", icon::PLUS_CIRCLE),
            Mode::Install => format!("{} Install Packages", icon::PACKAGE),
        };
        let mut open = true;
        egui::Window::new(title)
            .open(&mut open)
            .collapsible(false)
            .default_size([620.0, 420.0])
            .show(ctx, |ui| {
                ui.add_enabled_ui(!self.running(), |ui| self.fields(ui));
                ui.separator();
                self.progress(ui, actions);
            });
        open
    }

    fn fields(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("environment_grid")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                match self.mode {
                    Mode::Create => {
                        ui.label("Folder");
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.folder).desired_width(380.0),
                            );
                            if ui
                                .button(icon::FOLDER_OPEN)
                                .on_hover_text("Browse…")
                                .clicked()
                                && let Some(folder) = rfd::FileDialog::new().pick_folder()
                            {
                                self.folder = folder.display().to_string();
                            }
                        });
                        ui.end_row();
                        ui.label("Made with")
                            .on_hover_text("The Python that creates it (3.10 or newer)");
                        interpreter_field(ui, &mut self.base, &self.candidates);
                        ui.end_row();
                    }
                    Mode::Install => {
                        ui.label("Into");
                        interpreter_field(ui, &mut self.target, &self.candidates);
                        ui.end_row();
                    }
                }
                ui.label("Packages").on_hover_text(
                    "Names as for pip install, separated by spaces: cadquery, \
                     ocp-vscode>=2, trimesh[easy]…",
                );
                ui.add(
                    egui::TextEdit::singleline(&mut self.packages)
                        .desired_width(380.0)
                        .hint_text("for example: trimesh numpy-stl"),
                );
                ui.end_row();
            });
    }

    fn progress(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        ui.horizontal(|ui| {
            if self.running() {
                ui.spinner();
                ui.label("Working… (downloading CadQuery takes a few minutes)");
                if ui.button("Cancel").clicked()
                    && let Some(job) = &self.job
                {
                    job.cancel();
                }
                return;
            }
            let start = match self.mode {
                Mode::Create => "Create",
                Mode::Install => "Install",
            };
            if ui.button(start).clicked() {
                self.start();
            }
            match self.job.as_ref().map(Job::state) {
                Some(JobState::Succeeded) => match self.mode {
                    Mode::Create => {
                        ui.colored_label(ui.visuals().text_color(), "Done.");
                        let python = venv_python(Path::new(self.folder.trim()));
                        if ui
                            .button(format!("{} Use this environment", icon::CHECK))
                            .on_hover_text(python.display().to_string())
                            .clicked()
                        {
                            actions.push(Action::UsePython(python));
                        }
                    }
                    Mode::Install => {
                        ui.label("Installed; restart Python to import them.");
                        if ui.button("Restart Python").clicked() {
                            actions.push(Action::RestartPython);
                        }
                    }
                },
                Some(JobState::Failed(problem)) => {
                    ui.colored_label(ui.visuals().error_fg_color, problem);
                }
                Some(JobState::Cancelled) => {
                    ui.weak("Cancelled.");
                }
                Some(JobState::Running) | None => {}
            }
        });
        if let Some(problem) = &self.problem {
            ui.colored_label(ui.visuals().error_fg_color, problem);
        }
        if let Some(job) = &self.job {
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for line in job.output() {
                        ui.monospace(line);
                    }
                });
        }
    }
}

/// A path typed or picked among the interpreters found.
fn interpreter_field(ui: &mut egui::Ui, path: &mut String, candidates: &[PathBuf]) {
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(path).desired_width(350.0));
        ui.menu_button(icon::LIST_BULLETS, |ui| {
            if candidates.is_empty() {
                ui.weak("No other Python found");
            }
            for candidate in candidates {
                let text = candidate.display().to_string();
                if ui.button(&text).clicked() {
                    *path = text;
                    ui.close();
                }
            }
        })
        .response
        .on_hover_text("Pythons found on this computer");
    });
}
