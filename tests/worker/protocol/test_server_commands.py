"""Less common request sequences that must remain safe for the GUI."""

from cq_worker import network

BOX = "part = cq.Workplane().box(1, 2, 3)\nshow_object(part)"


def test_editor_navigation_requests_use_unsaved_source(conversation, tmp_path):
    source = "def make_part():\n    return 1\nvalue = make_part()\n"
    path = tmp_path / "main.py"
    talk = conversation(
        {"cmd": "outline", "id": 1, "source": source},
        {
            "cmd": "navigate",
            "id": 2,
            "source": source,
            "path": str(path),
            "project_root": str(tmp_path),
            "line": 3,
            "column": 13,
            "mode": "definition",
        },
    )
    talk.serve()
    assert [symbol["name"] for symbol in talk.of_type("outline")[0]["symbols"]] == [
        "make_part",
        "value",
    ]
    result = talk.of_type("navigation")[0]
    assert result["available"] and result["mode"] == "definition"
    assert result["locations"][0]["path"] is None
    assert result["locations"][0]["line"] == 1


def test_interference_geometry_returns_shared_volume_mesh(conversation):
    first = {"key": "n1", "name": "first", "part": 0, "solid": 0}
    second = {"key": "n2", "name": "second", "part": 0, "solid": 0}
    talk = conversation(
        {
            "cmd": "run",
            "id": 1,
            "script": (
                "show_object(cq.Workplane().box(2, 2, 2), name='first')\n"
                "show_object(cq.Workplane().box(2, 2, 2).translate((1, 0, 0)), "
                "name='second')"
            ),
        },
        {"cmd": "interference_geometry", "id": 2, "first": first, "second": second},
        {
            "cmd": "interference_geometry",
            "id": 3,
            "first": {**first, "solid": 99},
            "second": second,
        },
    )
    talk.serve()
    (reply,) = talk.of_type("interference_geometry")
    assert reply["id"] == 2
    assert reply["mesh"]["positions"]
    assert talk.of_type("error")[0]["id"] == 3


def test_check_and_export_share_the_geometry_report(conversation, tmp_path):
    script = (
        "box = cq.Workplane().box(1, 1, 1).val()\n"
        "open_shell = cq.Shell.makeShell(box.Faces()[:-1])\n"
        "show_object(open_shell)"
    )
    path = tmp_path / "shell.step"
    talk = conversation(
        {"cmd": "run", "id": 1, "script": script},
        {"cmd": "check", "id": 2, "keys": ["n1"]},
        {"cmd": "export", "id": 3, "keys": ["n1"], "format": "step", "path": str(path)},
        {
            "cmd": "export",
            "id": 4,
            "keys": ["n1"],
            "format": "step",
            "path": str(path),
            "on_invalid": "ignore",
        },
    )
    talk.serve()
    report = talk.of_type("geometry_checked")[0]["reports"][0]
    invalid = talk.of_type("geometry_invalid")[0]["reports"][0]
    assert report["name"] == invalid["name"]
    assert report["open_shells"] == invalid["open_shells"] == 1
    assert talk.of_type("export_finished")[0]["id"] == 4
    assert path.stat().st_size > 100


def test_successful_run_then_failed_run_retains_previous_scene(conversation):
    talk = conversation(
        {"cmd": "run", "id": 1, "script": BOX},
        {"cmd": "run", "id": 2, "script": "raise ValueError('bad')"},
    )
    talk.serve()
    first, second = talk.of_type("run_finished")
    assert first["ok"] and len(first["objects"]) == 1
    assert not second["ok"] and second["objects"] is None
    assert second["variables"] is None


def test_release_removes_objects_from_inspection_registry(conversation):
    talk = conversation(
        {"cmd": "run", "id": 1, "script": BOX},
        {"cmd": "inspect", "id": 2, "key": "n1"},
        {"cmd": "release", "id": 3, "keys": ["n1"]},
        {"cmd": "inspect", "id": 4, "key": "n1"},
        {"cmd": "inspect_geometry", "id": 5, "key": "n1", "frame": 0},
    )
    talk.serve()
    assert talk.of_type("inspection")[0]["id"] == 2
    assert [error["id"] for error in talk.of_type("error")] == [4, 5]


