use std::time::Instant;

use super::*;

use WorkerAvailability::{Ready, Starting, Unavailable};

/// An execution with the render `id` running.
fn rendering(id: RequestId) -> Execution {
    let mut execution = Execution::default();
    assert_eq!(execution.request_run(false, Ready), RunRequest::Start);
    execution.started(id, false);
    execution
}

#[test]
fn a_render_starts_when_idle_and_is_tracked_until_it_ends() {
    let mut execution = rendering(1);

    assert_eq!(
        execution.phase(),
        Phase::Running {
            id: 1,
            debug: false
        }
    );
    assert!(execution.is_busy() && !execution.is_debugging());
    assert_eq!(execution.pending(1), Some(&Pending::Run));

    assert_eq!(execution.ended(1), RunEnd::default());
    assert_eq!(execution.phase(), Phase::Idle);
    assert!(!execution.is_pending(1));
}

#[test]
fn renders_asked_for_during_a_render_collapse_into_one_rerun() {
    let mut execution = rendering(1);

    assert_eq!(execution.request_run(false, Ready), RunRequest::Queued);
    assert_eq!(execution.request_run(false, Ready), RunRequest::Queued);
    assert_eq!(
        execution.ended(1),
        RunEnd {
            stale: false,
            rerun: true
        }
    );
    // The rerun is reported once.
    execution.started(2, false);
    assert_eq!(execution.ended(2), RunEnd::default());
}

#[test]
fn debugging_never_queues_behind_or_in_front_of_another_run() {
    let mut execution = rendering(1);
    assert_eq!(execution.request_run(true, Ready), RunRequest::Busy);
    execution.ended(1);

    execution.started(2, true);
    assert!(execution.is_debugging());
    assert_eq!(execution.request_run(false, Ready), RunRequest::Busy);
    assert_eq!(execution.request_run(true, Ready), RunRequest::Busy);
    assert!(!execution.ended(2).rerun);
}

#[test]
fn a_run_asked_for_while_python_starts_follows_once_it_is_ready() {
    let mut execution = Execution::default();

    assert_eq!(
        execution.request_run(false, Starting),
        RunRequest::WhenReady
    );
    // The last request wins: debugging was asked for after rendering.
    assert_eq!(execution.request_run(true, Starting), RunRequest::WhenReady);
    assert!(!execution.is_busy());
    assert_eq!(execution.worker_ready(), Some(true));
    assert_eq!(execution.worker_ready(), None);
}

#[test]
fn no_run_starts_without_python() {
    let mut execution = Execution::default();

    assert_eq!(
        execution.request_run(false, Unavailable),
        RunRequest::Unavailable
    );
    assert_eq!(execution.worker_ready(), None);
    assert!(!execution.is_busy());
}

#[test]
fn worker_status_maps_to_availability() {
    assert_eq!(
        WorkerAvailability::from(&worker::Status::Starting),
        Starting
    );
    assert_eq!(
        WorkerAvailability::from(&worker::Status::NotStarted),
        Unavailable
    );
    assert_eq!(
        WorkerAvailability::from(&worker::Status::Failed("no Python".into())),
        Unavailable
    );
}

#[test]
fn a_paused_session_resumes_as_a_running_debug_session() {
    let mut execution = Execution::default();
    execution.started(4, true);

    assert!(!execution.paused(4), "the session of this document");
    assert!(execution.is_paused() && execution.is_debugging());
    // Paused, the run is not running but still goes on.
    assert_eq!(execution.phase().running_id(), None);
    assert_eq!(execution.phase().run_id(), Some(4));
    assert_eq!(execution.resume(), Some(4));
    assert_eq!(execution.phase().run_id(), Some(4));
    assert_eq!(execution.phase(), Phase::Running { id: 4, debug: true });
    // Only a paused session can be resumed.
    assert_eq!(execution.resume(), None);
}

#[test]
fn replacing_the_document_discards_the_running_render_and_queued_runs() {
    let mut execution = rendering(1);
    execution.request_run(false, Ready);

    assert!(!execution.abandon(), "a running render needs no stop");
    assert_eq!(
        execution.ended(1),
        RunEnd {
            stale: true,
            rerun: false
        }
    );
    // The stale mark does not outlive its run.
    execution.started(2, false);
    assert!(!execution.ended(2).stale);
}

#[test]
fn replacing_the_document_drops_a_run_waiting_for_python() {
    let mut execution = Execution::default();
    execution.request_run(false, Starting);

    assert!(!execution.abandon());
    assert_eq!(execution.worker_ready(), None);
}

