//! The Python worker process that runs CadQuery.
//!
//! The worker is started as `python -m cq_worker` and speaks the line-based
//! JSON protocol of [`protocol`]. Three helper threads keep the UI thread
//! non-blocking: one writes requests, one parses messages from stdout (mesh
//! decoding included) and one forwards stderr lines to the log.
//!
//! Stopping a runaway script is done by killing the process and starting a
//! fresh one, which is also how the GUI recovers from a crashed worker.

pub mod launch;
pub mod protocol;

use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Instant, SystemTime};

use protocol::{Message, ReadyInfo, Request, RequestId};

/// Text, completion and other control messages are expected to stay small.
const MAX_CONTROL_MESSAGE_BYTES: usize = 2 * 1024 * 1024;
/// A rich result may contain one base64-encoded 4 MiB image.
const MAX_DISPLAY_MESSAGE_BYTES: usize = 8 * 1024 * 1024;
/// Mesh-bearing replies need a separate, deliberately larger wire ceiling.
const MAX_MESH_MESSAGE_BYTES: usize = 512 * 1024 * 1024;
/// Native libraries can write directly to stderr, so bound those lines too.
const MAX_STDERR_LINE_BYTES: usize = 256 * 1024;

/// How to start the worker.
#[derive(Clone, Debug, PartialEq)]
pub struct LaunchConfig {
    pub python: PathBuf,
    /// Directory containing the `cq_worker` package.
    pub worker_dir: PathBuf,
}

#[derive(Clone, Debug)]
pub enum Status {
    NotStarted,
    /// Process started, CadQuery still importing.
    Starting,
    Ready(ReadyInfo),
    /// The worker could not start or exited; holds a user-facing reason.
    Failed(String),
}

/// Why the worker is not available, so the first-run dialog can say what
/// to do about it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    /// The interpreter could not be started at all.
    NoPython,
    /// Python started but could not import CadQuery.
    NoCadQuery,
    /// The worker exited before it was ready.
    Exited,
}

/// Windows' `python.exe` placeholder (an App Execution Alias) only offers
/// to install Python from the Microsoft Store and exits with this code.
const STORE_PLACEHOLDER_EXIT_CODE: i32 = 9009;

/// What an exit before the worker was ready means.
pub fn exit_problem(code: Option<i32>) -> Problem {
    if cfg!(windows) && code == Some(STORE_PLACEHOLDER_EXIT_CODE) {
        Problem::NoPython
    } else {
        Problem::Exited
    }
}

/// Something that happened in the worker process.
#[derive(Debug)]
pub enum Event {
    Message(Message),
    /// A line the worker printed on stderr (native library warnings, crashes).
    Stderr(String),
    /// The worker sent a message that cannot be read, larger than its
    /// limit. Nothing it sends afterwards is read, so it must be
    /// restarted; this is not a failed start.
    ProtocolError(String),
    /// The process ended (or its stdout closed).
    Exited,
}

pub struct Worker {
    ctx: egui::Context,
    config: Option<LaunchConfig>,
    process: Option<Process>,
    status: Status,
    /// Set together with [`Status::Failed`].
    problem: Option<Problem>,
    next_id: RequestId,
    /// Distinguishes events of the current process from those of killed ones.
    generation: u64,
    events_tx: Sender<(u64, Event)>,
    events_rx: Receiver<(u64, Event)>,
}

struct Process {
    child: Child,
    requests: Sender<String>,
}

impl Worker {
    pub fn new(ctx: egui::Context) -> Self {
        let (events_tx, events_rx) = mpsc::channel();
        Self {
            ctx,
            config: None,
            process: None,
            status: Status::NotStarted,
            problem: None,
            next_id: 1,
            generation: 0,
            events_tx,
            events_rx,
        }
    }

    pub fn status(&self) -> &Status {
        &self.status
    }