def test_measurement_uses_live_occt_vertices_and_rejects_stale_ids(conversation):
    talk = conversation(
        {
            "cmd": "run",
            "id": 1,
            "script": (
                "first = cq.Vertex.makeVertex(0, 0, 0)\n"
                "second = cq.Vertex.makeVertex(3, 4, 0)\n"
            ),
        },
        {
            "cmd": "measure",
            "id": 2,
            "selections": [
                {"key": "n1", "part": 0, "kind": "vertex", "identifier": 1},
                {"key": "n2", "part": 0, "kind": "vertex", "identifier": 1},
            ],
        },
        {"cmd": "release", "keys": ["n1", "n2"]},
        {
            "cmd": "measure",
            "id": 3,
            "selections": [{"key": "n1", "part": 0, "kind": "vertex", "identifier": 1}],
        },
    )

    talk.serve()

    (reply,) = talk.of_type("measurement")
    assert reply["id"] == 2
    assert reply["pair"]["distance"] == 5.0
    assert reply["identity"] == {"scope": "scene_entry", "persistent": False}
    (error,) = talk.of_type("error")
    assert error["id"] == 3
    assert "previous Python session" in error["message"]


def test_selector_for_picked_faces_and_its_errors(conversation):
    script = "box = cq.Workplane().box(2, 4, 6)\nshow_object(box)\n"
    # Face 6 of a box is its top: OCCT's order is deterministic for one box.
    top, bottom = 6, 5
    talk = conversation(
        {"cmd": "run", "id": 1, "script": script},
        {
            "cmd": "selector",
            "id": 2,
            "selections": [{"key": "n1", "part": 0, "kind": "face", "identifier": top}],
        },
        {
            "cmd": "selector",
            "id": 3,
            "selections": [
                {"key": "n1", "part": 0, "kind": "face", "identifier": top},
                {"key": "n1", "part": 0, "kind": "face", "identifier": bottom},
            ],
        },
        {
            "cmd": "selector",
            "id": 4,
            "selections": [{"key": "n1", "part": 0, "kind": "face", "identifier": 99}],
        },
    )
    talk.serve()

    first, both = talk.of_type("selector")
    assert first["id"] == 2 and first["method"] == "faces"
    assert first["candidates"][0] == {
        "expression": '.faces(">Z")',
        "selector": ">Z",
        "description": "largest Z",
        "index_based": False,
    }
    assert both["candidates"][0]["expression"] == '.faces("|Z")'
    (error,) = talk.of_type("error")
    assert error["id"] == 4 and "Unknown face id 99" in error["message"]


def test_selectors_are_available_while_debugging(conversation):
    talk = conversation(
        {
            "cmd": "run",
            "id": 1,
            "script": "box = cq.Workplane().box(1, 1, 1)\nx = 1\n",
            "debug": True,
            "breakpoints": [2],
        },
        {
            "cmd": "selector",
            "id": 2,
            "selections": [{"key": "n1", "part": 0, "kind": "edge", "identifier": 1}],
        },
        {"cmd": "debug", "action": "continue"},
    )
    talk.serve()

    (reply,) = talk.of_type("selector")
    assert reply["id"] == 2 and reply["candidates"]
    assert not talk.of_type("error")


def test_console_reset_discards_previous_script_variables(conversation):
    talk = conversation(
        {"cmd": "run", "id": 1, "script": "value = 7"},
        {"cmd": "console", "id": 2, "source": "print(value)"},
        {"cmd": "console_reset", "id": 3},
        {"cmd": "console", "id": 4, "source": "print('value' in globals())"},
    )
    talk.serve()
    assert talk.output(2) == "7\n"
    assert talk.output(4) == "False\n"
    assert [item["id"] for item in talk.of_type("console_result")] == [2, 3, 4]


def test_inspection_geometry_without_prior_inspection_reports_error(conversation):
    talk = conversation(
        {"cmd": "run", "id": 1, "script": BOX},
        {"cmd": "inspect_geometry", "id": 2, "key": "n1", "frame": 0},
    )
    talk.serve()
    (error,) = talk.of_type("error")
    assert error["id"] == 2
    assert "no longer inspected" in error["message"]


def test_python_upload_uses_utf8_source_and_no_model_selection(
    conversation, monkeypatch
):
    seen = {}

    def fake_upload(url, data, filename):
        seen.update(url=url, data=data, filename=filename)
        return {"status": 200, "body": "ok"}

    monkeypatch.setattr(network, "upload", fake_upload)
    talk = conversation(
        {
            "cmd": "upload",
            "id": 2,
            "url": "https://example.test",
            "file_type": "py",
            "source": "label = 'ação'",
        }
    )
    talk.serve()
    assert seen == {
        "url": "https://example.test",
        "data": "label = 'ação'".encode(),
        "filename": "model.py",
    }
    assert talk.of_type("upload_finished") == [
        {"id": 2, "type": "upload_finished", "status": 200, "body": "ok"}
    ]