#[test]
fn replacing_the_document_stops_a_paused_session() {
    let mut execution = Execution::default();
    execution.started(3, true);
    execution.paused(3);

    assert!(execution.abandon(), "the paused session must be stopped");
    assert_eq!(execution.resume(), Some(3));
    assert!(execution.ended(3).stale);
}

#[test]
fn a_stale_session_reaching_a_breakpoint_is_reported_as_stale() {
    let mut execution = Execution::default();
    execution.started(5, true);
    execution.abandon();

    assert!(execution.paused(5));
}

#[test]
fn a_failed_run_still_starts_the_render_queued_behind_it() {
    // BUG-27: a run request that failed in the worker dropped a queued render.
    let mut execution = rendering(1);
    execution.request_run(false, Ready);

    assert_eq!(execution.answer(1), Some(Pending::Run));
    assert_eq!(
        execution.ended(1),
        RunEnd {
            stale: false,
            rerun: true
        }
    );
    assert_eq!(execution.phase(), Phase::Idle);
}

#[test]
fn a_worker_restart_forgets_its_requests_but_not_a_render_asked_for_meanwhile() {
    let mut execution = rendering(1);
    execution.request_run(false, Ready);
    execution.abandon();
    execution.track(2, Pending::Console);
    execution.reset();

    assert_eq!(execution.phase(), Phase::Idle);
    assert!(!execution.is_pending(1) && !execution.is_pending(2));
    execution.started(3, false);
    assert_eq!(
        execution.ended(3),
        RunEnd::default(),
        "no stale mark, no rerun"
    );

    // F5 while the restarted Python is still importing CadQuery.
    execution.request_run(false, Starting);
    execution.reset();
    assert_eq!(execution.worker_ready(), Some(false));
}

#[test]
fn every_reply_is_routed_once() {
    let mut execution = Execution::default();
    let entry = (1, Some(2));
    execution.track(
        7,
        Pending::InspectGeometry {
            key: "s1-n1".into(),
            entry,
        },
    );

    assert!(execution.is_pending(7));
    assert_eq!(
        execution.answer(7),
        Some(Pending::InspectGeometry {
            key: "s1-n1".into(),
            entry
        })
    );
    assert_eq!(execution.answer(7), None);
    assert_eq!(execution.answer(8), None, "unknown ids are ignored");
}

#[test]
fn meshing_progress_is_kept_for_the_running_render_only() {
    let start = std::time::Instant::now();
    let later = start + std::time::Duration::from_millis(500);
    let mut execution = rendering(3);
    assert_eq!(execution.meshing(), None);

    execution.meshing_progress(2, 1, 10, start);
    assert_eq!(execution.meshing(), None, "another request");
    execution.meshing_progress(3, 0, 10, start);
    execution.meshing_progress(3, 4, 10, later);
    let meshing = execution.meshing().unwrap();
    assert_eq!((meshing.done, meshing.total), (4, 10));
    assert_eq!(meshing.since, start, "since the first report");

    execution.meshing_progress(3, 10, 10, later);
    assert_eq!(execution.meshing(), None, "done");
    execution.meshing_progress(3, 5, 10, later);
    execution.ended(3);
    assert_eq!(execution.meshing(), None);
    assert_eq!(execution.phase().running_id(), None);

    let mut execution = rendering(4);
    execution.meshing_progress(4, 1, 10, start);
    execution.reset();
    assert_eq!(
        execution.meshing(),
        None,
        "a restarted worker answers nothing"
    );
}

#[test]
fn stop_interrupts_first_and_restarts_python_only_when_pressed_again() {
    let now = Instant::now();
    let mut execution = rendering(5);
    assert_eq!(execution.stop(now), StopPlan::Interrupt(5));
    assert!(execution.is_stopping());
    assert_eq!(execution.stop_deadline(), Some(now + STOP_GRACE));
    assert_eq!(execution.stop(now), StopPlan::Restart, "pressed again");

    // Stopped in time: nothing more to do.
    let mut execution = rendering(6);
    execution.stop(now);
    execution.ended(6);
    assert!(!execution.is_stopping() && execution.stop_deadline().is_none());
    assert_eq!(execution.stop(now), StopPlan::Nothing);

    // Without a script, a console command is what Stop stops.
    let mut execution = Execution::default();
    execution.track(
        9,
        Pending::Completion {
            automatic: false,
            document: 0,
        },
    );
    execution.track(8, Pending::Console);
    assert_eq!(execution.stop(now), StopPlan::Interrupt(8));
    execution.answer(8);
    assert!(!execution.is_stopping());

    // A restarted worker answers nothing more.
    let mut execution = rendering(10);
    execution.stop(now);
    execution.reset();
    assert!(!execution.is_stopping());
}
