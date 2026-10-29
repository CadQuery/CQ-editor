//! Detects changes to the open script (and optionally its local imports)
//! for "Automatic reload and preview".
//!
//! Polling modification times is simple, dependency free and copes with
//! editors that save by deleting and recreating the file (vim, neovim):
//! a missing file is simply waited for.

use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

const POLL_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Clone, Copy, PartialEq, Eq)]
struct Stamp {
    modified: Option<SystemTime>,
    len: u64,
}

fn stamp(path: &PathBuf) -> Option<Stamp> {
    let metadata = std::fs::metadata(path).ok()?;
    Some(Stamp {
        modified: metadata.modified().ok(),
        len: metadata.len(),
    })
}

#[derive(Default)]
pub struct FileWatcher {
    files: Vec<(PathBuf, Option<Stamp>)>,
    last_poll: Option<Instant>,
    /// When a change was first seen; reported once the settle delay passed.
    changed_at: Option<Instant>,
    pending: Vec<PathBuf>,
}

impl FileWatcher {
    pub fn is_watching(&self) -> bool {
        !self.files.is_empty()
    }

    #[cfg(test)]
    pub fn watched(&self) -> impl Iterator<Item = &PathBuf> {
        self.files.iter().map(|(path, _)| path)
    }

    /// Replaces the watched files, keeping known stamps.
    pub fn set_files(&mut self, paths: Vec<PathBuf>) {
        let previous = std::mem::take(&mut self.files);
        self.files = paths
            .into_iter()
            .map(|path| {
                let known = previous
                    .iter()
                    .find(|(p, _)| *p == path)
                    .and_then(|(_, s)| *s);
                let current = known.or_else(|| stamp(&path));
                (path, current)
            })
            .collect();
        self.pending
            .retain(|changed| self.files.iter().any(|(path, _)| path == changed));
        if self.pending.is_empty() {
            self.changed_at = None;
        }
    }

    pub fn clear(&mut self) {
        self.files.clear();
        self.changed_at = None;
        self.pending.clear();
    }

    /// Returns the paths whose changes have settled for `delay` (so the
    /// writer has finished). Call every frame; ask for a repaint after
    /// [`FileWatcher::next_poll_in`] to keep polling while idle.
    pub fn poll(&mut self, delay: Duration) -> Vec<PathBuf> {
        let now = Instant::now();
        if self.files.is_empty() {
            return Vec::new();
        }
        if self
            .last_poll
            .is_none_or(|last| now - last >= POLL_INTERVAL)
        {
            self.last_poll = Some(now);
            for (path, known) in &mut self.files {
                // A file that vanished is probably being replaced.
                if let Some(current) = stamp(path)
                    && *known != Some(current)
                {
                    *known = Some(current);
                    if !self.pending.contains(path) {
                        self.pending.push(path.clone());
                    }
                    self.changed_at.get_or_insert(now);
                }
            }
        }
        match self.changed_at {
            Some(at) if now - at >= delay => {
                self.changed_at = None;
                std::mem::take(&mut self.pending)
            }
            _ => Vec::new(),
        }
    }

    pub fn next_poll_in(&self) -> Duration {
        POLL_INTERVAL
    }
}

#[cfg(test)]
#[path = "../tests/rust/app/test_file_watcher.rs"]
mod tests;
