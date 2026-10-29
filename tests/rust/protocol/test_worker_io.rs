use super::*;
use std::io::{BufReader, Cursor};

#[test]
fn shared_golden_messages_cross_the_python_rust_boundary() {
    let fixtures: serde_json::Value =
        serde_json::from_str(include_str!("../../fixtures/protocol/messages.json")).unwrap();
    for case in fixtures["valid"].as_array().unwrap() {
        let raw = serde_json::to_vec(&case["message"]).unwrap();
        assert!(raw.len() <= worker_message_limit(&raw), "{}", case["name"]);
        let message: Message = serde_json::from_slice(&raw).unwrap();
        match case["name"].as_str().unwrap() {
            "ready" => assert!(
                matches!(message, Message::Ready(info) if info.protocol == protocol::PROTOCOL_VERSION)
            ),
            "binary_part" => assert!(
                matches!(message, Message::RunPart { part, .. } if part.mesh.positions.len() == 3 && part.mesh.indices == [0, 1, 2])
            ),
            "referenced_part" => assert!(
                matches!(message, Message::RunPart { part, .. } if part.mesh.id == "triangle-geometry" && part.mesh.positions.is_empty())
            ),
            "finished_scene" => assert!(
                matches!(message, Message::RunFinished(result) if result.objects.as_ref().unwrap()[0].parts[0].mesh.id == "triangle-geometry")
            ),
            other => panic!("Unknown shared fixture: {other}"),
        }
    }

    for case in fixtures["invalid"].as_array().unwrap() {
        assert!(
            serde_json::from_str::<Message>(case["wire"].as_str().unwrap()).is_err(),
            "{}",
            case["name"]
        );
    }
    let oversized = &fixtures["oversized_control"];
    let payload = format!(
        r#"{{"type":"output","id":7,"stream":"stdout","text":"{}"}}"#,
        "x".repeat(oversized["repeat"].as_u64().unwrap() as usize)
    );
    assert_eq!(
        worker_message_limit(payload.as_bytes()),
        MAX_CONTROL_MESSAGE_BYTES
    );
    let mut reader = Cursor::new(payload.into_bytes());
    assert!(matches!(
        read_limited_line(&mut reader, MAX_CONTROL_MESSAGE_BYTES).unwrap(),
        LimitedLine::TooLong(_)
    ));
}

#[test]
fn limited_line_reader_caps_memory_and_resumes_at_the_next_line() {
    let input = Cursor::new(b"123456\nok\nlast".to_vec());
    let mut reader = BufReader::with_capacity(3, input);

    assert_eq!(
        read_limited_line(&mut reader, 4).unwrap(),
        LimitedLine::TooLong(b"1234".to_vec())
    );
    assert_eq!(
        read_limited_line(&mut reader, 4).unwrap(),
        LimitedLine::Line(b"ok".to_vec())
    );
    assert_eq!(
        read_limited_line(&mut reader, 4).unwrap(),
        LimitedLine::Line(b"last".to_vec())
    );
    assert_eq!(read_limited_line(&mut reader, 4).unwrap(), LimitedLine::Eof);
}

#[test]
fn exact_line_limit_is_accepted() {
    let mut reader = Cursor::new(b"1234\n".to_vec());
    assert_eq!(
        read_limited_line(&mut reader, 4).unwrap(),
        LimitedLine::Line(b"1234".to_vec())
    );
}

