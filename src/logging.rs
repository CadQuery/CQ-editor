//! Application-wide log shown by the Log panel.
//!
//! Records from the `log` crate, the worker's stderr, Python logging and
//! script `print`/`log()` output all land in one bounded buffer. Multi-line
//! messages are split into one entry per line so the panel can use fixed-height
//! rows.
//!
//! Every entry is also appended to a log file in the data folder (see
//! [`init_file`]), together with GUI panics, so the messages explaining a
//! crash survive it. The file is rotated at [`MAX_FILE_BYTES`].

use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufWriter, Write as _};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock, TryLockError};
use std::time::{Duration, Instant};

/// Oldest entries are dropped beyond this many lines.
const MAX_ENTRIES: usize = 10_000;
/// The log file is renamed to [`PREVIOUS_LOG_FILE`] beyond this size.
pub const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
pub const LOG_FILE: &str = "cq-editor.log";
pub const PREVIOUS_LOG_FILE: &str = "cq-editor.1.log";
/// Buffered informational lines reach the file at least this often;
/// warnings and errors are written at once.
const FLUSH_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Error,
    Warn,
    Info,
    Debug,
}

impl Level {
    pub fn label(self) -> &'static str {
        match self {
            Level::Error => "ERROR",
            Level::Warn => "WARN",
            Level::Info => "INFO",
            Level::Debug => "DEBUG",
        }
    }

    /// Maps Python logging level names.
    pub fn from_python(name: &str) -> Self {
        match name {
            "critical" | "error" => Level::Error,
            "warning" => Level::Warn,
            "debug" => Level::Debug,
            _ => Level::Info,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub time: String,
    pub level: Level,
    pub source: String,
    pub text: String,
}

#[derive(Default)]
struct Store {
    entries: VecDeque<Entry>,
    /// Increments on every change so views can cheaply detect updates.
    revision: u64,
    file: Option<LogFile>,
}

/// The rotating log file.
struct LogFile {
    directory: PathBuf,
    writer: BufWriter<File>,
    /// Bytes in the current file.
    size: u64,
    flushed_at: Instant,
}

impl LogFile {
    fn open(directory: &Path) -> std::io::Result<Self> {
        std::fs::create_dir_all(directory)?;
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(directory.join(LOG_FILE))?;
        let size = file.metadata()?.len();
        Ok(Self {
            directory: directory.to_path_buf(),
            writer: BufWriter::new(file),
            size,
            flushed_at: Instant::now(),
        })
    }

    fn write_line(&mut self, line: &str, urgent: bool) {
        if self.size >= MAX_FILE_BYTES
            && let Err(error) = self.rotate()
        {
            eprintln!("Cannot rotate the log file: {error}");
        }
        if self.writer.write_all(line.as_bytes()).is_ok() {
            self.size += line.len() as u64;
        }
        if urgent || self.flushed_at.elapsed() >= FLUSH_INTERVAL {
            self.flush();
        }
    }

    fn flush(&mut self) {
        let _ = self.writer.flush();
        self.flushed_at = Instant::now();
    }

    /// Keeps one previous file, which the current one replaces.
    fn rotate(&mut self) -> std::io::Result<()> {
        self.flush();
        let directory = self.directory.clone();
        // Windows cannot rename a file that is open, so close it first.
        let placeholder = BufWriter::new(tempfile::tempfile()?);
        drop(std::mem::replace(&mut self.writer, placeholder));
        let renamed = std::fs::rename(directory.join(LOG_FILE), directory.join(PREVIOUS_LOG_FILE));
        *self = Self::open(&directory)?;
        renamed
    }
}

fn store() -> &'static Mutex<Store> {
    static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
    STORE.get_or_init(Mutex::default)
}

fn lock() -> MutexGuard<'static, Store> {
    store().lock().unwrap_or_else(|e| e.into_inner())
}

