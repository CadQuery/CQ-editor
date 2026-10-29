//! Script runs and the requests that still wait for an answer.
//!
//! A pure state machine, without egui or a process, so every transition is
//! tested directly (`tests/rust/app/test_execution.rs`). `AppState` asks it
//! what a user request or a worker message leads to and carries out the side
//! effects: sending requests, updating the scene, the status bar and the log.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::scene::NodeKey;
use crate::worker::{
    self,
    protocol::{InterferencePart, RequestId},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Idle,
    Running {
        id: RequestId,
        debug: bool,
    },
    Paused {
        id: RequestId,
    },
}

/// What an outstanding request was for, so replies and errors can be routed.
impl Phase {
    /// The run going on, unless it is paused in the debugger.
    pub fn running_id(self) -> Option<RequestId> {
        match self {
            Phase::Running { id, .. } => Some(id),
            Phase::Idle | Phase::Paused { .. } => None,
        }
    }

    /// The run going on, paused in the debugger or not.
    pub fn run_id(self) -> Option<RequestId> {
        match self {
            Phase::Running { id, .. } | Phase::Paused { id } => Some(id),
            Phase::Idle => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Pending {
    Run,
    /// Completions for the editor of the document (generation)
    /// `document`; `automatic` ones were offered while typing a name, not
    /// asked for.
    Completion {
        automatic: bool,
        document: u64,
    },
    ConsoleCompletion,
    /// Documentation of the name under the pointer in `document`.
    Describe {
        document: u64,
    },
    Outline {
        document: u64,
    },
    Navigate {
        document: u64,
    },
    Inspect,
    /// The construction geometry of one inspector entry: the workplane and
    /// its selected (frame, item).
    InspectGeometry {
        key: NodeKey,
        entry: (usize, Option<usize>),
    },
    InterferenceGeometry {
        first: InterferencePart,
        second: InterferencePart,
    },
    Console,
    Export,
    Check,
    Upload,
    /// `automatic` checks, once a day, stay silent unless a newer
    /// CQ-editor is out.
    Updates {
        automatic: bool,
    },
    /// The local modules imported by the script at this path.
    Imports(PathBuf),
    Measure,
    /// Selector expressions for the topology selection.
    Selector,
    SelectorPreview,
    /// Code repeating the current measurement, for the clipboard.
    MeasurementCode,
}

/// Whether the worker can take a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkerAvailability {
    Ready,
    Starting,
    Unavailable,
}

impl From<&worker::Status> for WorkerAvailability {
    fn from(status: &worker::Status) -> Self {
        match status {
            worker::Status::Ready(_) => Self::Ready,
            worker::Status::Starting => Self::Starting,
            worker::Status::NotStarted | worker::Status::Failed(_) => Self::Unavailable,
        }
    }
}

/// What a request to run the script leads to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunRequest {
    /// Send the run now and report it with [`Execution::started`].
    Start,
    /// Python is still starting; [`Execution::worker_ready`] returns the run.
    WhenReady,
    /// There is no Python to run it.
    Unavailable,
    /// A render is running; one more render follows it.
    Queued,
    /// A debugging session runs, or a render runs and debugging was asked.
    Busy,
}

/// What follows the end of a run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RunEnd {
    /// The run belongs to a document that was replaced since: its results
    /// would look like the output of the new script and are discarded.
    pub stale: bool,
    /// A render was requested while it ran and starts now.
    pub rerun: bool,
}

#[derive(Debug, Default)]
pub struct Execution {
    phase: Phase,
    /// A render was requested while one was running.
    rerun: bool,
    /// The run of a document that was replaced since.
    stale_run: Option<RequestId>,
    /// Render as soon as the worker is ready (`Some(debug)`).
    run_when_ready: Option<bool>,
    pending: HashMap<RequestId, Pending>,
    meshing: Option<Meshing>,
    /// Stop interrupted this request and waits for it to answer.
    stopping: Option<(RequestId, Instant)>,
}

/// How long Stop waits for an interrupted script before restarting Python.
pub const STOP_GRACE: Duration = Duration::from_secs(3);

/// What Stop does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopPlan {
    /// Nothing is running.
    Nothing,
    /// Interrupt the request (`KeyboardInterrupt`) and wait for it.
    Interrupt(RequestId),
    /// Restart Python: Stop was pressed again while waiting.
    Restart,
}

/// How far the worker is in meshing the running run's objects.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Meshing {
    pub done: u32,
    pub total: u32,
    /// When the first progress report arrived.
    pub since: Instant,
}

impl Execution {
    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn is_busy(&self) -> bool {
        self.phase != Phase::Idle
    }

    pub fn is_paused(&self) -> bool {
        matches!(self.phase, Phase::Paused { .. })
    }

    pub fn is_debugging(&self) -> bool {
        matches!(
            self.phase,
            Phase::Paused { .. } | Phase::Running { debug: true, .. }
        )
    }

    // -- requests ---------------------------------------------------------------

    pub fn track(&mut self, id: RequestId, kind: Pending) {
        self.pending.insert(id, kind);
    }

    /// What the request `id` was for, without answering it.
    pub fn pending(&self, id: RequestId) -> Option<&Pending> {
        self.pending.get(&id)
    }