#[test]
fn image_and_mesh_messages_have_separate_larger_limits() {
    let control = br#"{"type":"output","text":"hello"}"#;
    let display = format!(
        r#"{{"type":"display","image":{{"data":"{}"}}}}"#,
        "x".repeat(MAX_CONTROL_MESSAGE_BYTES)
    );
    let run = format!(
        r#"{{"id":1,"type":"run_finished","padding":"{}"}}"#,
        "x".repeat(MAX_CONTROL_MESSAGE_BYTES)
    );
    let inspection = format!(
        r#"{{"type":"inspection_geometry","padding":"{}"}}"#,
        "x".repeat(MAX_CONTROL_MESSAGE_BYTES)
    );

    assert_eq!(worker_message_limit(control), MAX_CONTROL_MESSAGE_BYTES);
    assert_eq!(
        worker_message_limit(display.as_bytes()),
        MAX_DISPLAY_MESSAGE_BYTES
    );
    assert_eq!(worker_message_limit(run.as_bytes()), MAX_MESH_MESSAGE_BYTES);
    assert_eq!(
        worker_message_limit(inspection.as_bytes()),
        MAX_MESH_MESSAGE_BYTES
    );
}

#[test]
fn a_large_unknown_or_malformed_message_keeps_the_control_limit() {
    let unknown = format!(
        r#"{{"type":"future","data":"{}"}}"#,
        "x".repeat(2 * 1024 * 1024)
    );
    let malformed = vec![b'x'; MAX_CONTROL_MESSAGE_BYTES + 1];

    assert_eq!(
        worker_message_limit(unknown.as_bytes()),
        MAX_CONTROL_MESSAGE_BYTES
    );
    assert!(unknown.len() > worker_message_limit(unknown.as_bytes()));
    assert_eq!(worker_message_limit(&malformed), MAX_CONTROL_MESSAGE_BYTES);

    let deceptive_output = format!(
        r#"{{"type":"output","text":"\"type\":\"run_finished\"{}"}}"#,
        "x".repeat(MAX_CONTROL_MESSAGE_BYTES)
    );
    assert_eq!(
        worker_message_limit(deceptive_output.as_bytes()),
        MAX_CONTROL_MESSAGE_BYTES
    );
}

#[test]
fn an_unreadable_message_is_not_a_failed_start() {
    let mut worker = Worker::new(egui::Context::default());
    worker.generation = 1;
    let ready: Message = serde_json::from_str(
        r#"{"type":"ready","protocol":7,"python":"3.12","cadquery":"2.8.0",
            "ocp":"7.9","executable":"python","completion":true,"pid":1}"#,
    )
    .unwrap();
    worker.events_tx.send((1, Event::Message(ready))).unwrap();
    worker.poll();
    assert!(worker.is_ready());

    let message = oversized_message(MAX_MESH_MESSAGE_BYTES + 1, MAX_MESH_MESSAGE_BYTES);
    worker
        .events_tx
        .send((1, Event::ProtocolError(message.clone())))
        .unwrap();
    let events = worker.poll();
    assert!(matches!(&events[..], [Event::ProtocolError(m)] if *m == message));
    // CadQuery is not said to be missing: the app restarts the worker.
    assert!(worker.is_ready());
    assert_eq!(worker.problem(), None);
    assert!(message.contains("too large to transfer"), "{message}");
    assert!(message.contains("Deviation"), "{message}");
    assert!(message.contains("512.0 MiB, over its 512 MiB"), "{message}");

    let control = oversized_message(3 * 1024 * 1024, MAX_CONTROL_MESSAGE_BYTES);
    assert!(!control.contains("Deviation"), "{control}");
}

#[test]
fn a_failed_start_says_whether_python_or_cadquery_is_missing() {
    // The Microsoft Store placeholder for python.exe exits with 9009.
    let expected = if cfg!(windows) {
        Problem::NoPython
    } else {
        Problem::Exited
    };
    assert_eq!(exit_problem(Some(9009)), expected);
    assert_eq!(exit_problem(Some(1)), Problem::Exited);
    assert_eq!(exit_problem(None), Problem::Exited);

    // Only a failed worker has a problem.
    let mut worker = Worker::new(egui::Context::default());
    worker.problem = Some(Problem::NoCadQuery);
    assert_eq!(worker.problem(), None);
    worker.status = Status::Failed("Cannot load CadQuery".into());
    assert_eq!(worker.problem(), Some(Problem::NoCadQuery));
}
