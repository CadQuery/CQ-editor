use super::*;

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

#[test]
fn decodes_meshes() {
    let positions: Vec<u8> = [0.0f32, 1.0, 2.0, 3.0, 4.0, 5.0]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    let indices: Vec<u8> = [0u32, 1, 1].iter().flat_map(|v| v.to_le_bytes()).collect();
    let json = format!(
        r#"{{"type":"inspection_geometry","id":3,"plane":null,"meshes":[{{
            "positions":"{p}","normals":"{p}","indices":"{i}","edges":"","points":"",
            "topology":{{"face_positions":"{p}","face_indices":"{i}",
                "triangle_faces":"{f}","segment_edges":"","vertices":""}},
            "bbox":[[0,1,2],[3,4,5]]}}]}}"#,
        p = b64(&positions),
        i = b64(&indices),
        f = b64(&1u32.to_le_bytes()),
    );
    let Message::InspectionGeometry { id, meshes, plane } = serde_json::from_str(&json).unwrap()
    else {
        panic!("wrong message type");
    };
    assert_eq!(id, 3);
    assert!(plane.is_none());
    assert_eq!(meshes[0].positions, vec![[0.0, 1.0, 2.0], [3.0, 4.0, 5.0]]);
    assert_eq!(meshes[0].indices, vec![0, 1, 1]);
    assert!(meshes[0].edges.is_empty());
    assert_eq!(meshes[0].topology.triangle_faces, vec![1]);
}

#[test]
fn serializes_requests_with_command_tags() {
    let request = Request::Debug {
        action: DebugAction::StepIn,
        breakpoints: vec![3],
        module_breakpoints: [("/project/helper.py".into(), vec![7])].into(),
    };
    assert_eq!(
        serde_json::to_string(&request).unwrap(),
        r#"{"cmd":"debug","action":"step_in","breakpoints":[3],"module_breakpoints":{"/project/helper.py":[7]}}"#
    );
    assert_eq!(
        serde_json::to_string(&Request::Shutdown).unwrap(),
        r#"{"cmd":"shutdown"}"#
    );
}

#[test]
fn hover_documentation_travels_as_describe_and_description() {
    let request = Request::Describe {
        id: 4,
        source: "len(x)".into(),
        line: 1,
        column: 0,
        path: None,
    };
    assert_eq!(
        serde_json::to_string(&request).unwrap(),
        r#"{"cmd":"describe","id":4,"source":"len(x)","line":1,"column":0,"path":null}"#
    );

    let message: Message = serde_json::from_str(
        r#"{"type":"description","id":4,"available":true,
            "signatures":["len(obj: Sized, /) -> int"],"text":"Return the length."}"#,
    )
    .unwrap();
    let Message::Description { id, documentation } = message else {
        panic!("not a description: {message:?}");
    };
    assert_eq!(id, 4);
    assert_eq!(documentation.signatures, ["len(obj: Sized, /) -> int"]);
    assert_eq!(documentation.text, "Return the length.");

    // Nothing to say about the name.
    let empty: Message =
        serde_json::from_str(r#"{"type":"description","id":5,"available":false}"#).unwrap();
    assert!(matches!(
        empty,
        Message::Description { id: 5, documentation } if documentation == Documentation::default()
    ));
}

#[test]
fn source_navigation_uses_current_text_and_project_locations() {
    let request = serde_json::to_value(Request::Navigate {
        id: 7,
        source: "value = make_part()".into(),
        line: 1,
        column: 14,
        path: Some("/project/main.py".into()),
        project_root: Some("/project".into()),
        mode: NavigationMode::Definition,
    })
    .unwrap();
    assert_eq!(request["cmd"], "navigate");
    assert_eq!(request["mode"], "definition");
    assert_eq!(request["source"], "value = make_part()");
    assert_eq!(request["project_root"], "/project");

    let message: Message = serde_json::from_value(serde_json::json!({
        "type": "navigation", "id": 7, "mode": "definition", "available": true,
        "locations": [{"name": "make_part", "kind": "function", "path": "/project/helper.py",
            "line": 2, "column": 4}], "truncated": false
    }))
    .unwrap();
    let Message::Navigation {
        locations, mode, ..
    } = message
    else {
        panic!("expected navigation response");
    };
    assert_eq!(mode, NavigationMode::Definition);
    assert_eq!(locations[0].line, 2);
    assert_eq!(locations[0].path.as_deref(), Some("/project/helper.py"));
}

