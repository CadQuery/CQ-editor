//! Crash recovery and bounded local document history.
//!
//! The unsaved buffers of the open tabs are stored as the active session,
//! separately from history, so startup can distinguish a recoverable
//! interrupted session. Every file is written with the same atomic,
//! synchronized primitive used by normal document saves.
//!
//! History lives in one folder per document, named after a hash of its path
//! (untitled buffers share one), and every version's file name carries its
//! timestamp. A checkpoint therefore lists one small folder and reads no
//! other version. Once per session, versions older than [`MAX_HISTORY_AGE`]
//! (which also removes the history of deleted or renamed files) and the
//! oldest versions beyond [`MAX_HISTORY_BYTES`] are pruned.

use std::collections::HashSet;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::document::{Document, LineEnding, atomic_create, atomic_write};

const SCHEMA_VERSION: u32 = 1;
/// The session file holds every unsaved document since multiple tabs; the
/// single snapshot of earlier releases (schema 1) is still read.
const SESSION_SCHEMA_VERSION: u32 = 2;
pub const CHECKPOINT_INTERVAL: Duration = Duration::from_secs(5);
pub const MAX_HISTORY_PER_DOCUMENT: usize = 20;
/// Versions older than this are removed, whatever document they belong to.
pub const MAX_HISTORY_AGE: Duration = Duration::from_secs(90 * 24 * 60 * 60);
/// Size budget of the whole history; the oldest versions go first beyond it.
pub const MAX_HISTORY_BYTES: u64 = 64 * 1024 * 1024;

const UNTITLED_FOLDER: &str = "untitled";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecoverySnapshot {
    schema: u32,
    pub timestamp_ms: u64,
    pub path: Option<PathBuf>,
    pub text: String,
    pub line_ending: LineEnding,
    pub utf8_bom: bool,
}

impl RecoverySnapshot {
    fn capture(document: &Document, timestamp_ms: u64) -> Self {
        Self {
            schema: SCHEMA_VERSION,
            timestamp_ms,
            path: document.path().map(Path::to_path_buf),
            text: document.text.clone(),
            line_ending: document.line_ending(),
            utf8_bom: document.has_utf8_bom(),
        }
    }

    pub fn display_name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|path| path.file_name())
            .map_or_else(
                || "untitled".to_owned(),
                |name| name.to_string_lossy().into_owned(),
            )
    }

    pub fn into_document(self) -> Document {
        Document::recovered(self.text, self.path, self.line_ending, self.utf8_bom)
    }

    fn validate(self) -> io::Result<Self> {
        if self.schema != SCHEMA_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported recovery format version {}", self.schema),
            ));
        }
        Ok(self)
    }
}

/// The active session: the unsaved documents, in tab order.
#[derive(Serialize, Deserialize)]
struct SessionFile {
    schema: u32,
    documents: Vec<RecoverySnapshot>,
}

/// A history version on disk, known from its file name alone.
struct VersionFile {
    path: PathBuf,
    timestamp_ms: u64,
}

pub struct RecoveryStore {
    root: PathBuf,
    /// Content of the session last written, to skip identical ones.
    last_session: Option<u64>,
    /// Content of each document of that session, so only the documents
    /// that changed since get a history version.
    last_contents: HashSet<u64>,
    /// Documents whose saved baseline was archived since the session began.
    baselines: HashSet<Option<PathBuf>>,
    last_timestamp_ms: u64,
    session_active: bool,
    /// Whether this session already migrated and pruned the whole history.
    maintained: bool,
    max_history_bytes: u64,
}

impl RecoveryStore {
    pub fn new(root: PathBuf) -> Self {
        let session_active = root.join("active-session.json").is_file();
        Self {
            root,
            last_session: None,
            last_contents: HashSet::new(),
            baselines: HashSet::new(),
            last_timestamp_ms: 0,
            session_active,
            maintained: false,
            max_history_bytes: MAX_HISTORY_BYTES,
        }
    }

    fn session_path(&self) -> PathBuf {
        self.root.join("active-session.json")
    }

    fn history_dir(&self) -> PathBuf {
        self.root.join("history")
    }

