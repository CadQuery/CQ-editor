use super::*;

#[test]
fn only_web_links_are_opened() {
    assert!(is_web_url("https://cadquery.readthedocs.io/en/latest/"));
    assert!(is_web_url("HTTP://example.com/model?id=1"));
    // Links from an upload service's reply must not start local programs.
    assert!(!is_web_url(r"C:\Windows\System32\calc.exe"));
    assert!(!is_web_url("file:///etc/passwd"));
    assert!(!is_web_url("javascript:alert(1)"));
    assert!(!is_web_url("https://"));
    assert!(!is_web_url("https://example.com/\nsecond line"));
}

#[test]
fn the_documentation_link_points_to_these_docs() {
    assert!(DOCUMENTATION_URL.ends_with("/docs/README.md"));
    assert!(!DOCUMENTATION_URL.contains("/wiki"));
    assert!(is_web_url(DOCUMENTATION_URL));
}

#[test]
fn links_clicked_in_widgets_are_taken_from_the_platform_output() {
    let ctx = egui::Context::default();
    let mut urls = Vec::new();
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        ui.ctx()
            .open_url(egui::OpenUrl::new_tab("https://example.com/a"));
        ui.ctx().copy_text("kept".to_owned());
        urls = take_requested_urls(ui.ctx());
    });
    output.textures_delta.clear();

    assert_eq!(urls, ["https://example.com/a"]);
    // Other commands, such as copying, still reach the platform.
    assert_eq!(
        output.platform_output.commands,
        [egui::OutputCommand::CopyText("kept".to_owned())]
    );
}

// -- application state without a window ----------------------------------------

/// An `AppState` with defaults and no Python, for exercising actions and
/// worker replies directly.
pub(crate) fn test_state(text: &str) -> AppState {
    let ctx = egui::Context::default();
    let cache_dir = std::env::temp_dir().join("cq-editor-app-tests");
    AppState {
        worker: Worker::new(ctx.clone()),
        ctx,
        settings: Settings::default(),
        document: Document::untitled(text),
        editor: CodeEditor::default(),
        tabs: EditorTabs::new(0),
        project: None,
        scene: Scene::default(),
        viewer: Viewer::default(),
        variables: Vec::new(),
        traceback: None,
        traceback_document: None,
        inspector: InspectorState::default(),
        measurements: MeasurementState::default(),
        selector: SelectorState::default(),
        selector_preview: SelectorPreviewState::default(),
        navigation: NavigationState::default(),
        parameters: ParametersState::default(),
        parameter_render_at: None,
        document_generation: 0,
        run_script: None,
        console: ConsoleState::default(),
        log_view: LogView::default(),
        status: None,
        dialogs: Dialogs::default(),
        geometry_checks: std::collections::HashMap::new(),
        cad_checks: Vec::new(),
        cad_checks_document: None,
        export_job: None,
        execution: Execution::default(),
        actions: Vec::new(),
        watcher: FileWatcher::default(),
        watch_key: None,
        focused_last_frame: false,
        imports: Vec::new(),
        examples: Vec::new(),
        palette: Palette::DARK,
        recovery: RecoveryStore::new(cache_dir.join("recovery")),
        cache_dir,
        next_recovery: Instant::now() + CHECKPOINT_INTERVAL,
        recovery_error_reported: false,
        python_override: None,
        worker_was_ready: false,
        allow_close: false,
        title: String::new(),
        applied_theme: None,
        applied_scale: 1.0,
        logo: None,
        editor_panel_open: true,
    }
}