def test_model_upload_requires_a_selection(conversation, monkeypatch):
    monkeypatch.setattr(network, "upload", lambda *_args: None)
    talk = conversation(
        {"cmd": "upload", "id": 1, "url": "https://example.test", "file_type": "step"}
    )
    talk.serve()
    (error,) = talk.of_type("error")
    assert error["id"] == 1
    assert "select a model" in error["message"]


def test_update_check_returns_current_and_newer_versions(conversation, monkeypatch):
    monkeypatch.setattr(network, "newer_releases", lambda _current: ["v99.0.0"])
    talk = conversation({"cmd": "check_updates", "id": 8})
    talk.serve()
    (reply,) = talk.of_type("updates")
    assert reply["id"] == 8
    assert reply["current"]
    assert reply["newer"] == ["v99.0.0"]


def test_console_requests_are_blocked_while_debugger_is_paused(conversation):
    talk = conversation(
        {
            "cmd": "run",
            "id": 1,
            "debug": True,
            "breakpoints": [2],
            "script": "a = 1\nb = 2\n",
        },
        {"cmd": "console", "id": 2, "source": "print(a)"},
        {"cmd": "debug", "action": "continue"},
    )
    talk.serve()
    assert talk.of_type("debug_paused")[0]["line"] == 2
    (error,) = talk.of_type("error")
    assert error["id"] == 2
    assert "Not available while debugging" in error["message"]
    assert talk.of_type("run_finished")[0]["ok"]


def test_debug_pause_identifies_the_project_module(conversation, tmp_path):
    module = tmp_path / "module_req08.py"
    module.write_text("def work():\n    result = 7\n    return result\n")
    talk = conversation(
        {
            "cmd": "run",
            "id": 1,
            "script": "import module_req08\nx = module_req08.work()\n",
            "path": str(tmp_path / "main.py"),
            "project_root": str(tmp_path),
            "debug": True,
            "breakpoints": [],
            "module_breakpoints": {str(module): [2]},
        },
        {
            "cmd": "debug",
            "action": "continue",
            "breakpoints": [],
            "module_breakpoints": {},
        },
    )
    talk.serve()

    (pause,) = talk.of_type("debug_paused")
    assert pause["path"].lower() == str(module).lower()
    assert pause["line"] == 2
    assert talk.of_type("run_finished")[0]["ok"]


def test_run_streams_meshes_before_the_final_scene(conversation):
    script = (
        "show_object(cq.Workplane().box(1, 1, 1), name='first')\n"
        "show_object(cq.Workplane().sphere(2), name='second')\n"
    )
    talk = conversation({"cmd": "run", "id": 4, "script": script})
    talk.serve()

    parts = talk.of_type("run_part")
    (finished,) = talk.of_type("run_finished")
    assert [part["name"] for part in parts] == ["first", "second"]
    assert all(part["part"]["mesh"]["indices"] for part in parts)
    assert all(list(node["parts"][0]["mesh"]) == ["id"] for node in finished["objects"])
    assert [message["type"] for message in talk.messages].index("run_part") < [
        message["type"] for message in talk.messages
    ].index("run_finished")


def test_part_limit_reports_the_object_instead_of_breaking_the_pipe(
    conversation, monkeypatch
):
    from cq_worker import scene

    monkeypatch.setattr(scene, "MAX_STREAMED_MESH_BYTES", 1)
    talk = conversation(
        {
            "cmd": "run",
            "id": 9,
            "script": "show_object(cq.Workplane().box(1, 1, 1), name='heavy')",
        },
        {"cmd": "run", "id": 10, "script": "print('still alive')"},
    )
    talk.serve()
    (error,) = talk.of_type("error")
    assert error["id"] == 9
    assert "heavy" in error["message"] and "too large" in error["message"]
    assert talk.of_type("run_finished")[0]["id"] == 10


def test_brep_export_writes_a_nonempty_file(conversation, tmp_path):
    path = tmp_path / "model.brep"
    talk = conversation(
        {"cmd": "run", "id": 1, "script": BOX},
        {"cmd": "export", "id": 2, "keys": ["n1"], "format": "brep", "path": str(path)},
    )
    talk.serve()
    assert talk.of_type("export_finished")[0]["path"] == str(path)
    assert path.stat().st_size > 100


