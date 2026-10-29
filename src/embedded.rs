//! Files compiled into the binary by `build.rs`: the Python worker package
//! and the example scripts.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};

include!(concat!(env!("OUT_DIR"), "/embedded.rs"));

/// An example script shown in the File > Examples menu.
pub struct Example {
    pub title: String,
    pub source: &'static str,
}

/// The bundled examples, titled after their file names:
/// `03_extruded_profile.py` becomes "Extruded Profile".
pub fn examples() -> Vec<Example> {
    EXAMPLES
        .iter()
        .map(|(file_name, source)| Example {
            title: example_title(file_name),
            source,
        })
        .collect()
}

fn example_title(file_name: &str) -> String {
    let stem = file_name.strip_suffix(".py").unwrap_or(file_name);
    // Drop the "NN_" ordering prefix.
    let stem = match stem.split_once('_') {
        Some((number, rest)) if number.chars().all(|c| c.is_ascii_digit()) => rest,
        _ => stem,
    };
    stem.split('_')
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Directory that contains the `cq_worker` package, ready to be put on
/// `PYTHONPATH`.
///
/// `CQ_EDITOR_WORKER_DIR` overrides it, and debug builds use the source tree
/// directly so worker edits apply without rebuilding. Otherwise the embedded
/// copy is extracted into `cache_dir`, in a folder named after its content
/// hash so that different versions never mix.
pub fn worker_dir(cache_dir: &Path) -> io::Result<PathBuf> {
    if let Some(dir) = std::env::var_os("CQ_EDITOR_WORKER_DIR") {
        return Ok(PathBuf::from(dir));
    }

    let source_tree = Path::new(env!("CARGO_MANIFEST_DIR")).join("worker");
    if cfg!(debug_assertions) && source_tree.join("cq_worker").is_dir() {
        return Ok(source_tree);
    }
    extract(cache_dir, WORKER_FILES)
}

/// Writes `files` into a `worker-<hash>` folder of `cache_dir`, unless an
/// earlier start already did, and then removes the folders of other worker
/// versions.
fn extract(cache_dir: &Path, files: &[(&str, &str)]) -> io::Result<PathBuf> {
    let mut hasher = DefaultHasher::new();
    files.hash(&mut hasher);
    let dir = cache_dir.join(format!("worker-{:016x}", hasher.finish()));
    let marker = dir.join(".complete");
    if !marker.exists() {
        for (relative, contents) in files {
            let path = dir.join(relative);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, contents)?;
        }
        std::fs::write(marker, b"")?;
        remove_other_versions(cache_dir, &dir);
    }
    Ok(dir)
}

/// Every version used to leave its folder behind. A still running instance
/// of another version loses nothing: its worker has imported all its
/// modules, and a restart extracts them again.
fn remove_other_versions(cache_dir: &Path, keep: &Path) {
    let Ok(entries) = std::fs::read_dir(cache_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_worker = entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with("worker-"));
        if is_worker
            && path != keep
            && entry.file_type().is_ok_and(|kind| kind.is_dir())
            && let Err(error) = std::fs::remove_dir_all(&path)
        {
            log::warn!("Cannot remove the old worker {}: {error}", path.display());
        }
    }
}

#[cfg(test)]
#[path = "../tests/rust/app/test_embedded.rs"]
mod tests;