#[test]
fn measurement_request_and_exact_response_share_topology_references() {
    let request = Request::Measure {
        id: 12,
        selections: vec![TopologyReference {
            key: "s1-n2".into(),
            part: 0,
            kind: TopologyKind::Vertex,
            identifier: 3,
        }],
        objects: Vec::new(),
    };
    let json = serde_json::to_value(request).unwrap();
    assert_eq!(json["cmd"], "measure");
    assert_eq!(json["selections"][0]["kind"], "vertex");
    assert_eq!(json["selections"][0]["identifier"], 3);

    let message: Message = serde_json::from_value(serde_json::json!({
        "type": "measurement",
        "id": 12,
        "identity": {"scope": "scene_entry", "persistent": false},
        "entities": [{
            "key": "s1-n2", "part": 0, "kind": "vertex", "identifier": 3,
            "geometry_type": "vertex", "center": [3.0, 4.0, 0.0],
            "position": [3.0, 4.0, 0.0], "length": null, "area": null,
            "radius": null, "diameter": null
        }],
        "pair": null
    }))
    .unwrap();
    let Message::Measurement(measurement) = message else {
        panic!("expected measurement");
    };
    assert_eq!(measurement.entities[0].position, Some([3.0, 4.0, 0.0]));
    assert!(!measurement.identity.persistent);
}

#[test]
fn selector_preview_transfers_exact_topology_ids() {
    let request = serde_json::to_value(Request::SelectorPreview {
        id: 9,
        run_id: 4,
        line: 7,
        column: 18,
        preferred_key: Some("n1".into()),
    })
    .unwrap();
    assert_eq!(request["cmd"], "selector_preview");
    assert_eq!(request["run_id"], 4);
    let message: Message = serde_json::from_value(serde_json::json!({
        "type": "selector_preview", "id": 9, "active": true, "count": 1,
        "matches": [{"key":"n1","part":0,"kind":"face","identifier":6}],
        "message": null
    }))
    .unwrap();
    let Message::SelectorPreview { matches, .. } = message else {
        panic!("expected selector preview");
    };
    assert_eq!(matches[0].identifier, 6);
}