/// Starts writing the log to [`LOG_FILE`] in `directory` and records GUI
/// panics there. Returns the file's path.
pub fn init_file(directory: &Path) -> std::io::Result<PathBuf> {
    let mut file = LogFile::open(directory)?;
    let started = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    file.write_line(
        &format!(
            "--- CQ-editor {} started {started} ---\n",
            env!("CARGO_PKG_VERSION")
        ),
        true,
    );
    lock().file = Some(file);
    // Buffered lines reach the file within FLUSH_INTERVAL even when nothing
    // else is logged after them.
    let flusher = std::thread::Builder::new()
        .name("log-flush".into())
        .spawn(|| {
            loop {
                std::thread::sleep(FLUSH_INTERVAL);
                flush();
            }
        });
    if let Err(error) = flusher {
        eprintln!("Cannot start flushing the log file: {error}");
    }

    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        record_panic(&format!(
            "{info}\n{}",
            std::backtrace::Backtrace::force_capture()
        ));
        previous_hook(info);
    }));
    Ok(directory.join(LOG_FILE))
}

/// Writes buffered lines to the log file.
pub fn flush() {
    if let Some(file) = &mut lock().file {
        file.flush();
    }
}

/// Writes a panic report without waiting for the lock: the panic may have
/// happened while this thread held it.
fn record_panic(report: &str) {
    let mut store = match store().try_lock() {
        Ok(store) => store,
        Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
        Err(TryLockError::WouldBlock) => return,
    };
    if let Some(file) = &mut store.file {
        let time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        for line in report.lines() {
            file.write_line(&format!("{time} ERROR panic: {line}\n"), false);
        }
        file.flush();
    }
}

/// Appends a message, stripping terminal escape sequences.
pub fn push(level: Level, source: &str, message: &str) {
    let now = chrono::Local::now();
    let time = now.format("%H:%M:%S").to_string();
    let message = strip_ansi(message);
    let mut store = lock();
    for line in message.trim_end_matches('\n').split('\n') {
        let text = line.trim_end_matches('\r');
        if let Some(file) = &mut store.file {
            let line = format!(
                "{} {} {source}: {text}\n",
                now.format("%Y-%m-%d %H:%M:%S"),
                level.label()
            );
            file.write_line(&line, level <= Level::Warn);
        }
        store.entries.push_back(Entry {
            time: time.clone(),
            level,
            source: source.to_owned(),
            text: text.to_owned(),
        });
    }
    while store.entries.len() > MAX_ENTRIES {
        store.entries.pop_front();
    }
    store.revision += 1;
}

/// Clears the Log panel; the log file keeps everything.
pub fn clear() {
    let mut store = lock();
    store.entries.clear();
    store.revision += 1;
}

pub fn revision() -> u64 {
    lock().revision
}

/// Calls `f` with the current entries, oldest first.
pub fn with_entries<R>(f: impl FnOnce(&VecDeque<Entry>) -> R) -> R {
    f(&lock().entries)
}

/// Removes ANSI escape sequences such as color codes (`ESC [ ... letter`).
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            // Parameter and intermediate bytes, then one final byte in @..~.
            for c in chars.by_ref() {
                if ('@'..='~').contains(&c) {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

struct Logger;

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        // Our own records down to debug; libraries (wgpu, winit) only when
        // something is wrong.
        metadata.target().starts_with("cq_editor") || metadata.level() <= log::Level::Warn
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let level = match record.level() {
            log::Level::Error => Level::Error,
            log::Level::Warn => Level::Warn,
            log::Level::Info => Level::Info,
            log::Level::Debug | log::Level::Trace => Level::Debug,
        };
        let message = record.args().to_string();
        eprintln!("[{}] {}: {message}", level.label(), record.target());
        // The Vulkan loader reports every stale layer registered on the
        // system (e.g. of uninstalled overlays); that is noise for users.
        let loader_noise = record.target().starts_with("wgpu_hal::vulkan::instance");
        if level != Level::Debug && !loader_noise {
            push(level, "app", &message);
        }
    }

    fn flush(&self) {}
}

pub fn init() {
    static LOGGER: Logger = Logger;
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(log::LevelFilter::Debug);
    }
}

#[cfg(test)]
#[path = "../tests/rust/app/test_logging.rs"]
mod tests;
