//! Creating Python environments and installing packages into them, from the
//! preferences (FEAT-E3).
//!
//! A [`Job`] runs a few commands (`python -m venv`, `python -m pip install`)
//! one after the other on a thread of its own, collecting their output line
//! by line for the dialog, and can be cancelled. Commands are started
//! directly, never through a shell, and package names are checked first.

use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

use crate::worker::launch::venv_python;

/// What a new environment gets: CadQuery, and jedi for completion.
pub const DEFAULT_PACKAGES: &str = "cadquery jedi";
/// Output kept for the dialog; older lines are dropped.
const MAX_OUTPUT_LINES: usize = 5_000;

/// One command of a job.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    /// Shown in the output before the command's own lines.
    pub title: String,
    pub program: PathBuf,
    pub args: Vec<OsString>,
}

/// The packages typed by the user, as `pip install` arguments: names with
/// optional extras, versions or markers (`cadquery`, `ocp-vscode>=2`,
/// `trimesh[easy]`), or local wheels. Options are refused, so a typed
/// `--index-url` cannot point pip elsewhere.
pub fn parse_packages(text: &str) -> Result<Vec<String>, String> {
    let packages: Vec<String> = text
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect();
    if packages.is_empty() {
        return Err("Name at least one package".to_owned());
    }
    for package in &packages {
        if package.starts_with('-') {
            return Err(format!(
                "\"{package}\" is an option; only package names are accepted"
            ));
        }
        let allowed = |c: char| c.is_alphanumeric() || "-_.[]<>=!~*:/\\+@".contains(c);
        if let Some(bad) = package.chars().find(|&c| !allowed(c)) {
            return Err(format!("\"{package}\" contains \"{bad}\""));
        }
    }
    Ok(packages)
}

/// Creates a virtual environment in `folder` with `base`, then installs
/// `packages` into it.
pub fn create_steps(base: &Path, folder: &Path, packages: &[String]) -> Vec<Step> {
    let mut steps = vec![Step {
        title: format!("Creating the environment in {}", folder.display()),
        program: base.to_path_buf(),
        args: vec!["-m".into(), "venv".into(), folder.into()],
    }];
    steps.extend(install_steps(&venv_python(folder), packages));
    steps
}

/// Installs `packages` into the environment of `python`.
pub fn install_steps(python: &Path, packages: &[String]) -> Vec<Step> {
    let mut args: Vec<OsString> = ["-m", "pip", "install"].map(OsString::from).to_vec();
    args.extend(packages.iter().map(OsString::from));
    vec![Step {
        title: format!("Installing {}", packages.join(" ")),
        program: python.to_path_buf(),
        args,
    }]
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JobState {
    Running,
    Succeeded,
    Failed(String),
    Cancelled,
}

enum Event {
    Line(String),
    Finished(JobState),
}

/// Commands running one after the other in the background.
pub struct Job {
    output: Vec<String>,
    state: JobState,
    events: mpsc::Receiver<Event>,
    cancelled: Arc<AtomicBool>,
    child: Arc<Mutex<Option<Child>>>,
}

impl Job {
    pub fn start(steps: Vec<Step>) -> Self {
        let (sender, events) = mpsc::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        let child = Arc::new(Mutex::new(None));
        let (flag, slot) = (cancelled.clone(), child.clone());
        thread::Builder::new()
            .name("environment-job".into())
            .spawn(move || {
                let state = run_steps(&steps, &sender, &flag, &slot);
                let _ = sender.send(Event::Finished(state));
            })
            .expect("a thread for the job");
        Self {
            output: Vec::new(),
            state: JobState::Running,
            events,
            cancelled,
            child,
        }
    }

    /// Takes in what the commands printed since the last call.
    pub fn poll(&mut self) -> &JobState {
        while let Ok(event) = self.events.try_recv() {
            match event {
                Event::Line(line) => {
                    self.output.push(line);
                    if self.output.len() > MAX_OUTPUT_LINES {
                        self.output.drain(..self.output.len() - MAX_OUTPUT_LINES);
                    }
                }
                Event::Finished(state) => self.state = state,
            }
        }
        &self.state
    }

    pub fn state(&self) -> &JobState {
        &self.state
    }

    pub fn output(&self) -> &[String] {
        &self.output
    }

    /// Stops the command running now and skips the others.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        if let Some(child) = self.child.lock().expect("child slot").as_mut() {
            let _ = child.kill();
        }
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        if self.state == JobState::Running {
            self.cancel();
        }
    }
}

fn run_steps(
    steps: &[Step],
    sender: &mpsc::Sender<Event>,
    cancelled: &AtomicBool,
    slot: &Mutex<Option<Child>>,
) -> JobState {
    for step in steps {
        if cancelled.load(Ordering::SeqCst) {
            return JobState::Cancelled;
        }
        let _ = sender.send(Event::Line(format!("▶ {}", step.title)));
        let mut command = Command::new(&step.program);
        command
            .args(&step.args)
            .env("PIP_DISABLE_PIP_VERSION_CHECK", "1")
            .env("PYTHONIOENCODING", "utf-8")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt as _;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                return JobState::Failed(format!(
                    "Cannot start {}: {error}",
                    step.program.display()
                ));
            }
        };
        let stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take().expect("piped stderr");
        *slot.lock().expect("child slot") = Some(child);
        // A cancel between the check above and storing the child.
        if cancelled.load(Ordering::SeqCst)
            && let Some(child) = slot.lock().expect("child slot").as_mut()
        {
            let _ = child.kill();
        }
        let errors = {
            let sender = sender.clone();
            thread::spawn(move || forward_lines(stderr, &sender))
        };
        forward_lines(stdout, sender);
        let _ = errors.join();
        let status = slot
            .lock()
            .expect("child slot")
            .take()
            .map(|mut child| child.wait());
        if cancelled.load(Ordering::SeqCst) {
            return JobState::Cancelled;
        }
        match status {
            Some(Ok(status)) if status.success() => {}
            Some(Ok(status)) => {
                let code = status
                    .code()
                    .map_or_else(|| "a signal".to_owned(), |code| format!("code {code}"));
                return JobState::Failed(format!("{} failed (exit {code})", step.title));
            }
            Some(Err(error)) => return JobState::Failed(error.to_string()),
            None => return JobState::Cancelled,
        }
    }
    JobState::Succeeded
}

fn forward_lines(stream: impl Read, sender: &mpsc::Sender<Event>) {
    let mut reader = BufReader::new(stream);
    let mut line = Vec::new();
    while matches!(reader.read_until(b'\n', &mut line), Ok(read) if read > 0) {
        let text = String::from_utf8_lossy(&line);
        let _ = sender.send(Event::Line(text.trim_end().to_owned()));
        line.clear();
    }
}

#[cfg(test)]
#[path = "../tests/rust/app/test_environments.rs"]
mod tests;