#[test]
fn parses_errors_and_output() {
    let message: Message =
        serde_json::from_str(r#"{"type":"output","id":null,"stream":"stderr","text":"x"}"#)
            .unwrap();
    assert!(matches!(
        message,
        Message::Output {
            id: None,
            stream: OutputStream::Stderr,
            ..
        }
    ));
}

#[test]
fn overlap_geometry_request_and_mesh_reply_round_trip() {
    use crate::worker::protocol::InterferencePart;
    let first = InterferencePart {
        key: "n1".into(),
        name: "plate".into(),
        part: 0,
        solid: 0,
    };
    let request = Request::InterferenceGeometry {
        id: 8,
        first: first.clone(),
        second: InterferencePart {
            key: "n2".into(),
            name: "pin".into(),
            part: 0,
            solid: 0,
        },
        scene: SceneSettings {
            linear_tolerance: 0.001,
            angular_tolerance: 0.2,
            merge_assemblies: false,
            units: "millimeter".into(),
        },
    };
    let json = serde_json::to_value(request).unwrap();
    assert_eq!(json["cmd"], "interference_geometry");
    assert_eq!(json["first"]["name"], "plate");
    let reply: Message = serde_json::from_value(serde_json::json!({
        "type": "interference_geometry", "id": 8, "mesh": {}
    }))
    .unwrap();
    assert!(matches!(reply, Message::InterferenceGeometry { id: 8, .. }));
}

#[test]
fn serializes_run_request_with_worker_settings() {
    let request = Request::Run {
        id: 9,
        script: "show_object(cq.Workplane())".into(),
        path: None,
        debug: false,
        breakpoints: vec![4],
        project_root: Some("/project".into()),
        module_breakpoints: [("/project/helper.py".into(), vec![7])].into(),
        settings: RunSettings {
            reload_cq: false,
            add_script_dir_to_path: true,
            change_working_dir: true,
            reload_imported_modules: true,
        },
        scene: SceneSettings {
            linear_tolerance: 0.001,
            angular_tolerance: 0.2,
            merge_assemblies: false,
            units: "millimeter".into(),
        },
        known_geometry: vec!["abc".into()],
        profile: true,
    };
    let json = serde_json::to_value(request).unwrap();
    assert_eq!(json["cmd"], "run");
    assert_eq!(json["id"], 9);
    assert_eq!(json["path"], serde_json::Value::Null);
    assert_eq!(json["project_root"], "/project");
    assert_eq!(
        json["module_breakpoints"]["/project/helper.py"],
        serde_json::json!([7])
    );
    assert_eq!(json["breakpoints"], serde_json::json!([4]));
    assert_eq!(json["settings"]["add_script_dir_to_path"], true);
    assert_eq!(json["scene"]["linear_tolerance"], 0.001);
    assert_eq!(json["known_geometry"], serde_json::json!(["abc"]));
    assert_eq!(json["profile"], true);
}

#[test]
fn parses_worker_readiness_and_optional_fatal_detail() {
    let ready: Message = serde_json::from_value(serde_json::json!({
        "type": "ready", "protocol": PROTOCOL_VERSION,
        "python": "3.12", "cadquery": "2.8", "ocp": "7.9.3.1.1",
        "executable": "python",
        "completion": true, "pid": 123
    }))
    .unwrap();
    let Message::Ready(info) = ready else {
        panic!("expected ready");
    };
    assert_eq!(info.protocol, PROTOCOL_VERSION);
    assert_eq!(info.ocp, "7.9.3.1.1");
    assert!(info.completion);

    let old_ready: Message = serde_json::from_value(serde_json::json!({
        "type": "ready", "protocol": PROTOCOL_VERSION,
        "python": "3.10", "cadquery": "2.7", "executable": "python",
        "completion": false
    }))
    .unwrap();
    let Message::Ready(old_info) = old_ready else {
        panic!("expected ready");
    };
    assert_eq!(old_info.ocp, "unknown");

    let fatal: Message =
        serde_json::from_str(r#"{"type":"fatal","message":"no cadquery"}"#).unwrap();
    assert!(matches!(fatal, Message::Fatal { detail: None, .. }));
}

#[test]
fn rejects_corrupt_base64_mesh_data() {
    let base = serde_json::json!({
        "positions": "!!!", "normals": "", "indices": "",
        "edges": "", "points": "",
        "topology": {
            "face_positions": "", "face_indices": "", "triangle_faces": "",
            "segment_edges": "", "vertices": ""
        },
        "bbox": [[0,0,0],[1,1,1]]
    });
    assert!(serde_json::from_value::<Mesh>(base.clone()).is_err());

    let mut short_vector = base.clone();
    short_vector["positions"] = b64(&[0; 8]).into();
    assert!(serde_json::from_value::<Mesh>(short_vector).is_err());

    let mut short_index = base;
    short_index["positions"] = "".into();
    short_index["indices"] = b64(&[0; 3]).into();
    assert!(serde_json::from_value::<Mesh>(short_index).is_err());

    let mut short_topology = serde_json::json!({
        "positions": "", "normals": "", "indices": "", "edges": "", "points": "",
        "topology": {
            "face_positions": b64(&[0; 8]), "face_indices": "", "triangle_faces": "",
            "segment_edges": "", "vertices": ""
        },
        "bbox": [[0,0,0],[1,1,1]]
    });
    assert!(serde_json::from_value::<Mesh>(short_topology.clone()).is_err());
    short_topology["topology"]["face_positions"] = "".into();
    short_topology["topology"]["triangle_faces"] = b64(&[0; 3]).into();
    assert!(serde_json::from_value::<Mesh>(short_topology).is_err());
}

#[test]
fn export_formats_have_distinct_extensions_and_wire_values() {
    let expected = [
        (ExportFormat::Step, "step", "STEP"),
        (ExportFormat::Stl, "stl", "STL"),
        (ExportFormat::Brep, "brep", "BREP"),
        (ExportFormat::ThreeMf, "3mf", "3MF"),
        (ExportFormat::Gltf, "gltf", "glTF"),
        (ExportFormat::Glb, "glb", "GLB"),
        (ExportFormat::Amf, "amf", "AMF"),
        (ExportFormat::Vrml, "vrml", "VRML"),
        (ExportFormat::Vtkjs, "vtkjs", "VTK.js"),
        (ExportFormat::Xbf, "xbf", "XBF"),
        (ExportFormat::Svg, "svg", "SVG"),
        (ExportFormat::Dxf, "dxf", "DXF"),
    ];
    for (format, extension, label) in expected {
        assert_eq!(format.extension(), extension);
        assert_eq!(format.label(), label);
        assert_eq!(
            serde_json::to_string(&format).unwrap(),
            format!("\"{extension}\"")
        );
    }
}

#[test]
fn selector_requests_and_replies_round_trip() {
    let request = Request::Selector {
        id: 3,
        selections: vec![TopologyReference {
            key: "s1-n2".into(),
            part: 0,
            kind: TopologyKind::Edge,
            identifier: 7,
        }],
    };
    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["cmd"], "selector");
    assert_eq!(json["selections"][0]["kind"], "edge");
    assert_eq!(json["selections"][0]["identifier"], 7);

    let reply = r#"{"type":"selector","id":3,"method":"edges","candidates":[
        {"expression":".edges(\"|Z and >Y\")","selector":"|Z and >Y",
         "description":"parallel to Z, largest Y","index_based":false},
        {"expression":".edges()[6]","selector":null,
         "description":"by position","index_based":true}]}"#;
    let Ok(Message::Selector(reply)) = serde_json::from_str::<Message>(reply) else {
        panic!("not a selector reply");
    };
    assert_eq!(reply.method, "edges");
    assert_eq!(reply.candidates[0].expression, r#".edges("|Z and >Y")"#);
    assert!(reply.candidates[1].index_based);
}