    /// The folder holding the history of the document at `path`.
    fn document_dir(&self, path: Option<&Path>) -> PathBuf {
        self.history_dir().join(document_folder(path))
    }

    /// The unsaved documents of an interrupted session; empty when there
    /// is none.
    pub fn load_session(&self) -> io::Result<Vec<RecoverySnapshot>> {
        let path = self.session_path();
        match std::fs::read(&path) {
            Ok(bytes) => decode_session(&bytes),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => Err(error),
        }
    }

    /// Persists a modified buffer as the active session and as a history
    /// version; see [`RecoveryStore::checkpoint_all`].
    #[cfg(test)]
    pub fn checkpoint(&mut self, document: &Document) -> io::Result<bool> {
        self.checkpoint_all(&[document])
    }

    /// Persists the modified documents among `documents` as the active
    /// session, and each one that changed since the last checkpoint as a
    /// history version. Repeated identical contents write nothing, and
    /// without modified documents the session is cleared. Returns whether
    /// a session was written.
    pub fn checkpoint_all(&mut self, documents: &[&Document]) -> io::Result<bool> {
        let modified: Vec<&Document> = documents
            .iter()
            .copied()
            .filter(|document| document.is_modified())
            .collect();
        if modified.is_empty() {
            self.clear_session()?;
            return Ok(false);
        }
        self.maintain();

        // The saved version of each file first, so it is older than its
        // edits in the history.
        for document in &modified {
            let path = document.path().map(Path::to_path_buf);
            if self.baselines.insert(path.clone())
                && let Some(path) = path
                && let Err(error) = self.archive_path(&path)
            {
                log::warn!(
                    "Cannot archive the saved baseline {}: {error}",
                    path.display()
                );
            }
        }

        let timestamp = self.next_timestamp();
        let snapshots: Vec<RecoverySnapshot> = modified
            .iter()
            .map(|document| RecoverySnapshot::capture(document, timestamp))
            .collect();
        let contents: Vec<u64> = snapshots.iter().map(content_key).collect();
        let session = {
            let mut hasher = DefaultHasher::new();
            contents.hash(&mut hasher);
            hasher.finish()
        };
        if self.last_session == Some(session) {
            return Ok(false);
        }

        for (snapshot, content) in snapshots.iter().zip(&contents) {
            if self.last_contents.contains(content) {
                continue;
            }
            let encoded = serde_json::to_vec(snapshot).map_err(io::Error::other)?;
            self.append_history(snapshot, &encoded)?;
        }

        let encoded = serde_json::to_vec(&SessionFile {
            schema: SESSION_SCHEMA_VERSION,
            documents: snapshots,
        })
        .map_err(io::Error::other)?;
        std::fs::create_dir_all(&self.root)?;
        atomic_write(&self.session_path(), &encoded)?;
        self.session_active = true;
        self.last_session = Some(session);
        self.last_contents = contents.into_iter().collect();
        self.last_timestamp_ms = timestamp;
        Ok(true)
    }

    /// Adds the current document state to history without marking it as an
    /// interrupted session. Used to retain saved baselines and Save As targets.
    pub fn archive(&mut self, document: &Document) -> io::Result<bool> {
        self.maintain();
        let mut snapshot = RecoverySnapshot::capture(document, 0);
        if self
            .latest(snapshot.path.as_deref())?
            .is_some_and(|latest| content_key(&latest) == content_key(&snapshot))
        {
            return Ok(false);
        }
        snapshot.timestamp_ms = self.next_timestamp();
        let encoded = serde_json::to_vec(&snapshot).map_err(io::Error::other)?;
        self.append_history(&snapshot, &encoded)?;
        self.last_timestamp_ms = snapshot.timestamp_ms;
        Ok(true)
    }