def test_step_upload_sends_actual_model_bytes(conversation, monkeypatch):
    captured = {}

    def fake_upload(url, data, filename):
        captured.update(url=url, data=data, filename=filename)
        return {"status": 202, "body": "accepted"}

    monkeypatch.setattr(network, "upload", fake_upload)
    talk = conversation(
        {"cmd": "run", "id": 1, "script": BOX},
        {
            "cmd": "upload",
            "id": 2,
            "url": "https://example.test",
            "file_type": "step",
            "keys": ["n1"],
        },
    )
    talk.serve()
    assert captured["url"] == "https://example.test"
    assert captured["filename"] == "model.step"
    assert len(captured["data"]) > 100
    assert b"ISO-10303-21" in captured["data"]
    assert talk.of_type("upload_finished")[0]["status"] == 202


def test_console_selected_returns_the_picked_entities_and_objects(conversation):
    script = "box = cq.Workplane().box(2, 4, 6)\nshow_object(box)\n"
    top = {"key": "n1", "part": 0, "kind": "face", "identifier": 6}
    talk = conversation(
        {"cmd": "run", "id": 1, "script": script},
        {
            "cmd": "console",
            "id": 2,
            "source": "print(type(selected()[0]).__name__, selected()[0].Area())",
            "selection": {"entities": [top], "objects": ["n1"]},
        },
        {
            "cmd": "console",
            "id": 3,
            "source": "print(type(selected()[0]).__name__, selected()[0].Volume())",
            "selection": {"entities": [], "objects": ["n1"]},
        },
        {"cmd": "console", "id": 4, "source": "print(selected())"},
        {"cmd": "release", "keys": ["n1"]},
        {
            "cmd": "console",
            "id": 5,
            "source": "selected()",
            "selection": {"objects": ["n1"]},
        },
        {"cmd": "console", "id": 6, "source": "x = 1", "selection": {"objects": [7]}},
    )
    talk.serve()

    assert talk.output(2) == "Face 8.0\n"
    assert talk.output(3) == "Solid 48.0\n"
    assert talk.output(4) == "[]\n"
    assert "LookupError" in talk.output(5) and "render again" in talk.output(5)
    (error,) = talk.of_type("error")
    assert error["id"] == 6 and "scene keys" in error["message"]


def test_measurement_code_uses_the_scripts_variables(conversation):
    script = "box = cq.Workplane().box(2, 4, 6)\nshow_object(box)\n"
    talk = conversation(
        {"cmd": "run", "id": 1, "script": script},
        {"cmd": "measure", "id": 2, "selections": [], "objects": ["n1"]},
        {
            "cmd": "measurement_code",
            "id": 3,
            "selections": [{"key": "n1", "part": 0, "kind": "face", "identifier": 6}],
        },
    )
    talk.serve()

    (measured,) = talk.of_type("measurement")
    assert measured["objects"][0]["volume"] == 48
    (code,) = talk.of_type("measurement_code")
    assert code["id"] == 3 and code["verified"] and code["problem"] is None
    assert 'face_1 = box.faces(">Z").val()' in code["code"]


def test_measurement_code_waits_for_the_end_of_a_debugging_session(conversation):
    talk = conversation(
        {
            "cmd": "run",
            "id": 1,
            "script": "box = cq.Workplane().box(1, 1, 1)\nx = 1\n",
            "debug": True,
            "breakpoints": [2],
        },
        {
            "cmd": "measurement_code",
            "id": 2,
            "selections": [{"key": "n1", "part": 0, "kind": "face", "identifier": 1}],
        },
        {"cmd": "debug", "action": "continue"},
    )
    talk.serve()
    (error,) = talk.of_type("error")
    assert error["id"] == 2 and "debugging" in error["message"]


def test_update_checks_can_cover_cq_editor_and_skip_cadquery(conversation, monkeypatch):
    monkeypatch.setattr(network, "newer_releases", lambda _current: ["v99.0.0"])
    found = [{"tag": "v0.9.0", "name": "0.9", "url": "https://github.com/x"}]
    monkeypatch.setattr(network, "editor_releases", lambda current: found)
    talk = conversation(
        {"cmd": "check_updates", "id": 1, "editor": "0.8.0"},
        {"cmd": "check_updates", "id": 2, "editor": "0.8.0", "cadquery": False},
    )
    talk.serve()
    both, editor_only = talk.of_type("updates")
    assert both["newer"] == ["v99.0.0"]
    assert both["editor"] == {"current": "0.8.0", "newer": found}
    # The daily check of CQ-editor leaves CadQuery's releases alone.
    assert editor_only["newer"] == [] and editor_only["editor"]["newer"] == found