#[test]
fn console_commands_carry_the_selection_for_selected() {
    let request = Request::Console {
        id: 5,
        source: "selected()".into(),
        scene: SceneSettings {
            linear_tolerance: 0.001,
            angular_tolerance: 0.2,
            merge_assemblies: false,
            units: "millimeter".into(),
        },
        known_geometry: Vec::new(),
        selection: ConsoleSelection {
            entities: vec![TopologyReference {
                key: "s1-n1".into(),
                part: 0,
                kind: TopologyKind::Face,
                identifier: 6,
            }],
            objects: vec!["s1-n1".into()],
        },
    };
    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["cmd"], "console");
    assert_eq!(json["selection"]["entities"][0]["kind"], "face");
    assert_eq!(json["selection"]["objects"][0], "s1-n1");
}

#[test]
fn scene_additions_name_their_request() {
    let added = |text| match serde_json::from_str::<Message>(text) {
        Ok(Message::SceneAdd { id, objects }) => (id, objects.len()),
        other => panic!("not a scene addition: {other:?}"),
    };
    assert_eq!(
        added(r#"{"type":"scene_add","id":5,"objects":[]}"#),
        (Some(5), 0)
    );
    assert_eq!(added(r#"{"type":"scene_add","objects":[]}"#), (None, 0));
}

fn run_finished(extra: &str) -> RunFinished {
    let text = format!(
        r#"{{"type":"run_finished","id":4,"ok":true,"debug":false,"stopped":false,
            "objects":[],"variables":[],"error":null,"duration":1.5{extra}}}"#
    );
    match serde_json::from_str::<Message>(&text) {
        Ok(Message::RunFinished(result)) => *result,
        other => panic!("not a run result: {other:?}"),
    }
}

#[test]
fn cad_advice_is_optional_and_keeps_the_original_traceback() {
    let error: ScriptError = serde_json::from_value(serde_json::json!({
        "type_name": "StdFail_NotDone", "message": "BRep_API: command not done",
        "frames": [], "line": 2, "text": "original traceback",
        "diagnostic": {"operation":"fillet", "receiver":"part", "line":2,
            "hints":["Try a smaller radius"]}
    }))
    .unwrap();
    assert_eq!(error.text, "original traceback");
    assert_eq!(error.diagnostic.unwrap().operation, "fillet");
    let old: ScriptError = serde_json::from_value(serde_json::json!({
        "type_name": "ValueError", "message": "x", "frames": [],
        "line": null, "text": "old traceback"
    }))
    .unwrap();
    assert!(old.diagnostic.is_none());
}

#[test]
fn run_results_carry_script_requirements() {
    let result = run_finished(
        r#","checks":[{"name":"wall","kind":"volume","ok":false,
            "detail":"Volume 4; expected [5, 6]","line":12}]"#,
    );
    assert_eq!(result.checks.len(), 1);
    assert_eq!(result.checks[0].line, Some(12));
    assert!(!result.checks[0].ok);
    assert!(run_finished("").checks.is_empty());
}