    pub fn info(&self) -> Option<&ReadyInfo> {
        match &self.status {
            Status::Ready(info) => Some(info),
            _ => None,
        }
    }

    pub fn is_ready(&self) -> bool {
        matches!(self.status, Status::Ready(_))
    }

    /// Why the worker failed, while it is [`Status::Failed`].
    pub fn problem(&self) -> Option<Problem> {
        self.problem
            .filter(|_| matches!(self.status, Status::Failed(_)))
    }

    pub fn config(&self) -> Option<&LaunchConfig> {
        self.config.as_ref()
    }

    /// (Re)starts the worker with `config`.
    pub fn start(&mut self, config: LaunchConfig) {
        self.kill();
        self.generation += 1;
        self.config = Some(config.clone());
        match self.spawn(&config) {
            Ok(process) => {
                log::info!(
                    "Starting the CadQuery worker with {}",
                    config.python.display()
                );
                self.process = Some(process);
                self.status = Status::Starting;
            }
            Err(err) => {
                let reason = format!(
                    "Could not start Python ({}): {err}",
                    config.python.display()
                );
                log::error!("{reason}");
                self.status = Status::Failed(reason);
                self.problem = Some(Problem::NoPython);
            }
        }
    }

    pub fn next_id(&mut self) -> RequestId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Queues `request` for the worker. Returns `false` when no worker runs.
    pub fn send(&mut self, request: &Request) -> bool {
        let Some(process) = &self.process else {
            return false;
        };
        let line = match serde_json::to_string(request) {
            Ok(line) => line,
            Err(err) => {
                log::error!("Cannot encode worker request: {err}");
                return false;
            }
        };
        process.requests.send(line).is_ok()
    }

    /// Drains pending events of the current process, updating the status.
    pub fn poll(&mut self) -> Vec<Event> {
        let mut events = Vec::new();
        while let Ok((generation, event)) = self.events_rx.try_recv() {
            if generation != self.generation {
                continue; // from a process we already killed
            }
            match &event {
                Event::Message(Message::Ready(info)) => {
                    if info.protocol != protocol::PROTOCOL_VERSION {
                        log::warn!(
                            "Worker protocol {} differs from the GUI's {}",
                            info.protocol,
                            protocol::PROTOCOL_VERSION
                        );
                    }
                    self.status = Status::Ready(info.clone());
                }
                Event::Message(Message::Fatal { message, .. }) => {
                    self.status = Status::Failed(message.clone());
                    self.problem = Some(Problem::NoCadQuery);
                }
                Event::Exited => {
                    let code = self
                        .process
                        .take()
                        .and_then(|mut p| p.child.wait().ok())
                        .and_then(|status| status.code());
                    if !matches!(self.status, Status::Failed(_)) {
                        self.problem = Some(exit_problem(code));
                        self.status = Status::Failed(match code {
                            Some(code) => format!("The Python worker exited (code {code})"),
                            None => "The Python worker stopped unexpectedly".to_owned(),
                        });
                    }
                }
                _ => {}
            }
            events.push(event);
        }
        events
    }

    /// Asks the worker to exit, giving it a moment to clean up before it is
    /// killed.
    pub fn shutdown(&mut self) {
        if let Some(process) = &mut self.process {
            let _ = process
                .requests
                .send(serde_json::to_string(&Request::Shutdown).unwrap_or_default());
            let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
            while std::time::Instant::now() < deadline {
                if matches!(process.child.try_wait(), Ok(Some(_))) {
                    break;
                }
                thread::sleep(std::time::Duration::from_millis(20));
            }
        }
        self.kill();
    }

    fn kill(&mut self) {
        if let Some(mut process) = self.process.take() {
            let _ = process.child.kill();
            let _ = process.child.wait();
        }
        self.status = Status::NotStarted;
    }