    pub fn is_pending(&self, id: RequestId) -> bool {
        self.pending.contains_key(&id)
    }

    /// The request `id` was answered: what it was for, once.
    pub fn answer(&mut self, id: RequestId) -> Option<Pending> {
        self.forget_stop(id);
        self.pending.remove(&id)
    }

    // -- stopping -----------------------------------------------------------------

    /// What Stop does now: interrupt the running script, else a console
    /// command, and restart Python if Stop is pressed again before it
    /// answers. A paused debugging session is stopped by the debugger.
    pub fn stop(&mut self, now: Instant) -> StopPlan {
        if let Some((target, _)) = self.stopping.take()
            && self.pending.contains_key(&target)
        {
            return StopPlan::Restart;
        }
        let console = || {
            self.pending
                .iter()
                .filter(|(_, kind)| **kind == Pending::Console)
                .map(|(id, _)| *id)
                .min()
        };
        match self.phase.running_id().or_else(console) {
            Some(target) => {
                self.stopping = Some((target, now));
                StopPlan::Interrupt(target)
            }
            None => StopPlan::Nothing,
        }
    }

    /// When Python is restarted if the interrupted request has not
    /// answered.
    pub fn stop_deadline(&self) -> Option<Instant> {
        self.stopping.map(|(_, since)| since + STOP_GRACE)
    }

    pub fn is_stopping(&self) -> bool {
        self.stopping.is_some()
    }

    fn forget_stop(&mut self, id: RequestId) {
        if self.stopping.is_some_and(|(target, _)| target == id) {
            self.stopping = None;
        }
    }

    // -- runs ---------------------------------------------------------------------

    /// A render (`debug` false) or a debugging session was asked for.
    pub fn request_run(&mut self, debug: bool, worker: WorkerAvailability) -> RunRequest {
        match worker {
            WorkerAvailability::Ready => {}
            WorkerAvailability::Starting => {
                self.run_when_ready = Some(debug);
                return RunRequest::WhenReady;
            }
            WorkerAvailability::Unavailable => return RunRequest::Unavailable,
        }
        match self.phase {
            Phase::Idle => RunRequest::Start,
            Phase::Running { debug: false, .. } if !debug => {
                self.rerun = true;
                RunRequest::Queued
            }
            _ => RunRequest::Busy,
        }
    }

    /// The run `id` was sent.
    pub fn started(&mut self, id: RequestId, debug: bool) {
        self.pending.insert(id, Pending::Run);
        self.phase = Phase::Running { id, debug };
        self.meshing = None;
    }

    /// The worker meshed `done` of the `total` parts of run `id`. Reports
    /// for anything but the running run are ignored.
    pub fn meshing_progress(&mut self, id: RequestId, done: u32, total: u32, now: Instant) {
        if self.phase.running_id() == Some(id) {
            let since = self.meshing.map_or(now, |meshing| meshing.since);
            self.meshing = Some(Meshing { done, total, since });
        }
    }

    /// The meshing of the running run, while it goes on.
    pub fn meshing(&self) -> Option<Meshing> {
        self.meshing.filter(|meshing| meshing.done < meshing.total)
    }

    /// The debugger stopped the run `id`. Returns whether the run is stale,
    /// and should be stopped instead of shown.
    pub fn paused(&mut self, id: RequestId) -> bool {
        self.phase = Phase::Paused { id };
        self.stale_run == Some(id)
    }

    /// A debugger command continues the paused run, whose id is returned.
    pub fn resume(&mut self) -> Option<RequestId> {
        let Phase::Paused { id } = self.phase else {
            return None;
        };
        self.phase = Phase::Running { id, debug: true };
        Some(id)
    }

    /// The run `id` finished or failed.
    pub fn ended(&mut self, id: RequestId) -> RunEnd {
        self.forget_stop(id);
        self.pending.remove(&id);
        self.phase = Phase::Idle;
        self.meshing = None;
        RunEnd {
            stale: self.stale_run.take() == Some(id),
            rerun: std::mem::take(&mut self.rerun),
        }
    }

    /// The worker became ready: the run asked for while it started, if any.
    pub fn worker_ready(&mut self) -> Option<bool> {
        self.run_when_ready.take()
    }

    /// The document was replaced. Queued runs are dropped and a run still
    /// going becomes stale. Returns whether that run is paused and must be
    /// stopped.
    pub fn abandon(&mut self) -> bool {
        self.rerun = false;
        self.run_when_ready = None;
        match self.phase {
            Phase::Idle => false,
            Phase::Running { id, .. } => {
                self.stale_run = Some(id);
                false
            }
            Phase::Paused { id } => {
                self.stale_run = Some(id);
                true
            }
        }
    }

    /// The worker restarted: nothing it was doing will answer. A render
    /// asked for while Python starts still follows.
    pub fn reset(&mut self) {
        self.phase = Phase::Idle;
        self.rerun = false;
        self.stale_run = None;
        self.pending.clear();
        self.meshing = None;
        self.stopping = None;
    }
}

#[cfg(test)]
#[path = "../../tests/rust/app/test_execution.rs"]
mod tests;