#[test]
fn run_results_carry_stage_timings() {
    let result = run_finished(
        r#","timings":{"script":1.5,"identify":0.01,"tessellate":0.25,"encode":0.02,
            "meshed":1,"cached":2,"referenced":3,"mesh_bytes":2500000,"sent_at":1000.0}"#,
    );
    let timings = result.timings.unwrap();
    assert_eq!(
        (timings.meshed, timings.cached, timings.referenced),
        (1, 2, 3)
    );

    let receipt = Receipt {
        received_at: Some(UNIX_EPOCH + Duration::from_millis(1_000_040)),
        parse: Duration::from_millis(6),
    };
    let transfer = timings.transfer(&receipt).unwrap();
    assert!((transfer.as_secs_f64() - 0.04).abs() < 1e-6);

    let report = timings.report(&receipt, Duration::from_millis(2));
    for part in [
        "script 1.500",
        "meshing 0.250",
        "encoding 0.020",
        "transfer 0.040",
        "parsing 0.006",
        "scene 0.002",
        "1 meshed, 2 cached, 3 reused by reference, 2.50 MB sent",
    ] {
        assert!(report.contains(part), "{part:?} missing from {report:?}");
    }
}

#[test]
fn transfer_time_is_unknown_when_the_clocks_disagree() {
    let timings = RunTimings {
        sent_at: 2000.0,
        ..RunTimings::default()
    };
    let early = Receipt {
        received_at: Some(UNIX_EPOCH + Duration::from_secs(1000)),
        parse: Duration::ZERO,
    };
    assert_eq!(timings.transfer(&early), None);
    assert_eq!(timings.transfer(&Receipt::default()), None);
    assert!(
        timings
            .report(&early, Duration::ZERO)
            .contains("transfer ?")
    );
}

#[test]
fn run_results_without_timings_still_decode() {
    assert_eq!(run_finished("").timings, None);
}

#[test]
fn unknown_worker_message_is_rejected() {
    assert!(serde_json::from_str::<Message>(r#"{"type":"unknown"}"#).is_err());
}

#[test]
fn nodes_and_stack_frames_carry_their_script_lines() {
    let node: SceneNode = serde_json::from_str(
        r#"{"key":"n1","name":"plate","kind":"Workplane","inspectable":true,"line":4,
            "color":null,"alpha":null,"edge_color":null,"parts":[],"children":[]}"#,
    )
    .unwrap();
    assert_eq!(node.line, Some(4));
    let older: SceneNode = serde_json::from_str(
        r#"{"key":"n1","name":"plate","kind":"Workplane","inspectable":true,
            "color":null,"alpha":null,"edge_color":null,"parts":[],"children":[]}"#,
    )
    .unwrap();
    assert_eq!(older.line, None);

    let frame: StackFrame = serde_json::from_str(
        r#"{"type":"Workplane","value":"origin (0, 0, 0)","line":2,
            "items":[{"type":"Solid","value":"1 solid(s)","displayable":true,"line":2}]}"#,
    )
    .unwrap();
    assert_eq!((frame.line, frame.items[0].line), (Some(2), Some(2)));
}