fn selector_reply(id: crate::worker::protocol::RequestId) -> crate::worker::protocol::Message {
    use crate::worker::protocol::{Message, SelectorCandidate, SelectorReply};
    let candidate = |expression: &str, index_based| SelectorCandidate {
        expression: expression.to_owned(),
        description: String::new(),
        index_based,
    };
    Message::Selector(SelectorReply {
        id,
        method: "faces".into(),
        candidates: vec![
            candidate(r#".faces(">Z")"#, false),
            candidate(".faces()[5]", true),
        ],
    })
}

/// A face picked in the view, as `select` records it.
fn pick_face(state: &mut AppState) {
    use crate::scene::{TopologyEntity, TopologySelection};
    let mut mesh = crate::scene::Mesh::new(
        vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        vec![[0.0, 0.0, 1.0]; 3],
        vec![0, 1, 2],
        Vec::new(),
        Vec::new(),
    );
    mesh.topology.triangle_faces = vec![6];
    let node = crate::scene::SceneNode {
        key: "n1".into(),
        name: "box".into(),
        kind: "Workplane".into(),
        inspectable: false,
        parts: vec![crate::scene::Part {
            mesh: std::sync::Arc::new(mesh),
            transform: glam::Mat4::IDENTITY,
            color: None,
            alpha: None,
            face_attributes: Vec::new(),
            material: None,
            render_mesh: None,
        }],
        children: Vec::new(),
        visible: true,
        color: None,
        alpha: None,
        edge_color: None,
        finish: None,
        overrides: Default::default(),
        line: None,
        script: None,
    };
    state.scene.append(vec![node], None);
    state.scene.select_only("n1");
    assert!(state.scene.select_topology(
        TopologySelection {
            node: "n1".into(),
            part: 0,
            entity: TopologyEntity::Face(6),
            position: Default::default(),
        },
        false,
    ));
}

/// Pretends a selector request is out, as `request_selector` sends it.
fn awaiting_selector(state: &mut AppState, id: crate::worker::protocol::RequestId) {
    state.execution.track(id, Pending::Selector);
    state.selector.loading = true;
    state.selector.request_id = Some(id);
}

#[test]
fn stale_selector_preview_replies_do_not_replace_the_current_caret() {
    let mut state = test_state("part.faces(\">Z\")");
    state.execution.track(7, Pending::SelectorPreview);
    state.selector_preview.request_id = Some(7);
    state.handle_message(crate::worker::protocol::Message::SelectorPreview {
        id: 6,
        active: true,
        count: 4,
        matches: Vec::new(),
        message: None,
    });
    assert!(!state.selector_preview.active);
    state.handle_message(crate::worker::protocol::Message::SelectorPreview {
        id: 7,
        active: true,
        count: 1,
        matches: Vec::new(),
        message: Some("No displayed match".into()),
    });
    assert_eq!(state.selector_preview.count, 1);
    assert_eq!(
        state.selector_preview.message.as_deref(),
        Some("No displayed match")
    );
}

#[test]
fn navigation_discards_replies_for_edited_source_and_old_outline_requests() {
    use crate::worker::protocol::{Message, NavigationLocation, NavigationMode, OutlineSymbol};

    let mut state = test_state("old_name = 1\n");
    state.navigation.document = 0;
    state.navigation.source = state.document.text.clone();
    state.navigation.outline_request = Some(8);
    state.execution.track(7, Pending::Outline { document: 0 });
    state.execution.track(8, Pending::Outline { document: 0 });
    let symbols = vec![OutlineSymbol {
        name: "old_name".into(),
        kind: "variable".into(),
        line: 1,
        depth: 0,
    }];
    state.handle_message(Message::Outline {
        id: 7,
        symbols: symbols.clone(),
    });
    assert!(state.navigation.symbols.is_empty());
    state.document.text = "new_name = 1\n".into();
    state.handle_message(Message::Outline { id: 8, symbols });
    assert!(state.navigation.symbols.is_empty());

    state.navigation.navigation_request = Some(9);
    state.navigation.navigation_source = "old_name = 1\n".into();
    state.execution.track(9, Pending::Navigate { document: 0 });
    state.handle_message(Message::Navigation {
        id: 9,
        available: true,
        mode: NavigationMode::References,
        locations: vec![NavigationLocation {
            name: "old_name".into(),
            kind: "statement".into(),
            path: None,
            line: 1,
            column: 0,
        }],
        truncated: false,
    });
    assert!(state.navigation.locations.is_empty());
}

#[test]
fn a_selector_asked_for_while_loading_is_inserted_when_it_arrives() {
    let mut state = test_state("result = cq.Workplane().box(1, 1, 1)\n");
    pick_face(&mut state);
    awaiting_selector(&mut state, 7);
    state.use_selector(SelectorUse::Insert);
    assert_eq!(state.selector.then, Some(SelectorUse::Insert));
    assert!(!state.document.text.contains("faces"));

    // A late reply to an earlier selection changes nothing.
    state.handle_message(selector_reply(3));
    assert!(state.selector.reply.is_none());

    state.handle_message(selector_reply(7));
    assert_eq!(state.selector.then, None);
    assert!(!state.selector.loading);
    assert_eq!(
        state.document.text, ".faces(\">Z\")result = cq.Workplane().box(1, 1, 1)\n",
        "inserted at the caret, at the start of a fresh document"
    );
    assert_eq!(state.status.as_ref().unwrap().kind, StatusKind::Success);

    // One undo step takes it out again.
    state.editor.undo(&mut state.document);
    assert_eq!(
        state.document.text,
        "result = cq.Workplane().box(1, 1, 1)\n"
    );
}

#[test]
fn copying_a_selector_puts_it_on_the_clipboard_and_flags_index_fallbacks() {
    let mut state = test_state("");
    pick_face(&mut state);
    awaiting_selector(&mut state, 4);
    state.handle_message(selector_reply(4));

    let ctx = state.ctx.clone();
    let mut output = ctx.run_ui(egui::RawInput::default(), |_| {
        state.use_selector(SelectorUse::Copy);
    });
    output.textures_delta.clear();
    assert_eq!(
        output.platform_output.commands,
        [egui::OutputCommand::CopyText(r#".faces(">Z")"#.to_owned())]
    );
    assert!(!state.status.as_ref().unwrap().text.contains("index"));

    // The alternative chosen in the panel is used, with a caution.
    state.selector.chosen = 1;
    let mut output = ctx.run_ui(egui::RawInput::default(), |_| {
        state.use_selector(SelectorUse::Copy);
    });
    output.textures_delta.clear();
    assert_eq!(
        output.platform_output.commands,
        [egui::OutputCommand::CopyText(".faces()[5]".to_owned())]
    );
    assert!(state.status.as_ref().unwrap().text.contains("by index"));
}

#[test]
fn a_selector_error_is_kept_and_reported_to_a_waiting_copy() {
    let mut state = test_state("");
    pick_face(&mut state);
    awaiting_selector(&mut state, 9);
    state.use_selector(SelectorUse::Copy);
    state.request_failed(Some(9), "Selectors are generated for workplanes".into());

    assert_eq!(
        state.selector.error.as_deref(),
        Some("Selectors are generated for workplanes")
    );
    assert!(!state.selector.loading);
    let status = state.status.as_ref().unwrap();
    assert_eq!(status.kind, StatusKind::Warning);
    assert!(status.text.contains("workplanes"));
}

#[test]
fn selector_actions_need_picked_entities() {
    let mut state = test_state("");
    state.use_selector(SelectorUse::Insert);
    assert_eq!(state.status.as_ref().unwrap().kind, StatusKind::Warning);
    assert!(state.document.text.is_empty());
}

#[test]
fn the_console_gets_the_picked_entities_and_the_selected_objects() {
    use crate::worker::protocol::TopologyKind;
    let mut state = test_state("");
    assert_eq!(state.console_selection(), Default::default());

    pick_face(&mut state);
    let selection = state.console_selection();
    assert_eq!(selection.objects, ["n1"]);
    let [entity] = selection.entities.as_slice() else {
        panic!("one picked face");
    };
    assert_eq!(
        (entity.key.as_str(), entity.kind, entity.identifier),
        ("n1", TopologyKind::Face, 6)
    );
}

#[test]
fn a_parameter_change_rewrites_its_literal_and_renders_once_it_settles() {
    let script = "width = 40  # [10, 100]\nheight = 12.5\nresult = box(width, height)\n";
    let mut state = test_state(script);

    state.set_parameter("height", 2, 20.25);
    assert_eq!(
        state.document.text,
        "width = 40  # [10, 100]\nheight = 20.2\nresult = box(width, height)\n"
    );
    assert!(state.document.is_modified());
    let due = state.parameter_render_at.expect("a render is scheduled");

    // Not yet: the drag may go on.
    state.parameter_render_at = Some(f64::MAX);
    let ctx = state.ctx.clone();
    state.poll_parameter_render(&ctx);
    assert!(state.parameter_render_at.is_some());
    assert!(state.status.is_none());

    // Once due, it renders (here: reports that Python is not running).
    state.parameter_render_at = Some(due - 10.0);
    state.poll_parameter_render(&ctx);
    assert!(state.parameter_render_at.is_none());
    assert!(state.status.as_ref().unwrap().text.contains("Python"));
}

#[test]
fn a_parameter_is_found_again_by_line_and_name() {
    let mut state = test_state("width = 40\n");
    state.set_parameter("width", 1, 55.0);
    assert_eq!(state.document.text, "width = 55\n");
    // Edited meanwhile into something that is no longer a parameter.
    state.document.text = "width = w0\n".to_owned();
    state.set_parameter("width", 1, 60.0);
    assert_eq!(state.document.text, "width = w0\n");
    // Another name on that line is another parameter.
    state.document.text = "depth = 3\n".to_owned();
    state.set_parameter("width", 1, 60.0);
    assert_eq!(state.document.text, "depth = 3\n");
}

#[test]
fn a_commented_range_bounds_the_written_value() {
    let mut state = test_state("width = 40  # [10, 100]\n");
    state.set_parameter("width", 1, 400.0);
    assert_eq!(state.document.text, "width = 100  # [10, 100]\n");
}

/// A scene with one node made on `line` by a run of `ran`.
fn shown_from_run(state: &mut AppState, ran: &str, line: usize) {
    pick_face(state);
    let script = std::sync::Arc::new(crate::scene::RunScript {
        document: state.document_generation,
        text: ran.to_owned(),
    });
    let node = state.scene.find_mut("n1").unwrap();
    node.line = Some(line);
    node.attach_script(&script);
}

fn status_text(state: &AppState) -> &str {
    &state.status.as_ref().unwrap().text
}

#[test]
fn going_to_the_source_follows_the_edits_made_since_the_run() {
    let ran = "import cadquery as cq\nbox = cq.Workplane().box(1, 2, 3)\n";
    let mut state = test_state(ran);
    shown_from_run(&mut state, ran, 2);

    state.go_to_source("n1", None);
    assert_eq!(status_text(&state), "Line 2");
    assert_eq!(state.status.as_ref().unwrap().kind, StatusKind::Info);

    state.document.text = format!("# a box\n{ran}");
    state.go_to_source("n1", None);
    assert_eq!(status_text(&state), "Line 3 (line 2 when the script ran)");

    state.document.text = state.document.text.replace("box(1, 2, 3)", "box(1, 2, 4)");
    state.go_to_source("n1", None);
    assert_eq!(state.status.as_ref().unwrap().kind, StatusKind::Warning);
    assert!(status_text(&state).contains("edited since the last run"));

    state.document.text = "import cadquery as cq\n".to_owned();
    state.go_to_source("n1", None);
    assert!(status_text(&state).contains("deleted"));
}

#[test]
fn objects_of_another_document_or_the_console_have_no_source_here() {
    let ran = "box = cq.Workplane().box(1, 1, 1)\n";
    let mut state = test_state(ran);
    shown_from_run(&mut state, ran, 1);
    state.document_generation += 1;
    state.go_to_source("n1", None);
    assert!(status_text(&state).contains("another script"));

    let node = state.scene.find_mut("n1").unwrap();
    node.script = None;
    state.go_to_source("n1", None);
    assert!(status_text(&state).contains("console"));
}

#[test]
fn inspector_steps_go_to_the_line_that_made_them() {
    use crate::worker::protocol::{StackFrame, StackItem};
    let ran = "plate = (\n    cq.Workplane()\n    .box(4, 4, 1)\n    .faces(\">Z\")\n)\n";
    let mut state = test_state(ran);
    shown_from_run(&mut state, ran, 4);
    state.inspector.key = Some("n1".into());
    state.inspector.frames = vec![StackFrame {
        type_name: "Workplane".into(),
        value: String::new(),
        line: Some(4),
        items: vec![StackItem {
            type_name: "Solid".into(),
            value: String::new(),
            displayable: true,
            line: Some(3),
        }],
    }];

    state.go_to_source("n1", Some((0, Some(0))));
    assert_eq!(status_text(&state), "Line 3");
    state.go_to_source("n1", Some((0, None)));
    assert_eq!(status_text(&state), "Line 4");
    // A step of another object's stack is not this node's.
    state.inspector.key = Some("other".into());
    state.go_to_source("n1", Some((0, None)));
    assert!(status_text(&state).contains("No script line"));
}

#[test]
fn only_the_run_that_made_the_nodes_gives_them_its_script() {
    let mut state = test_state("x = 1\n");
    let script = std::sync::Arc::new(crate::scene::RunScript {
        document: 0,
        text: "x = 1\n".into(),
    });
    state.run_script = Some((7, script));
    pick_face(&mut state);
    let mut nodes = std::mem::take(&mut state.scene.nodes);
    state.attach_run_script(6, &mut nodes);
    assert!(nodes[0].script.is_none());
    state.attach_run_script(7, &mut nodes);
    assert!(nodes[0].script.is_some());
}

#[test]
fn objects_are_measured_when_no_entity_is_picked() {
    let mut state = test_state("");
    pick_face(&mut state);
    let (selections, objects) = state.measured();
    assert_eq!(selections.len(), 1);
    assert!(objects.is_empty(), "picked entities come first");

    state.scene.clear_topology_selection();
    let (selections, objects) = state.measured();
    assert!(selections.is_empty());
    assert_eq!(objects, ["n1"]);
}

#[test]
fn measurement_code_goes_to_the_clipboard_with_its_check() {
    use crate::worker::protocol::Message;
    let mut state = test_state("");
    state.request_measurement_code();
    assert!(status_text(&state).contains("Select objects"));

    let ctx = state.ctx.clone();
    state.execution.track(11, Pending::MeasurementCode);
    let mut output = ctx.run_ui(egui::RawInput::default(), |_| {
        state.handle_message(Message::MeasurementCode {
            id: 11,
            code: "area_1 = face_1.Area()  # 40\n".into(),
            verified: true,
            problem: None,
        });
    });
    output.textures_delta.clear();
    assert_eq!(
        output.platform_output.commands,
        [egui::OutputCommand::CopyText(
            "area_1 = face_1.Area()  # 40\n".into()
        )]
    );
    assert_eq!(state.status.as_ref().unwrap().kind, StatusKind::Success);

    state.execution.track(12, Pending::MeasurementCode);
    let mut output = ctx.run_ui(egui::RawInput::default(), |_| {
        state.handle_message(Message::MeasurementCode {
            id: 12,
            code: "shape_1 = obj.val()\n".into(),
            verified: false,
            problem: Some("the Workplane is not a variable of the script".into()),
        });
    });
    output.textures_delta.clear();
    assert_eq!(state.status.as_ref().unwrap().kind, StatusKind::Warning);
    assert!(status_text(&state).contains("not a variable"));

    // A reply nobody asked for is not copied.
    let mut output = ctx.run_ui(egui::RawInput::default(), |_| {
        state.handle_message(Message::MeasurementCode {
            id: 99,
            code: "stale\n".into(),
            verified: true,
            problem: None,
        });
    });
    output.textures_delta.clear();
    assert!(output.platform_output.commands.is_empty());

    state.execution.track(13, Pending::MeasurementCode);
    state.request_failed(Some(13), "Not available while debugging".into());
    assert!(status_text(&state).contains("Cannot write the measurement as code"));
}

#[test]
fn explode_and_isolate_commands_change_what_the_view_shows() {
    let mut state = test_state("");

    // Isolating needs a selection.
    state.execute(Action::ToggleIsolate);
    assert!(!state.scene.is_isolating());
    assert_eq!(state.status.as_ref().unwrap().kind, StatusKind::Info);

    pick_face(&mut state);
    state.execute(Action::ToggleIsolate);
    assert!(state.scene.is_isolating());
    state.execute(Action::ToggleIsolate);
    assert!(!state.scene.is_isolating(), "the same command ends it");

    state.execute(Action::ToggleIsolate);
    state.scene.set_visible("n1", false);
    state.execute(Action::ShowAll);
    assert!(!state.scene.is_isolating());
    assert!(state.scene.find("n1").unwrap().visible);

    state.execute(Action::ToggleExplode);
    assert!(state.viewer.explode_controls);
    assert!(state.scene.explode() > 0.0, "opening spreads the parts");
    state.execute(Action::SetExplode(1.5));
    assert_eq!(state.scene.explode(), 1.5);
    state.execute(Action::ToggleExplode);
    assert!(!state.viewer.explode_controls);
    assert_eq!(state.scene.explode(), 0.0, "closing assembles them");
}

#[test]
fn named_views_follow_their_file() {
    use crate::document::Encoding;
    let dir = std::env::temp_dir().join(format!("cq-editor-views-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (a, b) = (dir.join("a.py"), dir.join("b.py"));
    std::fs::write(&a, "a = 1\n").unwrap();
    std::fs::write(&b, "b = 2\n").unwrap();

    let mut state = test_state("");
    state.open_path_with(&a, Encoding::Utf8);
    state.execute(Action::SaveView(Some("front".into())));
    assert_eq!(state.viewer.named_views.len(), 1);
    assert_eq!(state.status.as_ref().unwrap().kind, StatusKind::Success);

    state.open_path_with(&b, Encoding::Utf8);
    assert!(state.viewer.named_views.is_empty(), "b has no views");
    state.open_path_with(&a, Encoding::Utf8);
    assert_eq!(state.viewer.named_views[0].name, "front");

    // Going to a view brings its projection back.
    state.settings.viewer.projection = crate::settings::Projection::Perspective;
    state.execute(Action::GoToView(0));
    assert_eq!(
        state.settings.viewer.projection,
        crate::settings::Projection::Orthographic
    );

    // A new document starts without views.
    state.execute(Action::DeleteView(0));
    state.execute(Action::SaveView(None));
    state.open_path_with(&b, Encoding::Utf8);
    state.open_path_with(&a, Encoding::Utf8);
    assert_eq!(state.viewer.named_views[0].name, "View 1");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_ground_grid_and_scale_bar_are_toggled_from_the_view_menu() {
    let mut state = test_state("");
    assert!(state.settings.viewer.ground_grid && state.settings.viewer.scale_bar);
    state.execute(Action::ToggleGroundGrid);
    state.execute(Action::ToggleScaleBar);
    assert!(!state.settings.viewer.ground_grid && !state.settings.viewer.scale_bar);
}

#[test]
fn an_interrupted_run_is_reported_as_stopped_not_as_an_error() {
    let mut state = test_state("while True:\n    pass\n");
    state.execution.started(4, false);
    let message: crate::worker::protocol::Message = serde_json::from_str(
        r#"{"type":"run_finished","id":4,"ok":false,"debug":false,"stopped":false,
            "cancelled":false,"interrupted":true,"objects":null,"variables":null,
            "error":{"type_name":"KeyboardInterrupt","message":"","frames":[],"line":2,
                "text":"Traceback (most recent call last):\nKeyboardInterrupt\n"},
            "duration":1.0}"#,
    )
    .unwrap();
    state.handle_message(message);

    let status = state.status.as_ref().unwrap();
    assert_eq!(status.kind, StatusKind::Info);
    assert_eq!(status.text, "Stopped at line 2");
    assert_eq!(state.editor.error_line, None, "no error marker");
    assert!(
        state.traceback.is_none(),
        "the view does not offer it as an error"
    );
    assert!(!state.is_busy());
}

#[test]
fn a_profiled_run_puts_its_times_in_the_gutter_of_the_edited_script() {
    let run_text = "import time\ntime.sleep(1)\nx = 1\n";
    let mut state = test_state(run_text);
    state.execution.started(4, false);
    state.run_script = Some((
        4,
        std::sync::Arc::new(crate::scene::RunScript {
            document: 0,
            text: run_text.to_owned(),
        }),
    ));
    state.editor.sync_markers(run_text);
    // A line was added while it ran.
    state.document.text = format!("# timing\n{run_text}");
    let message: crate::worker::protocol::Message = serde_json::from_str(
        r#"{"type":"run_finished","id":4,"ok":true,"debug":false,"stopped":false,
            "objects":[],"variables":[],"error":null,"duration":1.0,
            "profile":{"lines":[{"line":1,"seconds":0.002,"hits":1},
                {"line":2,"seconds":1.001,"hits":1},{"line":3,"seconds":0.0001,"hits":1}],
                "operations":[]}}"#,
    )
    .unwrap();
    state.handle_message(message);

    let lines: Vec<_> = state.editor.line_profile.keys().copied().collect();
    assert_eq!(lines, [2, 3, 4]);
    assert_eq!(state.editor.line_profile[&3].seconds, 1.001);
    let status = state.status.as_ref().unwrap();
    assert!(
        status.text.contains("slowest line is untitled:3 (1.0 s)"),
        "{}",
        status.text
    );

    state.execute(Action::ClearProfile);
    assert!(state.editor.line_profile.is_empty());
}

#[test]
fn a_pause_in_a_module_opens_its_tab_and_closing_it_ends_debugging() {
    let dir = std::env::temp_dir().join(format!("cq-editor-debug-modules-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let main = dir.join("main.py");
    let module = dir.join("helper.py");
    std::fs::write(&main, "import helper\nhelper.work()\n").unwrap();
    std::fs::write(&module, "def work():\n    return 4\n").unwrap();
    let mut state = test_state("");
    state.open_path(&main);
    let main_generation = state.document_generation;
    state.open_path(&module);
    let module_generation = state.document_generation;
    state.editor.breakpoints.insert(2);
    state.activate(main_generation);
    let (main_breaks, modules) = state.open_breakpoints(main_generation);
    assert!(main_breaks.is_empty());
    assert_eq!(modules[&module.display().to_string()], [2]);

    state.execution.started(17, true);
    state.run_script = Some((
        17,
        std::sync::Arc::new(crate::scene::RunScript {
            document: main_generation,
            text: state.document.text.clone(),
        }),
    ));
    let message: crate::worker::protocol::Message = serde_json::from_value(serde_json::json!({
        "type": "debug_paused", "id": 17, "line": 2,
        "path": module.display().to_string(), "variables": [], "objects": []
    }))
    .unwrap();
    state.handle_message(message);
    assert_eq!(state.document_generation, module_generation);
    assert_eq!(state.editor.debug_line, Some(2));
    state.close_now(module_generation);
    assert!(!state.is_debugging());
    assert_eq!(state.document_generation, main_generation);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn closing_the_paused_run_document_ends_the_session() {
    let mut state = test_state("a = 1\n");
    state.execution.started(19, true);
    state.run_script = Some((
        19,
        std::sync::Arc::new(crate::scene::RunScript {
            document: state.document_generation,
            text: state.document.text.clone(),
        }),
    ));
    assert!(!state.execution.paused(19));
    state.close_now(state.document_generation);
    assert!(!state.is_debugging());
}

#[test]
fn module_profile_lines_appear_in_the_module_gutter() {
    let dir =
        std::env::temp_dir().join(format!("cq-editor-profile-modules-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let main = dir.join("main.py");
    let module = dir.join("helper.py");
    std::fs::write(&main, "import helper\nhelper.work()\n").unwrap();
    std::fs::write(&module, "def work():\n    return 4\n").unwrap();
    let mut state = test_state("");
    state.open_path(&main);
    let main_generation = state.document_generation;
    state.open_path(&module);
    let module_generation = state.document_generation;
    state.activate(main_generation);
    state.execution.started(18, false);
    state.run_script = Some((
        18,
        std::sync::Arc::new(crate::scene::RunScript {
            document: main_generation,
            text: state.document.text.clone(),
        }),
    ));
    let message: crate::worker::protocol::Message = serde_json::from_value(serde_json::json!({
        "type": "run_finished", "id": 18, "ok": true, "debug": false, "stopped": false,
        "objects": [], "variables": [], "error": null, "duration": 0.1,
        "profile": {"lines": [{"file": module.display().to_string(), "line": 2,
                   "seconds": 0.1, "hits": 1}], "operations": [],
                   "sources": {}}
    }))
    .unwrap();
    state.handle_message(message);
    assert!(
        state
            .open_mut(module_generation)
            .unwrap()
            .1
            .line_profile
            .contains_key(&2)
    );
    state.execute(Action::ClearProfile);
    assert!(
        state
            .open_mut(module_generation)
            .unwrap()
            .1
            .line_profile
            .is_empty()
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn new_environments_go_to_a_free_folder() {
    let root = std::env::temp_dir().join(format!("cq-editor-free-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let preferred = root.join("cadquery");
    assert_eq!(unused_folder(&preferred), preferred, "missing: free");
    std::fs::create_dir_all(&preferred).unwrap();
    assert_eq!(unused_folder(&preferred), preferred, "empty: free");
    std::fs::write(preferred.join("pyvenv.cfg"), "").unwrap();
    assert_eq!(unused_folder(&preferred), root.join("cadquery-2"));
    std::fs::create_dir_all(root.join("cadquery-2/bin")).unwrap();
    assert_eq!(unused_folder(&preferred), root.join("cadquery-3"));
    let _ = std::fs::remove_dir_all(&root);

    let mut state = test_state("");
    state.execute(Action::InstallPackages);
    assert!(state.dialogs.environment.is_some());
}

fn updates_reply(id: u64, editor: &str) -> crate::worker::protocol::Message {
    serde_json::from_str(&format!(
        r#"{{"type":"updates","id":{id},"current":"2.8.0","newer":[],"editor":{editor}}}"#
    ))
    .unwrap()
}

#[test]
fn the_daily_check_only_speaks_when_a_newer_cq_editor_is_out() {
    let mut state = test_state("");
    let newer = r#"{"current":"0.8.0","newer":[{"tag":"v99.0.0","name":"99",
        "url":"https://github.com/igors93/CQ-editor/releases/tag/v99.0.0"}]}"#;

    state
        .execution
        .track(1, Pending::Updates { automatic: true });
    state.handle_message(updates_reply(1, r#"{"current":"0.8.0","newer":[]}"#));
    assert!(
        state.status.is_none() && state.dialogs.message.is_none(),
        "silent"
    );

    state
        .execution
        .track(2, Pending::Updates { automatic: true });
    state.handle_message(updates_reply(2, newer));
    assert!(state.dialogs.message.is_none());
    assert!(state.status.as_ref().unwrap().text.contains("v99.0.0"));
    assert_eq!(
        state.settings.general.newer_release.as_ref().unwrap().tag,
        "v99.0.0"
    );

    // A failed daily check is only logged.
    state
        .execution
        .track(3, Pending::Updates { automatic: true });
    state.handle_message(crate::worker::protocol::Message::Error {
        id: Some(3),
        message: "offline".into(),
    });
    assert!(state.dialogs.message.is_none());
}

#[test]
fn checking_by_hand_reports_cq_editor_and_cadquery() {
    let mut state = test_state("");
    state
        .execution
        .track(4, Pending::Updates { automatic: false });
    state.handle_message(updates_reply(4, r#"{"current":"0.8.0","newer":[]}"#));
    let message = state.dialogs.message.as_ref().unwrap();
    assert_eq!(message.title, "No updates available");
    assert!(message.text.contains("is the latest version"));
    assert!(message.text.contains("CadQuery 2.8.0"));
}

#[test]
fn console_results_from_the_worker_reach_the_console() {
    use crate::app::execution::Pending;
    use crate::worker::protocol::{DisplayImage, DisplayOutput, Message};

    let mut state = test_state("");
    state.execution.track(3, Pending::Console);
    state.handle_message(Message::Display(DisplayOutput {
        id: 3,
        text: "<Figure size 2x1>".into(),
        image: Some(DisplayImage {
            size: [2, 1],
            rgba: vec![255; 8],
        }),
    }));
    let entry = state.console.entries.last().unwrap();
    assert!(entry.image.is_some());
    assert_eq!(entry.text, "<Figure size 2x1>\n");

    state.handle_message(Message::Display(DisplayOutput {
        id: 4,
        text: "stale".into(),
        image: None,
    }));
    assert_eq!(state.console.entries.len(), 1, "stale output is ignored");
}

// -- the dock layout ---------------------------------------------------------

fn test_app() -> CqEditorApp {
    let mut dock = panels::default_layout();
    label_tab_menus(&mut dock);
    CqEditorApp {
        dock,
        maximized: None,
        state: test_state(""),
    }
}

fn panels_shown(app: &CqEditorApp) -> Vec<Tab> {
    let mut tabs: Vec<Tab> = app.dock.iter_all_tabs().map(|(_, tab)| *tab).collect();
    tabs.sort_by_key(|tab| Tab::ALL.iter().position(|t| t == tab));
    tabs
}

#[test]
fn a_maximized_panel_fills_the_window_until_it_is_restored() {
    let mut app = test_app();
    let layout = panels_shown(&app);
    assert_eq!(
        app.dock.translations.tab_context_menu.eject_button,
        "Float in a Window"
    );

    app.execute(Action::ToggleMaximize(Some(Tab::Viewer)));
    assert_eq!(panels_shown(&app), [Tab::Viewer]);
    assert!(
        app.state
            .status
            .as_ref()
            .unwrap()
            .text
            .contains("double-click")
    );
    app.execute(Action::ToggleMaximize(None));
    assert_eq!(panels_shown(&app), layout);

    // Going to a line needs the editor: the layout comes back.
    app.execute(Action::ToggleMaximize(Some(Tab::Objects)));
    app.execute(Action::GoToLine(1));
    assert_eq!(panels_shown(&app), layout);

    // So does showing or hiding a panel, and resetting the layout.
    app.execute(Action::ToggleMaximize(Some(Tab::Console)));
    app.execute(Action::TogglePanel(Tab::Log));
    assert!(app.maximized.is_none());
    assert!(!panels_shown(&app).contains(&Tab::Log));
    app.execute(Action::ToggleMaximize(Some(Tab::Console)));
    app.execute(Action::ResetLayout);
    assert_eq!(panels_shown(&app), layout);
}

#[derive(Default)]
struct MemoryStorage(std::collections::HashMap<String, String>);

impl eframe::Storage for MemoryStorage {
    fn get_string(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }

    fn set_string(&mut self, key: &str, value: String) {
        self.0.insert(key.to_owned(), value);
    }

    fn remove_string(&mut self, key: &str) {
        self.0.remove(key);
    }

    fn flush(&mut self) {}
}

#[test]
fn a_closed_editor_comes_back_whenever_a_file_or_a_line_is_shown() {
    let dir = std::env::temp_dir().join(format!("cq-editor-editor-panel-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.py"), "a = 1\n").unwrap();
    let mut app = test_app();
    app.state.recovery = RecoveryStore::new(dir.join(".recovery"));

    for action in [
        Action::OpenPath(dir.join("a.py")),
        Action::NewFile,
        Action::PreviousTab,
        Action::GoToLine(1),
    ] {
        app.execute(Action::TogglePanel(Tab::Editor));
        assert!(!panels_shown(&app).contains(&Tab::Editor));
        app.execute(action.clone());
        assert!(panels_shown(&app).contains(&Tab::Editor), "{action:?}");
    }
    // Next to the 3D view, not inside its panel.
    let editor = app.dock.find_tab(&Tab::Editor).unwrap();
    let viewer = app.dock.find_tab(&Tab::Viewer).unwrap();
    assert_ne!(editor.node_path(), viewer.node_path());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_layout_saved_without_the_editor_gets_it_back() {
    let without = |tabs: &[Tab]| {
        let mut dock = panels::default_layout();
        for tab in tabs {
            let path = dock.find_tab(tab).unwrap();
            dock.remove_tab(path);
        }
        dock
    };
    let restored = restored_layout(Some(without(&[Tab::Editor, Tab::Console])));
    assert!(restored.find_tab(&Tab::Editor).is_some());
    assert!(
        restored.find_tab(&Tab::Console).is_none(),
        "the rest of the layout is kept"
    );
    // Without the 3D view, or without a saved layout: the default one.
    for saved in [Some(without(&[Tab::Viewer])), None] {
        let restored = restored_layout(saved);
        assert!(restored.find_tab(&Tab::Viewer).is_some());
        assert!(restored.find_tab(&Tab::Console).is_some());
    }
}

#[test]
fn restored_dock_draws_the_editor_and_viewer_without_a_window() {
    struct Titles(Vec<Tab>);
    impl egui_dock::TabViewer for Titles {
        type Tab = Tab;

        fn id(&mut self, tab: &mut Tab) -> egui::Id {
            egui::Id::new(*tab)
        }

        fn title(&mut self, tab: &mut Tab) -> egui::WidgetText {
            self.0.push(*tab);
            tab.title().into()
        }

        fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Tab) {
            ui.label(tab.title());
        }
    }

    let mut saved = panels::default_layout();
    let editor = saved.find_tab(&Tab::Editor).unwrap();
    saved.remove_tab(editor);
    let mut dock = restored_layout(Some(saved));
    let ctx = egui::Context::default();
    let mut titles = Titles(Vec::new());
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        egui_dock::DockArea::new(&mut dock).show_inside(ui, &mut titles);
    });
    output.textures_delta.clear();
    assert!(titles.0.contains(&Tab::Editor));
    assert!(titles.0.contains(&Tab::Viewer));
    assert!(!output.shapes.is_empty());
}

#[test]
fn a_drawn_modal_blocks_global_shortcuts_behind_it() {
    let mut state = test_state("value = 1\n");
    state.dialogs.info(
        "File changed on disk",
        "Resolve this before closing the tab",
    );
    let ctx = state.ctx.clone();
    let press_close = || egui::RawInput {
        events: vec![egui::Event::Key {
            key: egui::Key::W,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        }],
        ..Default::default()
    };
    let mut output = ctx.run_ui(press_close(), |_| {
        state.begin_frame(&ctx);
        crate::dialogs::show(&ctx, &mut state);
    });
    output.textures_delta.clear();
    assert!(!output.shapes.is_empty(), "the modal is actually rendered");
    assert!(state.dialogs.modal_open());
    assert!(state.actions.is_empty());

    state.dialogs.message = None;
    let mut output = ctx.run_ui(press_close(), |_| state.begin_frame(&ctx));
    output.textures_delta.clear();
    assert_eq!(state.actions, [Action::CloseTab]);
}

#[test]
fn required_panels_return_to_their_workspace() {
    let mut app = test_app();
    for tab in [Tab::Console, Tab::Traceback, Tab::Project] {
        if let Some(path) = app.dock.find_tab(&tab) {
            app.dock.remove_tab(path);
        }
        app.show_panel(tab);
        assert!(app.dock.find_tab(&tab).is_some());
    }
    let console = app.dock.find_tab(&Tab::Console).unwrap();
    let traceback = app.dock.find_tab(&Tab::Traceback).unwrap();
    assert_eq!(console.node_path(), traceback.node_path());
    let project = app.dock.find_tab(&Tab::Project).unwrap();
    let editor = app.dock.find_tab(&Tab::Editor).unwrap();
    assert_ne!(project.node_path(), editor.node_path());

    app.execute(Action::TogglePanel(Tab::Console));
    app.execute(Action::ResetConsole);
    assert!(app.dock.find_tab(&Tab::Console).is_some());
}

#[test]
fn a_result_too_large_to_transfer_ends_the_run_and_restarts_python() {
    let mut state = test_state("");
    // No process is started in tests.
    state.python_override = Some("no-such-python-for-tests".into());
    state.execution.started(5, false);
    assert!(state.is_busy());

    state.worker_protocol_failed("Python sent a message too large to transfer".into());
    assert!(!state.is_busy(), "the run no longer waits for an answer");
    let message = state.dialogs.message.as_ref().expect("told why");
    assert_eq!(message.title, "Cannot show the result");
    assert!(message.text.contains("too large to transfer"));
    assert_ne!(state.worker.problem(), Some(worker::Problem::NoCadQuery));
}

#[test]
fn invalid_geometry_stops_export_for_a_user_decision() {
    use crate::worker::protocol::{GeometryReport, Message};
    let mut state = test_state("");
    state.execution.track(17, Pending::Export);
    state.handle_message(Message::GeometryInvalid {
        id: 17,
        reports: vec![GeometryReport {
            key: "n1".into(),
            name: "open shell".into(),
            valid: false,
            solids: 0,
            open_shells: 1,
            non_manifold_edges: 0,
            issues: vec!["1 open shell(s)".into()],
        }],
        interferences: vec![],
    });
    assert!(
        state
            .dialogs
            .geometry
            .as_ref()
            .is_some_and(|dialog| dialog.for_export)
    );
    assert!(!state.geometry_checks["n1"].valid);
    assert!(state.dialogs.modal_open());
}

#[test]
fn interference_check_keeps_both_parts_in_the_review() {
    use crate::worker::protocol::{InterferencePart, InterferenceReport, Message};
    let mut state = test_state("");
    state.execution.track(18, Pending::Check);
    state.handle_message(Message::GeometryChecked {
        id: 18,
        reports: vec![],
        interferences: vec![InterferenceReport {
            first: InterferencePart {
                key: "plate".into(),
                name: "plate".into(),
                part: 0,
                solid: 0,
            },
            second: InterferencePart {
                key: "pin".into(),
                name: "pin".into(),
                part: 0,
                solid: 0,
            },
            kind: "interference".into(),
            overlap_volume: Some(43.2),
            distance: Some(0.0),
            point: Some([0.0, 0.0, 0.0]),
            reason: None,
            allowance_reason: None,
        }],
    });
    let review = state.dialogs.geometry.as_ref().expect("review opened");
    assert_eq!(review.interferences.len(), 1);
    assert_eq!(review.interferences[0].second.name, "pin");
}

#[test]
fn overlap_mesh_reply_only_applies_to_the_pair_still_selected() {
    use crate::worker::protocol::{InterferencePart, InterferenceReport, Mesh, Message};
    let mut state = test_state("");
    let first = InterferencePart {
        key: "plate".into(),
        name: "plate".into(),
        part: 0,
        solid: 0,
    };
    let second = InterferencePart {
        key: "pin".into(),
        name: "pin".into(),
        part: 0,
        solid: 0,
    };
    state.execution.track(20, Pending::Check);
    state.handle_message(Message::GeometryChecked {
        id: 20,
        reports: vec![],
        interferences: vec![InterferenceReport {
            first: first.clone(),
            second: second.clone(),
            kind: "interference".into(),
            overlap_volume: Some(1.0),
            distance: Some(0.0),
            point: Some([0.0, 0.0, 0.0]),
            reason: None,
            allowance_reason: None,
        }],
    });
    let review = state.dialogs.geometry.as_mut().unwrap();
    let mut other_pair = review.interferences[0].clone();
    other_pair.second.key = "other".into();
    other_pair.second.name = "other".into();
    review.interferences.push(other_pair);
    review.selected_pair = Some(1);
    state.execution.track(
        21,
        Pending::InterferenceGeometry {
            first: first.clone(),
            second: second.clone(),
        },
    );
    state.handle_message(Message::InterferenceGeometry {
        id: 21,
        mesh: Box::<Mesh>::default(),
    });
    assert!(state.scene.overlay.meshes.is_empty());

    state.dialogs.geometry.as_mut().unwrap().selected_pair = Some(0);
    state
        .execution
        .track(22, Pending::InterferenceGeometry { first, second });
    state.handle_message(Message::InterferenceGeometry {
        id: 22,
        mesh: Box::<Mesh>::default(),
    });
    assert_eq!(state.scene.overlay.meshes.len(), 1);
}

#[test]
fn the_empty_view_says_when_the_editor_is_closed() {
    let mut state = test_state("");
    assert_eq!(state.empty_view_hint(), "Press F5 to render the script");
    state.editor_panel_open = false;
    assert!(
        state
            .empty_view_hint()
            .ends_with("the editor is closed: View > Editor")
    );
}

#[test]
fn the_whole_layout_is_saved_while_a_panel_is_maximized() {
    let mut app = test_app();
    app.execute(Action::ToggleMaximize(Some(Tab::Editor)));
    let mut storage = MemoryStorage::default();
    eframe::App::save(&mut app, &mut storage);
    let saved: DockState<Tab> = eframe::get_value(&storage, LAYOUT_KEY).unwrap();
    assert!(saved.find_tab(&Tab::Viewer).is_some());
    assert!(saved.find_tab(&Tab::Console).is_some());
}

#[test]
fn modified_parameters_can_be_restored_to_their_initial_values_and_undone() {
    let initial = "width = 40\nheight = 20.5\n";
    let mut state = test_state(initial);
    state.execute(Action::SetParameter {
        name: "width".into(),
        line: 1,
        value: 65.0,
    });
    state.execute(Action::SetParameter {
        name: "height".into(),
        line: 2,
        value: 30.0,
    });
    assert_eq!(state.document.text, "width = 65\nheight = 30.0\n");

    state.execute(Action::ResetParameters);
    assert_eq!(state.document.text, initial);

    state.execute(Action::Undo);
    assert_eq!(state.document.text, "width = 65\nheight = 30.0\n");
}