    pub fn archive_path(&mut self, path: &Path) -> io::Result<bool> {
        match Document::open_any(path) {
            Ok(document) => self.archive(&document),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }

    pub fn clear_session(&mut self) -> io::Result<()> {
        if self.session_active {
            match std::fs::remove_file(self.session_path()) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            self.session_active = false;
        }
        self.last_session = None;
        self.last_contents.clear();
        self.baselines.clear();
        Ok(())
    }

    /// The versions of the document at `path`, newest first.
    pub fn history(&mut self, path: Option<&Path>) -> io::Result<Vec<RecoverySnapshot>> {
        self.maintain();
        let mut versions = Vec::new();
        for file in version_files(&self.document_dir(path))? {
            if let Some(snapshot) = read_version(&file.path)
                && snapshot.path.as_deref() == path
            {
                versions.push(snapshot);
            }
        }
        versions.sort_by_key(|snapshot| std::cmp::Reverse(snapshot.timestamp_ms));
        Ok(versions)
    }

    /// The newest version of the document at `path`.
    fn latest(&self, path: Option<&Path>) -> io::Result<Option<RecoverySnapshot>> {
        let mut files = version_files(&self.document_dir(path))?;
        files.sort_by_key(|file| std::cmp::Reverse(file.timestamp_ms));
        // Two paths could share a folder; skip the other one's versions.
        Ok(files
            .iter()
            .filter_map(|file| read_version(&file.path))
            .find(|snapshot| snapshot.path.as_deref() == path))
    }

    fn append_history(&self, snapshot: &RecoverySnapshot, encoded: &[u8]) -> io::Result<()> {
        let directory = self.document_dir(snapshot.path.as_deref());
        std::fs::create_dir_all(&directory)?;
        for suffix in 0..1000 {
            let path = directory.join(version_name(snapshot.timestamp_ms, suffix));
            match atomic_create(&path, encoded) {
                Ok(()) => return prune_document(&directory),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate a unique history version name",
        ))
    }

    /// Migrates the flat history of earlier releases into document folders,
    /// then prunes by age and size. Problems are logged, never fatal: the
    /// history is a convenience and must not block checkpoints.
    fn maintain(&mut self) {
        if std::mem::replace(&mut self.maintained, true) {
            return;
        }
        if let Err(error) = self.migrate_flat_history() {
            log::warn!("Cannot migrate the local history: {error}");
        }
        if let Err(error) = self.prune_all(timestamp_ms()) {
            log::warn!("Cannot prune the local history: {error}");
        }
    }

    fn migrate_flat_history(&self) -> io::Result<()> {
        for file in version_files(&self.history_dir())? {
            match std::fs::read(&file.path).and_then(|bytes| decode(&bytes)) {
                Ok(snapshot) => {
                    let directory = self.document_dir(snapshot.path.as_deref());
                    std::fs::create_dir_all(&directory)?;
                    let name = file.path.file_name().expect("a version file has a name");
                    std::fs::rename(&file.path, directory.join(name))?;
                }
                Err(error) => {
                    log::warn!(
                        "Cannot read recovery history {}: {error}",
                        file.path.display()
                    );
                }
            }
        }
        Ok(())
    }

    /// Removes versions older than [`MAX_HISTORY_AGE`], then the oldest
    /// versions until the history fits [`MAX_HISTORY_BYTES`], then empty
    /// folders. Recovery backups made before reloads expire by age too.
    fn prune_all(&self, now_ms: u64) -> io::Result<()> {
        let cutoff = now_ms.saturating_sub(MAX_HISTORY_AGE.as_millis() as u64);
        let mut kept = Vec::new();
        for directory in subdirectories(&self.history_dir())? {
            for file in version_files(&directory)? {
                if file.timestamp_ms < cutoff {
                    remove_file(&file.path)?;
                } else {
                    let size = std::fs::metadata(&file.path).map_or(0, |m| m.len());
                    kept.push((file, size));
                }
            }
        }

        let mut total: u64 = kept.iter().map(|(_, size)| size).sum();
        kept.sort_by_key(|(file, _)| file.timestamp_ms);
        for (file, size) in kept {
            if total <= self.max_history_bytes {
                break;
            }
            remove_file(&file.path)?;
            total -= size;
        }

        for directory in subdirectories(&self.history_dir())? {
            // Fails harmlessly while the folder still has versions.
            let _ = std::fs::remove_dir(directory);
        }
        self.prune_recovery_backups(cutoff)
    }

    fn prune_recovery_backups(&self, cutoff_ms: u64) -> io::Result<()> {
        let entries = match std::fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };
        let cutoff = UNIX_EPOCH + Duration::from_millis(cutoff_ms);
        for entry in entries {
            let entry = entry?;
            let is_backup = entry
                .file_name()
                .to_string_lossy()
                .ends_with(".recovery.py");
            let expired = entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .is_ok_and(|modified| modified < cutoff);
            if is_backup && expired {
                remove_file(&entry.path())?;
            }
        }
        Ok(())
    }

