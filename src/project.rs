//! The project folder of the Project panel: its file tree, its main script
//! and the file operations the panel offers.
//!
//! Folder listings are cached and read again when the folder's
//! modification time changes, checked at most once a second per shown
//! folder, so drawing the tree every frame touches the disk very little and
//! files added by other programs still appear.

use std::collections::{BTreeSet, HashMap};
use std::io::{self, Read as _};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

/// How often a shown folder is checked for changes made by other programs.
const RECHECK_INTERVAL: Duration = Duration::from_secs(1);
/// Root scripts read when looking for the main script, and how much of each.
const MAX_MAIN_CANDIDATES: usize = 64;
const MAX_MAIN_BYTES: u64 = 256 * 1024;

/// Extensions of the files the editor opens from the tree; other files are
/// listed but left to other programs.
const TEXT_EXTENSIONS: &[&str] = &[
    "py", "pyw", "pyi", "txt", "md", "rst", "json", "toml", "cfg", "ini", "yaml", "yml", "csv",
];

/// A file or folder of the tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
}

/// One line of the tree as shown: an entry, how deep it is and, for a
/// folder, whether its contents are shown below it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub entry: Entry,
    pub depth: usize,
    pub expanded: bool,
}

struct Listing {
    entries: Vec<Entry>,
    modified: Option<SystemTime>,
    checked: Instant,
}

pub struct Project {
    root: PathBuf,
    /// The script F5 runs while a file of the project is shown.
    pub main: Option<PathBuf>,
    /// Current file search query in the Project panel.
    pub filter: String,
    /// Folders whose contents are shown; the root's always are.
    expanded: BTreeSet<PathBuf>,
    listings: HashMap<PathBuf, Listing>,
}