    fn spawn(&self, config: &LaunchConfig) -> std::io::Result<Process> {
        let mut command = Command::new(&config.python);
        command
            .args(["-u", "-m", "cq_worker"])
            .env("PYTHONPATH", python_path(&config.worker_dir))
            .env("PYTHONIOENCODING", "utf-8")
            // Namespaces the worker's scene keys: objects of a killed worker
            // stay on screen, and a new worker must not reuse their keys.
            .env("CQ_EDITOR_SESSION", self.generation.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt as _;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = command.spawn()?;
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take().expect("piped stderr");

        let (requests, pending) = mpsc::channel::<String>();
        thread::Builder::new()
            .name("worker-writer".into())
            .spawn(move || write_requests(stdin, pending))?;

        let (tx, ctx, generation) = (self.events_tx.clone(), self.ctx.clone(), self.generation);
        thread::Builder::new()
            .name("worker-reader".into())
            .spawn(move || {
                let mut reader = BufReader::new(stdout);
                let mut protocol_failed = false;
                loop {
                    match read_limited_line(&mut reader, MAX_MESH_MESSAGE_BYTES) {
                        Ok(LimitedLine::Line(line)) => {
                            if protocol_failed {
                                continue;
                            }
                            if line.iter().all(u8::is_ascii_whitespace) {
                                continue;
                            }
                            let limit = worker_message_limit(&line);
                            if line.len() > limit {
                                let _ = tx.send((
                                    generation,
                                    Event::ProtocolError(oversized_message(line.len(), limit)),
                                ));
                                ctx.request_repaint();
                                protocol_failed = true;
                                continue;
                            }
                            let received_at = SystemTime::now();
                            let started = Instant::now();
                            match serde_json::from_slice::<Message>(&line) {
                                Ok(mut message) => {
                                    if let Message::RunFinished(result) = &mut message {
                                        result.receipt = protocol::Receipt {
                                            received_at: Some(received_at),
                                            parse: started.elapsed(),
                                        };
                                    }
                                    let _ = tx.send((generation, Event::Message(message)));
                                }
                                Err(err) => log::warn!(
                                    "Unreadable worker message ({err}): {}",
                                    String::from_utf8_lossy(&line[..line.len().min(200)])
                                ),
                            }
                            ctx.request_repaint();
                        }
                        Ok(LimitedLine::TooLong(_)) => {
                            if !protocol_failed {
                                let _ = tx.send((
                                    generation,
                                    Event::ProtocolError(oversized_message(
                                        MAX_MESH_MESSAGE_BYTES + 1,
                                        MAX_MESH_MESSAGE_BYTES,
                                    )),
                                ));
                                ctx.request_repaint();
                                protocol_failed = true;
                            }
                        }
                        Ok(LimitedLine::Eof) | Err(_) => break,
                    }
                }
                let _ = tx.send((generation, Event::Exited));
                ctx.request_repaint();
            })?;

        let (tx, ctx) = (self.events_tx.clone(), self.ctx.clone());
        thread::Builder::new()
            .name("worker-stderr".into())
            .spawn(move || {
                let mut reader = BufReader::new(stderr);
                loop {
                    let line = match read_limited_line(&mut reader, MAX_STDERR_LINE_BYTES) {
                        Ok(LimitedLine::Line(line)) => String::from_utf8_lossy(&line).into_owned(),
                        Ok(LimitedLine::TooLong(prefix)) => format!(
                            "{}\n[stderr line truncated after {MAX_STDERR_LINE_BYTES} bytes]",
                            String::from_utf8_lossy(&prefix)
                        ),
                        Ok(LimitedLine::Eof) | Err(_) => break,
                    };
                    let _ = tx.send((generation, Event::Stderr(line)));
                    ctx.request_repaint();
                }
            })?;

        Ok(Process { child, requests })
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn write_requests(mut stdin: ChildStdin, pending: Receiver<String>) {
    for line in pending {
        if stdin
            .write_all(line.as_bytes())
            .and_then(|()| stdin.write_all(b"\n"))
            .and_then(|()| stdin.flush())
            .is_err()
        {
            break; // the process is gone; the reader reports it
        }
    }
}

/// `worker_dir` in front of any `PYTHONPATH` the user already has.
fn python_path(worker_dir: &std::path::Path) -> std::ffi::OsString {
    let mut paths = vec![worker_dir.to_path_buf()];
    if let Some(existing) = std::env::var_os("PYTHONPATH") {
        paths.extend(std::env::split_paths(&existing));
    }
    std::env::join_paths(paths).unwrap_or_else(|_| worker_dir.as_os_str().to_owned())
}

#[derive(Debug, PartialEq, Eq)]
enum LimitedLine {
    Line(Vec<u8>),
    /// The retained value is only the bounded prefix; the remainder was drained.
    TooLong(Vec<u8>),
    Eof,
}

/// Reads and drains one line while retaining at most `limit` bytes.
fn read_limited_line(reader: &mut impl BufRead, limit: usize) -> io::Result<LimitedLine> {
    let mut line = Vec::new();
    let mut too_long = false;
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return Ok(if too_long {
                LimitedLine::TooLong(line)
            } else if line.is_empty() {
                LimitedLine::Eof
            } else {
                LimitedLine::Line(line)
            });
        }

        let newline = available.iter().position(|byte| *byte == b'\n');
        let content_len = newline.unwrap_or(available.len());
        if !too_long {
            let retained = content_len.min(limit.saturating_sub(line.len()));
            line.extend_from_slice(&available[..retained]);
            too_long = retained < content_len;
        }
        let consumed = newline.map_or(available.len(), |position| position + 1);
        reader.consume(consumed);
        if newline.is_some() {
            return Ok(if too_long {
                LimitedLine::TooLong(line)
            } else {
                LimitedLine::Line(line)
            });
        }
    }
}

fn worker_message_limit(line: &[u8]) -> usize {
    if line.len() <= MAX_CONTROL_MESSAGE_BYTES {
        return MAX_CONTROL_MESSAGE_BYTES;
    }
    // Channel.send emits compact JSON and puts the top-level type near the
    // start. Looking only at that bounded prefix avoids parsing a large mesh
    // message twice. Quotes inside a JSON string are escaped and therefore
    // cannot match these field encodings.
    const HEADER_BYTES: usize = 1024;
    const MESH_TYPES: [&[u8]; 5] = [
        br#""type":"run_finished""#,
        br#""type":"run_part""#,
        br#""type":"debug_paused""#,
        br#""type":"inspection_geometry""#,
        br#""type":"scene_add""#,
    ];
    let header = &line[..line.len().min(HEADER_BYTES)];
    if header
        .windows(br#""type":"display""#.len())
        .any(|window| window == br#""type":"display""#)
    {
        return MAX_DISPLAY_MESSAGE_BYTES;
    }
    if MESH_TYPES
        .iter()
        .any(|kind| header.windows(kind.len()).any(|window| window == *kind))
    {
        MAX_MESH_MESSAGE_BYTES
    } else {
        MAX_CONTROL_MESSAGE_BYTES
    }
}

/// What the user is told about a message over its limit. Mesh replies are
/// the large ones: a finer tolerance or more objects make them grow.
fn oversized_message(actual: usize, limit: usize) -> String {
    const MIB: usize = 1024 * 1024;
    let advice = if limit >= MAX_MESH_MESSAGE_BYTES {
        "; the model is too large to transfer: raise Deviation in \
         Preferences > 3D View, or show fewer objects"
    } else {
        ""
    };
    format!(
        "Python sent a message of at least {:.1} MiB, over its {} MiB limit{advice}",
        actual as f64 / MIB as f64,
        limit / MIB
    )
}

#[cfg(test)]
#[path = "../../tests/rust/protocol/test_worker_io.rs"]
mod tests;