#[test]
fn object_measurements_bounding_boxes_and_measurement_code_decode() {
    let message: Message = serde_json::from_value(serde_json::json!({
        "type": "measurement", "id": 5,
        "identity": {"scope": "scene_entry", "persistent": false},
        "entities": [], "pair": null,
        "objects": [{
            "key": "s1-n1", "shape_type": "solid", "volume": 6.0, "area": 22.0,
            "length": null, "center": [0.0, 0.0, 0.0],
            "bounding_box": {"min": [-0.5, -1.0, -1.5], "max": [0.5, 1.0, 1.5],
                             "size": [1.0, 2.0, 3.0], "center": [0.0, 0.0, 0.0]},
            "solids": 1, "faces": 6, "edges": 12, "vertices": 8
        }],
        "bounding_box": {"min": [-0.5, -1.0, -1.5], "max": [0.5, 1.0, 1.5],
                         "size": [1.0, 2.0, 3.0], "center": [0.0, 0.0, 0.0]}
    }))
    .unwrap();
    let Message::Measurement(result) = message else {
        panic!("not a measurement");
    };
    assert_eq!(result.objects[0].volume, Some(6.0));
    assert_eq!(result.objects[0].faces, 6);
    assert_eq!(result.bounding_box.unwrap().size, [1.0, 2.0, 3.0]);

    let request = Request::MeasurementCode {
        id: 6,
        selections: Vec::new(),
        objects: vec!["s1-n1".into()],
    };
    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["cmd"], "measurement_code");
    assert_eq!(json["objects"][0], "s1-n1");

    let message: Message = serde_json::from_str(
        r#"{"type":"measurement_code","id":6,"code":"x = 1\n","verified":false,
            "problem":"The Workplane is not a variable of the script"}"#,
    )
    .unwrap();
    let Message::MeasurementCode {
        verified, problem, ..
    } = message
    else {
        panic!("not measurement code");
    };
    assert!(!verified);
    assert!(problem.unwrap().contains("variable"));
}