impl Project {
    /// The project in the folder `root`, with nothing below it expanded.
    pub fn open(root: &Path) -> io::Result<Self> {
        let root = std::path::absolute(root)?;
        if !root.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                format!("{} is not a folder", root.display()),
            ));
        }
        Ok(Self {
            root,
            main: None,
            filter: String::new(),
            expanded: BTreeSet::new(),
            listings: HashMap::new(),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The folder's name, for the panel and the menus.
    pub fn name(&self) -> String {
        self.root.file_name().map_or_else(
            || self.root.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        )
    }

    /// Whether `path` is inside the project folder.
    pub fn contains(&self, path: &Path) -> bool {
        path.starts_with(&self.root)
    }

    /// Path of `path` relative to the project folder, for display.
    pub fn relative<'a>(&self, path: &'a Path) -> &'a Path {
        path.strip_prefix(&self.root).unwrap_or(path)
    }

    pub fn is_expanded(&self, folder: &Path) -> bool {
        folder == self.root || self.expanded.contains(folder)
    }

    pub fn toggle(&mut self, folder: &Path) {
        if !self.expanded.remove(folder) {
            self.expanded.insert(folder.to_path_buf());
        }
    }

    pub fn collapse_all(&mut self) {
        self.expanded.clear();
    }

    /// Expands the folders down to `path`, so its row is shown.
    pub fn reveal(&mut self, path: &Path) {
        let Ok(relative) = path.strip_prefix(&self.root) else {
            return;
        };
        let mut folder = self.root.clone();
        let parents = relative
            .parent()
            .map(Path::components)
            .into_iter()
            .flatten();
        for component in parents {
            folder.push(component);
            self.expanded.insert(folder.clone());
        }
    }

    /// The expanded folders, for the next session.
    pub fn expanded(&self) -> Vec<PathBuf> {
        self.expanded.iter().cloned().collect()
    }

    /// Expands folders of an earlier session that still exist.
    pub fn restore_expanded(&mut self, folders: &[PathBuf]) {
        self.expanded = folders
            .iter()
            .filter(|folder| self.contains(folder) && folder.is_dir())
            .cloned()
            .collect();
    }

    /// Reads every listing again on the next [`Project::rows`].
    pub fn refresh(&mut self) {
        self.listings.clear();
    }

    /// Follows a rename or move from `from` to `to` (a file, or a folder and
    /// everything in it).
    pub fn relocate(&mut self, from: &Path, to: &Path) {
        if let Some(main) = self
            .main
            .as_deref()
            .and_then(|main| relocated(main, from, to))
        {
            self.main = Some(main);
        }
        self.expanded = std::mem::take(&mut self.expanded)
            .into_iter()
            .map(|folder| relocated(&folder, from, to).unwrap_or(folder))
            .collect();
        self.refresh();
    }

    /// Forgets `path`, which was deleted, and what was below it.
    pub fn forget(&mut self, path: &Path) {
        if self
            .main
            .as_deref()
            .is_some_and(|main| main.starts_with(path))
        {
            self.main = None;
        }
        self.expanded.retain(|folder| !folder.starts_with(path));
        self.refresh();
    }

    /// The rows of the tree as shown: the contents of the root, and of
    /// every expanded folder below its row. Unreadable folders show empty.
    pub fn rows(&mut self) -> Vec<Row> {
        let mut rows = Vec::new();
        let root = self.root.clone();
        self.push_rows(&root, 0, &mut rows);
        rows
    }

    /// Searches all files in the project (including inside collapsed folders)
    /// whose file name or relative path matches `query` (case-insensitive),
    /// without altering the expanded folder state.
    pub fn search_rows(&mut self, query: &str) -> Vec<Row> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return self.rows();
        }
        let needle = trimmed.to_lowercase();
        let mut rows = Vec::new();
        let root = self.root.clone();
        let mut visited_dirs = 0usize;
        self.push_search_rows(&root, 0, &needle, &mut visited_dirs, &mut rows);
        rows
    }

    fn push_search_rows(
        &mut self,
        folder: &Path,
        depth: usize,
        needle: &str,
        visited_dirs: &mut usize,
        rows: &mut Vec<Row>,
    ) {
        const MAX_SEARCH_DEPTH: usize = 16;
        const MAX_SEARCH_DIRS: usize = 512;
        if depth > MAX_SEARCH_DEPTH || *visited_dirs >= MAX_SEARCH_DIRS {
            return;
        }
        *visited_dirs += 1;
        for entry in self.entries(folder) {
            if entry.is_dir {
                let path = entry.path.clone();
                self.push_search_rows(&path, depth + 1, needle, visited_dirs, rows);
            } else {
                let rel = self
                    .relative(&entry.path)
                    .to_string_lossy()
                    .replace('\\', "/");
                if entry.name.to_lowercase().contains(needle) || rel.to_lowercase().contains(needle)
                {
                    rows.push(Row {
                        entry,
                        depth: 0,
                        expanded: false,
                    });
                }
            }
        }
    }

    fn push_rows(&mut self, folder: &Path, depth: usize, rows: &mut Vec<Row>) {
        for entry in self.entries(folder) {
            let expanded = entry.is_dir && self.expanded.contains(&entry.path);
            let path = entry.path.clone();
            rows.push(Row {
                entry,
                depth,
                expanded,
            });
            if expanded {
                self.push_rows(&path, depth + 1, rows);
            }
        }
    }

    /// The cached entries of `folder`, read again when it changed.
    fn entries(&mut self, folder: &Path) -> Vec<Entry> {
        let now = Instant::now();
        if let Some(listing) = self.listings.get_mut(folder) {
            if now.duration_since(listing.checked) < RECHECK_INTERVAL {
                return listing.entries.clone();
            }
            listing.checked = now;
            let modified = modified(folder);
            if modified.is_some() && modified == listing.modified {
                return listing.entries.clone();
            }
        }
        let entries = list(folder).unwrap_or_else(|error| {
            log::warn!("Cannot list {}: {error}", folder.display());
            Vec::new()
        });
        self.listings.insert(
            folder.to_path_buf(),
            Listing {
                entries: entries.clone(),
                modified: modified(folder),
                checked: now,
            },
        );
        entries
    }
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// The visible entries of `folder`: folders first, then files, each sorted
/// by name regardless of case. Hidden files, caches and compiled files are
/// left out.
pub fn list(folder: &Path) -> io::Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for item in std::fs::read_dir(folder)? {
        let item = item?;
        let name = item.file_name().to_string_lossy().into_owned();
        if is_hidden(&name) {
            continue;
        }
        // Follows links, so a linked folder is shown as a folder.
        let is_dir = item.path().is_dir();
        entries.push(Entry {
            path: item.path(),
            name,
            is_dir,
        });
    }
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(entries)
}

/// Names the tree does not show: hidden files and folders (`.git`,
/// `.venv`), Python's caches and compiled files, and packaging metadata.
pub fn is_hidden(name: &str) -> bool {
    name.starts_with('.')
        || name == "__pycache__"
        || name.ends_with(".pyc")
        || name.ends_with(".pyo")
        || name.ends_with(".egg-info")
}

/// Whether the editor opens `path` from the tree: Python scripts and plain
/// text files.
pub fn is_editable(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            TEXT_EXTENSIONS
                .iter()
                .any(|known| known.eq_ignore_ascii_case(extension))
        })
}

/// Whether `path` is a Python script, which the project can run.
pub fn is_script(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("py"))
}