    fn next_timestamp(&self) -> u64 {
        timestamp_ms().max(self.last_timestamp_ms.saturating_add(1))
    }
}

/// Keeps the newest [`MAX_HISTORY_PER_DOCUMENT`] versions of one folder.
fn prune_document(directory: &Path) -> io::Result<()> {
    let mut files = version_files(directory)?;
    files.sort_by_key(|file| std::cmp::Reverse(file.timestamp_ms));
    for file in files.into_iter().skip(MAX_HISTORY_PER_DOCUMENT) {
        remove_file(&file.path)?;
    }
    Ok(())
}

/// Folder name of a document's history: a hash of its path that does not
/// change between releases (unlike `DefaultHasher`, whose algorithm may).
///
/// The path is rebuilt from its components first, so the same file gets
/// one folder however its path is written: on Windows `/` and `\` both
/// separate, and are hashed as `\`, which the paths of the system's file
/// dialogs, and so the folders of earlier releases, already use.
fn document_folder(path: Option<&Path>) -> String {
    let Some(path) = path else {
        return UNTITLED_FOLDER.to_owned();
    };
    let path: PathBuf = path.components().collect();
    // FNV-1a, 64 bits.
    let hash = path
        .as_os_str()
        .as_encoded_bytes()
        .iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, &byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
        });
    format!("doc-{hash:016x}")
}

fn version_name(timestamp_ms: u64, suffix: usize) -> String {
    format!("version-{timestamp_ms}-{suffix}.json")
}

/// Parses the timestamp of a [`version_name`].
fn version_timestamp(name: &str) -> Option<u64> {
    let rest = name.strip_prefix("version-")?.strip_suffix(".json")?;
    rest.split_once('-')?.0.parse().ok()
}

/// The version files directly inside `directory`.
fn version_files(directory: &Path) -> io::Result<Vec<VersionFile>> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry?;
        if let Some(timestamp_ms) = version_timestamp(&entry.file_name().to_string_lossy())
            && entry.file_type()?.is_file()
        {
            files.push(VersionFile {
                path: entry.path(),
                timestamp_ms,
            });
        }
    }
    Ok(files)
}

fn subdirectories(directory: &Path) -> io::Result<Vec<PathBuf>> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut directories = Vec::new();
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            directories.push(entry.path());
        }
    }
    Ok(directories)
}

fn read_version(path: &Path) -> Option<RecoverySnapshot> {
    match std::fs::read(path).and_then(|bytes| decode(&bytes)) {
        Ok(snapshot) => Some(snapshot),
        Err(error) => {
            log::warn!("Cannot read recovery history {}: {error}", path.display());
            None
        }
    }
}

fn remove_file(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// The documents of a session file, of this release or a single snapshot
/// of an earlier one.
fn decode_session(bytes: &[u8]) -> io::Result<Vec<RecoverySnapshot>> {
    match serde_json::from_slice::<SessionFile>(bytes) {
        Ok(session) if session.schema == SESSION_SCHEMA_VERSION => session
            .documents
            .into_iter()
            .map(RecoverySnapshot::validate)
            .collect(),
        Ok(session) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported recovery session version {}", session.schema),
        )),
        Err(_) => decode(bytes).map(|snapshot| vec![snapshot]),
    }
}

fn decode(bytes: &[u8]) -> io::Result<RecoverySnapshot> {
    serde_json::from_slice::<RecoverySnapshot>(bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
        .validate()
}

fn content_key(snapshot: &RecoverySnapshot) -> u64 {
    let mut hasher = DefaultHasher::new();
    snapshot.path.hash(&mut hasher);
    snapshot.text.hash(&mut hasher);
    snapshot.line_ending.as_str().hash(&mut hasher);
    snapshot.utf8_bom.hash(&mut hasher);
    hasher.finish()
}

fn timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "../tests/rust/editor/test_recovery.rs"]
mod tests;