#[test]
fn meshing_progress_and_cancellation_cross_the_protocol() {
    let message: Message =
        serde_json::from_str(r#"{"type":"progress","id":7,"stage":"meshing","done":3,"total":40}"#)
            .unwrap();
    let Message::Progress {
        id,
        stage,
        done,
        total,
    } = message
    else {
        panic!("not progress: {message:?}");
    };
    assert_eq!((id, stage.as_str(), done, total), (7, "meshing", 3, 40));

    assert_eq!(
        serde_json::to_value(Request::Cancel { target: 7 }).unwrap(),
        serde_json::json!({"cmd": "cancel", "target": 7})
    );
    assert!(run_finished(r#","cancelled":true"#).cancelled);
    assert!(!run_finished("").cancelled, "older workers never cancel");
}

#[test]
fn stop_interrupts_cross_the_protocol() {
    assert_eq!(
        serde_json::to_value(Request::Interrupt { target: 4 }).unwrap(),
        serde_json::json!({"cmd": "interrupt", "target": 4})
    );
    assert!(run_finished(r#","interrupted":true"#).interrupted);
    assert!(
        !run_finished("").interrupted,
        "older workers never interrupt"
    );
}

#[test]
fn the_worker_declares_the_same_protocol_version() {
    let worker = include_str!("../../../worker/cq_worker/protocol.py");
    let declared = worker
        .lines()
        .find_map(|line| line.strip_prefix("PROTOCOL_VERSION = "))
        .expect("protocol.py declares PROTOCOL_VERSION");
    assert_eq!(declared.trim().parse::<u32>().unwrap(), PROTOCOL_VERSION);
}

#[test]
fn profiles_cross_the_protocol() {
    let result = run_finished(
        r#","profile":{"lines":[{"line":2,"seconds":1.5,"hits":3}],
            "operations":[{"line":2,"name":"Workplane.fillet","seconds":1.2,"calls":3}]}"#,
    );
    let profile = result.profile.unwrap();
    assert_eq!(
        profile.lines,
        [LineTime {
            file: None,
            line: 2,
            seconds: 1.5,
            hits: 3
        }]
    );
    assert_eq!(profile.operations[0].name, "Workplane.fillet");
    assert!(run_finished("").profile.is_none());
}

#[test]
fn update_checks_name_the_running_cq_editor() {
    assert_eq!(
        serde_json::to_value(Request::CheckUpdates {
            id: 3,
            editor: "0.8.0".into(),
            cadquery: false,
        })
        .unwrap(),
        serde_json::json!({"cmd": "check_updates", "id": 3, "editor": "0.8.0", "cadquery": false})
    );
    // An older worker answers without CQ-editor's releases.
    let reply: Message =
        serde_json::from_str(r#"{"type":"updates","id":3,"current":"2.8","newer":[]}"#).unwrap();
    assert!(matches!(reply, Message::Updates { editor: None, .. }));
}

fn png(width: u32, height: u32) -> String {
    let pixels =
        image::RgbaImage::from_fn(width, height, |x, _| image::Rgba([x as u8, 128, 255, 255]));
    let mut bytes = std::io::Cursor::new(Vec::new());
    pixels
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    b64(&bytes.into_inner())
}

fn jpeg(width: u32, height: u32) -> String {
    let pixels = image::RgbImage::from_pixel(width, height, image::Rgb([32, 64, 128]));
    let mut bytes = std::io::Cursor::new(Vec::new());
    pixels
        .write_to(&mut bytes, image::ImageFormat::Jpeg)
        .unwrap();
    b64(&bytes.into_inner())
}

fn display(image: &str) -> DisplayOutput {
    let line = format!(r#"{{"type":"display","id":4,"text":"figure","image":{image}}}"#);
    match serde_json::from_str(&line).unwrap() {
        Message::Display(output) => output,
        other => panic!("not a display: {other:?}"),
    }
}

#[test]
fn console_images_are_decoded_when_they_arrive() {
    let output = display(&format!(r#"{{"format":"png","data":"{}"}}"#, png(2, 1)));
    assert_eq!(output.id, 4);
    assert_eq!(output.text, "figure");
    let image = output.image.unwrap();
    assert_eq!(image.size, [2, 1]);
    assert_eq!(image.rgba, [0, 128, 255, 255, 1, 128, 255, 255]);

    let text_only = display("null");
    assert!(text_only.image.is_none());
    assert_eq!(text_only.text, "figure");

    let jpeg = display(&format!(r#"{{"format":"jpeg","data":"{}"}}"#, jpeg(3, 2)));
    assert_eq!(jpeg.image.unwrap().size, [3, 2]);

    let missing_id = r#"{"type":"display","text":"orphan","image":null}"#;
    assert!(serde_json::from_str::<Message>(missing_id).is_err());
}

#[test]
fn console_images_that_cannot_be_trusted_become_a_note() {
    let cases = [
        (
            r#"{"format":"svg","data":"PHN2Zy8+"}"#.to_owned(),
            "svg is not shown",
        ),
        (
            r#"{"format":"png","data":"not base64!"}"#.to_owned(),
            "bad data",
        ),
        (
            format!(r#"{{"format":"jpeg","data":"{}"}}"#, png(1, 1)),
            "image not shown",
        ),
        (
            format!(
                r#"{{"format":"png","data":"{}"}}"#,
                png(MAX_DISPLAY_SIDE + 1, 1)
            ),
            "image not shown",
        ),
        (
            format!(
                r#"{{"format":"png","data":"{}"}}"#,
                b64(&vec![0; MAX_DISPLAY_IMAGE_BYTES + 1])
            ),
            "too large",
        ),
    ];
    for (image, note) in cases {
        let output = display(&image);
        assert!(output.image.is_none(), "{image:.60}");
        assert!(output.text.starts_with("figure\n[image not shown: "));
        assert!(output.text.contains(note), "{}", output.text);
    }

    let without_text =
        r#"{"type":"display","id":4,"text":"","image":{"format":"svg","data":"PHN2Zy8+"}}"#;
    let Message::Display(output) = serde_json::from_str(without_text).unwrap() else {
        panic!("not a display message");
    };
    assert_eq!(output.text, "[image not shown: svg is not shown]");
}