/// The script that most probably renders the project in `root`: `main.py`,
/// `__main__.py` or a script named after the folder, else the only script
/// at the top of the folder that calls `show_object`. `None` when it is not
/// clear, rather than a guess that runs the wrong file.
pub fn guess_main(root: &Path) -> Option<PathBuf> {
    let folder_script = root
        .file_name()
        .map(|name| format!("{}.py", name.to_string_lossy()));
    let named = ["main.py", "__main__.py"]
        .into_iter()
        .map(str::to_owned)
        .chain(folder_script);
    for name in named {
        let path = root.join(name);
        if path.is_file() {
            return Some(path);
        }
    }
    let mut showing = list(root)
        .ok()?
        .into_iter()
        .filter(|entry| !entry.is_dir && is_script(&entry.path))
        .take(MAX_MAIN_CANDIDATES)
        .filter(|entry| calls_show_object(&entry.path));
    let only = showing.next()?;
    showing.next().is_none().then_some(only.path)
}

fn calls_show_object(path: &Path) -> bool {
    let mut text = String::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(MAX_MAIN_BYTES).read_to_string(&mut text))
        .is_ok()
        && text.contains("show_object(")
}

/// Checks a file or folder name typed by the user. Names that some systems
/// refuse are refused everywhere, so a project can move between them.
pub fn check_name(name: &str) -> Result<&str, String> {
    const RESERVED: &[&str] = &[
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let name = name.trim();
    if name.is_empty() {
        return Err("Enter a name".to_owned());
    }
    if name == "." || name == ".." {
        return Err(format!("\"{name}\" is not a valid name"));
    }
    if let Some(invalid) = name.chars().find(|c| {
        matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control()
    }) {
        return Err(format!("A name cannot contain \"{invalid}\""));
    }
    if name.ends_with('.') {
        return Err("A name cannot end with a dot".to_owned());
    }
    let stem = name.split('.').next().unwrap_or(name);
    if RESERVED
        .iter()
        .any(|reserved| reserved.eq_ignore_ascii_case(stem))
    {
        return Err(format!("\"{stem}\" is a reserved name on Windows"));
    }
    Ok(name)
}

fn invalid_name(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

/// Creates an empty file named `name` in `folder`; a name without an
/// extension becomes a Python script. An existing file is never replaced.
pub fn create_file(folder: &Path, name: &str) -> io::Result<PathBuf> {
    let name = check_name(name).map_err(invalid_name)?;
    let mut path = folder.join(name);
    if path.extension().is_none() {
        path.set_extension("py");
    }
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| describe_existing(error, &path))?;
    Ok(path)
}

/// Creates the folder `name` in `folder`.
pub fn create_folder(folder: &Path, name: &str) -> io::Result<PathBuf> {
    let name = check_name(name).map_err(invalid_name)?;
    let path = folder.join(name);
    std::fs::create_dir(&path).map_err(|error| describe_existing(error, &path))?;
    Ok(path)
}

/// Renames the file or folder at `path` to `name`, in the same folder.
/// Nothing is replaced: renaming onto an existing name fails, except to
/// change the case of the name where the system ignores case, which finds
/// the file itself under the new name. Where case matters, another file
/// may hold that name, and it is kept.
pub fn rename(path: &Path, name: &str) -> io::Result<PathBuf> {
    let name = check_name(name).map_err(invalid_name)?;
    let parent = path
        .parent()
        .ok_or_else(|| invalid_name("The project folder itself cannot be renamed".to_owned()))?;
    let target = parent.join(name);
    if target == path {
        return Ok(target);
    }
    let same_but_case = path
        .file_name()
        .is_some_and(|old| old.to_string_lossy().to_lowercase() == name.to_lowercase());
    let itself = same_but_case && same_file::is_same_file(path, &target).unwrap_or(false);
    if !itself && std::fs::symlink_metadata(&target).is_ok() {
        return Err(describe_existing(
            io::Error::from(io::ErrorKind::AlreadyExists),
            &target,
        ));
    }
    std::fs::rename(path, &target)?;
    Ok(target)
}

/// Deletes the file, or the folder and everything in it, at `path`.
/// Links are removed, never followed.
pub fn delete(path: &Path) -> io::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
}

fn describe_existing(error: io::Error, path: &Path) -> io::Error {
    if error.kind() == io::ErrorKind::AlreadyExists {
        let name = path.file_name().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        io::Error::new(error.kind(), format!("{name} already exists"))
    } else {
        error
    }
}

/// Where `path` is after `from` moved to `to`: `to` itself, or the same
/// place below it. `None` when `path` was not moved.
pub fn relocated(path: &Path, from: &Path, to: &Path) -> Option<PathBuf> {
    let rest = path.strip_prefix(from).ok()?;
    Some(if rest.as_os_str().is_empty() {
        to.to_path_buf()
    } else {
        to.join(rest)
    })
}

#[cfg(test)]
#[path = "../tests/rust/app/test_project.rs"]
mod tests;
