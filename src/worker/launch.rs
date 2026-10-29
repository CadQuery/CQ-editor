//! Finding a Python interpreter that has CadQuery.

use std::path::{Path, PathBuf};

#[cfg(windows)]
const VENV_PYTHON: &[&str] = &["Scripts", "python.exe"];
#[cfg(not(windows))]
const VENV_PYTHON: &[&str] = &["bin", "python3"];

#[cfg(windows)]
const CONDA_PYTHON: &[&str] = &["python.exe"];
#[cfg(not(windows))]
const CONDA_PYTHON: &[&str] = &["bin", "python3"];

#[cfg(windows)]
const PATH_PYTHON: &str = "python";
#[cfg(not(windows))]
const PATH_PYTHON: &str = "python3";

/// Picks the interpreter, in order of preference:
///
/// 1. `explicit` (command line or preferences), when not empty;
/// 2. the `CQ_EDITOR_PYTHON` environment variable;
/// 3. the Python bundled by an installed CQ-editor distribution;
/// 4. the active virtual or conda environment (`VIRTUAL_ENV`, `CONDA_PREFIX`);
/// 5. a `.venv` next to the executable, in the working directory or (for
///    development builds) in the source tree;
/// 6. `python`/`python3` from `PATH`.
pub fn find_python(explicit: Option<&str>) -> PathBuf {
    if let Some(path) = explicit.map(str::trim).filter(|p| !p.is_empty()) {
        return PathBuf::from(path);
    }
    if let Some(path) = std::env::var_os("CQ_EDITOR_PYTHON") {
        return PathBuf::from(path);
    }
    if let Some(python) = std::env::current_exe()
        .ok()
        .as_deref()
        .and_then(bundled_python_for)
    {
        return python;
    }

    let environments = [
        std::env::var_os("VIRTUAL_ENV").map(|p| (PathBuf::from(p), VENV_PYTHON)),
        std::env::var_os("CONDA_PREFIX").map(|p| (PathBuf::from(p), CONDA_PYTHON)),
    ];
    for (root, relative) in environments.into_iter().flatten() {
        if let Some(python) = existing(&root, relative) {
            return python;
        }
    }

    let mut roots = Vec::new();
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
    {
        roots.push(dir);
    }
    if let Ok(dir) = std::env::current_dir() {
        roots.push(dir);
    }
    if cfg!(debug_assertions) {
        roots.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    }
    for root in roots {
        if let Some(python) = existing(&root.join(".venv"), VENV_PYTHON) {
            return python;
        }
    }

    PathBuf::from(PATH_PYTHON)
}

/// The interpreter of the virtual environment in `folder`.
pub fn venv_python(folder: &Path) -> PathBuf {
    VENV_PYTHON
        .iter()
        .fold(folder.to_path_buf(), |path, part| path.join(part))
}

/// Interpreters on this computer, for the user to choose from when the
/// detected one has no CadQuery: the places [`find_python`] looks at, conda
/// and mamba environments in the usual folders, Python installs from
/// python.org (Windows) and `python` on `PATH`. Only existing files, each
/// once; whether they have CadQuery is only known by starting them.
pub fn candidates() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    let mut add = |path: PathBuf| {
        if path.is_file() && !found.iter().any(|other| same_path(other, &path)) {
            found.push(path);
        }
    };

    if let Some(path) = std::env::var_os("CQ_EDITOR_PYTHON") {
        add(PathBuf::from(path));
    }
    if let Ok(executable) = std::env::current_exe()
        && let Some(python) = bundled_python_for(&executable)
    {
        add(python);
    }
    for (variable, relative) in [("VIRTUAL_ENV", VENV_PYTHON), ("CONDA_PREFIX", CONDA_PYTHON)] {
        if let Some(root) = std::env::var_os(variable) {
            add(relative
                .iter()
                .fold(PathBuf::from(root), |p, part| p.join(part)));
        }
    }
    for root in [std::env::current_dir().ok(), std::env::current_exe().ok()]
        .into_iter()
        .flatten()
    {
        for directory in root.ancestors().take(2) {
            add(VENV_PYTHON
                .iter()
                .fold(directory.join(".venv"), |p, part| p.join(part)));
        }
    }

    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" });
    if let Some(home) = home.map(PathBuf::from) {
        for distribution in [
            "miniforge3",
            "mambaforge",
            "miniconda3",
            "anaconda3",
            "micromamba",
        ] {
            environments_in(&home.join(distribution))
                .into_iter()
                .for_each(&mut add);
        }
    }
    if cfg!(windows)
        && let Some(programs) = std::env::var_os("LOCALAPPDATA")
        && let Ok(entries) =
            std::fs::read_dir(PathBuf::from(programs).join("Programs").join("Python"))
    {
        let mut installs: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
        installs.sort();
        // The newest first.
        for install in installs.into_iter().rev() {
            add(install.join("python.exe"));
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&path) {
            // The Microsoft Store placeholder is not an interpreter.
            if directory.ends_with("WindowsApps") {
                continue;
            }
            add(directory.join(if cfg!(windows) {
                "python.exe"
            } else {
                PATH_PYTHON
            }));
        }
    }
    found
}

/// Whether two paths name the same file; Windows paths ignore case, so a
/// working directory with a lowercase drive letter adds nothing new.
fn same_path(a: &Path, b: &Path) -> bool {
    if cfg!(windows) {
        a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
    } else {
        a == b
    }
}

/// The base interpreter of a conda-like distribution at `root` and those of
/// its environments, sorted by name.
fn environments_in(root: &Path) -> Vec<PathBuf> {
    let interpreter = |environment: &Path| existing(environment, CONDA_PYTHON);
    let mut environments: Vec<PathBuf> = std::fs::read_dir(root.join("envs"))
        .map(|entries| entries.flatten().map(|entry| entry.path()).collect())
        .unwrap_or_default();
    environments.sort();
    std::iter::once(root.to_path_buf())
        .chain(environments)
        .filter_map(|environment| interpreter(&environment))
        .collect()
}

fn existing(root: &Path, relative: &[&str]) -> Option<PathBuf> {
    let path = relative
        .iter()
        .fold(root.to_path_buf(), |path, part| path.join(part));
    path.is_file().then_some(path)
}

/// Finds the base Python in a constructor installation. On Windows the GUI
/// and `python.exe` live in the prefix root. On Unix the GUI and Python live
/// in `<prefix>/bin`, so the second candidate is the prefix itself.
fn bundled_python_for(executable: &Path) -> Option<PathBuf> {
    let executable_dir = executable.parent()?;
    [Some(executable_dir), executable_dir.parent()]
        .into_iter()
        .flatten()
        // Do not mistake `/usr/local/bin/python3` for a bundled interpreter
        // when someone copies only the portable executable to `/usr/local/bin`.
        .filter(|root| root.join(".installer.info").is_file())
        .find_map(|root| existing(root, CONDA_PYTHON))
}

#[cfg(test)]
#[path = "../../tests/rust/protocol/test_launch.rs"]
mod tests;
