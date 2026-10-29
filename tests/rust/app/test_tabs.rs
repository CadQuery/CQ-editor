use super::*;
use crate::app::tests::test_state;
use crate::commands::Action;
use crate::document::{DEFAULT_SCRIPT, Encoding};
use crate::recovery::RecoveryStore;

/// A folder of its own for each test, with `files` written in it.
fn folder(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let folder = std::env::temp_dir().join(format!("cq-editor-tabs-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    for (path, text) in files {
        let path = folder.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

/// The startup state, with recovery files kept in `folder`.
fn state_in(folder: &Path) -> AppState {
    let mut state = test_state(DEFAULT_SCRIPT);
    state.recovery = RecoveryStore::new(folder.join(".recovery"));
    state.cache_dir = folder.join(".cache");
    state
}

fn open(state: &mut AppState, path: &Path) {
    state.execute(Action::OpenPath(path.to_path_buf()));
}

fn names(state: &AppState) -> Vec<String> {
    state.tabs().into_iter().map(|tab| tab.name).collect()
}

fn shown_name(state: &AppState) -> String {
    state.document.display_name()
}

fn generation_of(state: &AppState, name: &str) -> u64 {
    state
        .tabs()
        .into_iter()
        .find(|tab| tab.name == name)
        .unwrap()
        .generation
}

#[test]
fn files_open_in_tabs_and_the_first_replaces_the_startup_script() {
    let dir = folder("open", &[("a.py", "a = 1\n"), ("b.py", "b = 2\n")]);
    let mut state = state_in(&dir);
    assert_eq!(names(&state), ["untitled"]);

    open(&mut state, &dir.join("a.py"));
    assert_eq!(
        names(&state),
        ["a.py"],
        "the untouched startup script gave way"
    );
    open(&mut state, &dir.join("b.py"));
    assert_eq!(names(&state), ["a.py", "b.py"]);
    assert_eq!(shown_name(&state), "b.py");

    // An open file is shown, not opened twice.
    open(&mut state, &dir.join("a.py"));
    assert_eq!(names(&state), ["a.py", "b.py"]);
    assert_eq!(shown_name(&state), "a.py");

    // New and examples add tabs after the shown one.
    state.execute(Action::NewFile);
    assert_eq!(names(&state), ["a.py", "untitled", "b.py"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn each_tab_keeps_its_text_breakpoints_and_undo_history() {
    let dir = folder("state", &[("a.py", "a = 1\n"), ("b.py", "b = 2\nc = 3\n")]);
    let mut state = state_in(&dir);
    open(&mut state, &dir.join("a.py"));
    state.document.text = "a = 10\n".into();
    state.editor.toggle_breakpoint(1);
    let a_cursor = state.editor.cursor_position(&state.document.text);

    open(&mut state, &dir.join("b.py"));
    assert_eq!(state.document.text, "b = 2\nc = 3\n");
    assert!(state.editor.breakpoints.is_empty(), "b has its own editor");
    state.editor.toggle_breakpoint(2);
    state.execute(Action::ToggleComment);
    assert_eq!(state.document.text, "# b = 2\nc = 3\n");

    state.execute(Action::PreviousTab);
    assert_eq!(state.document.text, "a = 10\n", "unsaved edits stay");
    assert!(state.document.is_modified());
    assert_eq!(
        state.editor.breakpoints.iter().copied().collect::<Vec<_>>(),
        [1]
    );
    assert_eq!(state.editor.cursor_position(&state.document.text), a_cursor);

    state.execute(Action::NextTab);
    assert_eq!(
        state.editor.breakpoints.iter().copied().collect::<Vec<_>>(),
        [2]
    );
    state.execute(Action::Undo);
    assert_eq!(
        state.document.text, "b = 2\nc = 3\n",
        "b's own undo history"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tabs_cycle_both_ways_and_wrap_around() {
    let dir = folder("cycle", &[("a.py", ""), ("b.py", ""), ("c.py", "")]);
    let mut state = state_in(&dir);
    for name in ["a.py", "b.py", "c.py"] {
        open(&mut state, &dir.join(name));
    }
    state.execute(Action::NextTab);
    assert_eq!(shown_name(&state), "a.py");
    state.execute(Action::PreviousTab);
    assert_eq!(shown_name(&state), "c.py");
    state.execute(Action::ActivateTab(generation_of(&state, "b.py")));
    assert_eq!(shown_name(&state), "b.py");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tabs_of_files_with_the_same_name_show_their_folder() {
    let dir = folder(
        "twins",
        &[("left/part.py", ""), ("right/part.py", ""), ("main.py", "")],
    );
    let mut state = state_in(&dir);
    open(&mut state, &dir.join("left/part.py"));
    open(&mut state, &dir.join("right/part.py"));
    open(&mut state, &dir.join("main.py"));
    assert_eq!(
        names(&state),
        ["part.py — left", "part.py — right", "main.py"]
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn closing_a_modified_tab_asks_first_then_saves_or_discards() {
    let dir = folder("close", &[("a.py", "a = 1\n"), ("b.py", "b = 2\n")]);
    let mut state = state_in(&dir);
    open(&mut state, &dir.join("a.py"));
    open(&mut state, &dir.join("b.py"));

    state.document.text = "b = 20\n".into();
    state.execute(Action::CloseTab);
    let b = generation_of(&state, "b.py");
    assert_eq!(state.dialogs.unsaved, Some(Intent::Close(vec![b])));
    assert_eq!(names(&state), ["a.py", "b.py"], "nothing closed yet");
    state.resolve_unsaved(
        Intent::Close(vec![b]),
        crate::dialogs::UnsavedChoice::Cancel,
    );
    assert_eq!(names(&state), ["a.py", "b.py"]);

    state.resolve_unsaved(
        Intent::Close(vec![b]),
        crate::dialogs::UnsavedChoice::Discard,
    );
    assert_eq!(names(&state), ["a.py"]);
    assert_eq!(shown_name(&state), "a.py", "its neighbor is shown");
    assert_eq!(
        std::fs::read_to_string(dir.join("b.py")).unwrap(),
        "b = 2\n"
    );

    state.document.text = "a = 10\n".into();
    let a = state.document_generation;
    state.execute(Action::CloseDocument(a));
    state.resolve_unsaved(Intent::Close(vec![a]), crate::dialogs::UnsavedChoice::Save);
    assert_eq!(
        std::fs::read_to_string(dir.join("a.py")).unwrap(),
        "a = 10\n"
    );
    // There is always a document.
    assert_eq!(names(&state), ["untitled"]);
    assert!(state.document.text.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unmodified_tabs_close_without_a_question() {
    let dir = folder("close-clean", &[("a.py", ""), ("b.py", ""), ("c.py", "")]);
    let mut state = state_in(&dir);
    for name in ["a.py", "b.py", "c.py"] {
        open(&mut state, &dir.join(name));
    }
    state.execute(Action::CloseOtherTabs(generation_of(&state, "b.py")));
    assert!(state.dialogs.unsaved.is_none());
    assert_eq!(names(&state), ["b.py"]);
    state.execute(Action::CloseAllTabs);
    assert_eq!(names(&state), ["untitled"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn closing_the_script_paused_in_the_debugger_ends_its_session() {
    let dir = folder("paused", &[("a.py", "a = 1\nb = 2\n"), ("b.py", "c = 3\n")]);
    let mut state = state_in(&dir);
    open(&mut state, &dir.join("a.py"));
    open(&mut state, &dir.join("b.py"));
    let a = generation_of(&state, "a.py");
    let text = state.document_of(a).unwrap().text.clone();

    // A debugging session of a.py, paused at a breakpoint.
    let id = 7;
    state.run_script = Some((
        id,
        std::sync::Arc::new(crate::scene::RunScript { document: a, text }),
    ));
    state.execution.started(id, true);
    assert!(!state.execution.paused(id));
    assert!(state.is_paused());

    state.execute(Action::CloseDocument(a));
    assert!(state.document_of(a).is_none());
    assert!(
        !state.is_debugging(),
        "a disconnected worker leaves no paused session"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn quitting_asks_about_every_modified_tab_at_once() {
    let dir = folder("quit", &[("a.py", "a\n"), ("b.py", "b\n"), ("c.py", "c\n")]);
    let mut state = state_in(&dir);
    for name in ["a.py", "b.py", "c.py"] {
        open(&mut state, &dir.join(name));
    }
    state.document.text = "c changed\n".into();
    state.execute(Action::ActivateTab(generation_of(&state, "a.py")));
    state.document.text = "a changed\n".into();
    assert!(state.any_modified());

    state.execute(Action::Quit);
    assert_eq!(state.dialogs.unsaved, Some(Intent::Quit));
    let unsaved = state.unsaved_of(&Intent::Quit);
    assert_eq!(state.names_of(&unsaved), ["a.py", "c.py"], "in tab order");

    state.resolve_unsaved(Intent::Quit, crate::dialogs::UnsavedChoice::Save);
    assert_eq!(
        std::fs::read_to_string(dir.join("a.py")).unwrap(),
        "a changed\n"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("c.py")).unwrap(),
        "c changed\n"
    );
    assert!(!state.any_modified());
    assert!(state.allow_close);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_session_reopens_the_tabs_and_shows_the_last_file() {
    let dir = folder(
        "session",
        &[("a.py", "a\n"), ("b.py", "b\n"), ("c.py", "c\n")],
    );
    let mut state = state_in(&dir);
    for name in ["a.py", "b.py"] {
        open(&mut state, &dir.join(name));
    }
    state.execute(Action::PreviousTab);
    state.remember_session();
    assert_eq!(
        state.settings.open_files,
        [dir.join("a.py"), dir.join("b.py")]
    );
    assert_eq!(
        state.settings.last_file.as_deref(),
        Some(dir.join("a.py").as_path())
    );

    let mut next = state_in(&dir);
    next.settings = state.settings.clone();
    std::fs::remove_file(dir.join("b.py")).unwrap();
    let problems = next.restore_session(None);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(names(&next), ["a.py"], "a deleted file is skipped");
    assert_eq!(shown_name(&next), "a.py");

    // A file given on the command line joins them and is shown.
    let mut next = state_in(&dir);
    next.settings = state.settings.clone();
    next.restore_session(Some(dir.join("c.py")));
    assert_eq!(names(&next), ["a.py", "c.py"]);
    assert_eq!(shown_name(&next), "c.py");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn settings_of_a_release_without_tabs_reopen_their_last_file() {
    let dir = folder("legacy", &[("a.py", "a\n")]);
    let mut state = state_in(&dir);
    state.settings.last_file = Some(dir.join("a.py"));
    state.restore_session(None);
    assert_eq!(names(&state), ["a.py"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn f5_renders_the_main_script_while_a_project_file_is_shown() {
    let dir = folder(
        "main",
        &[
            ("main.py", "from part import plate\nshow_object(plate)\n"),
            ("part.py", "plate = 1\n"),
        ],
    );
    let outside = folder("outside", &[("other.py", "x = 1\n")]);
    let mut state = state_in(&dir);
    state.open_project(&dir).unwrap();
    let project = state.project.as_ref().unwrap();
    assert_eq!(project.main, Some(dir.join("main.py")), "guessed");

    open(&mut state, &dir.join("part.py"));
    assert_eq!(state.main_script(), Some(dir.join("main.py").as_path()));
    let part = state.document_generation;
    assert_eq!(state.run_target(), part, "not open yet");

    let target = state.prepare_run_target();
    assert_ne!(target, part);
    assert_eq!(shown_name(&state), "part.py", "opened without being shown");
    assert_eq!(names(&state), ["part.py", "main.py"]);
    assert_eq!(state.run_target(), target);
    assert!(
        state
            .tabs()
            .iter()
            .any(|tab| tab.main && tab.generation == target)
    );

    // A file outside the project renders itself.
    open(&mut state, &outside.join("other.py"));
    assert_eq!(state.main_script(), None);
    assert_eq!(state.run_target(), state.document_generation);

    // Without a main script, F5 renders the file shown.
    state.execute(Action::ActivateTab(part));
    state.execute(Action::SetMainScript(None));
    assert_eq!(state.run_target(), part);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&outside);
}

#[test]
fn errors_and_parameters_belong_to_the_script_that_runs() {
    let dir = folder(
        "errors",
        &[
            ("main.py", "width = 4\nboom()\n"),
            ("part.py", "plate = 1\n"),
        ],
    );
    let mut state = state_in(&dir);
    state.open_project(&dir).unwrap();
    open(&mut state, &dir.join("part.py"));
    let main = state.prepare_run_target();
    state.execution.started(4, false);
    state.run_script = Some((
        4,
        std::sync::Arc::new(crate::scene::RunScript {
            document: main,
            text: "width = 4\nboom()\n".into(),
        }),
    ));
    let message: crate::worker::protocol::Message = serde_json::from_str(
        r#"{"type":"run_finished","id":4,"ok":false,"debug":false,"stopped":false,
            "objects":null,"variables":null,"duration":0.1,
            "error":{"type_name":"NameError","message":"name 'boom' is not defined",
                "frames":[{"file":"<cq-editor>","line":2,"function":"<module>","code":"boom()"}],
                "line":2,"text":"NameError\n"}}"#,
    )
    .unwrap();
    state.handle_message(message);

    assert_eq!(shown_name(&state), "part.py", "the shown file stays");
    assert_eq!(state.editor.error_line, None);
    assert_eq!(state.traceback_document, Some(main));
    let status = state.status.as_ref().unwrap();
    assert!(
        status.text.starts_with("main.py: NameError"),
        "{}",
        status.text
    );

    // The traceback's line is in main.py.
    state.execute(Action::GoToScriptLine {
        document: main,
        line: 2,
    });
    assert_eq!(shown_name(&state), "main.py");
    assert_eq!(state.editor.error_line, Some(2));

    // Parameters are the main script's, even from another tab.
    state.execute(Action::PreviousTab);
    assert_eq!(shown_name(&state), "part.py");
    state.set_parameter("width", 1, 6.0);
    assert_eq!(state.document_of(main).unwrap().text, "width = 6\nboom()\n");
    assert_eq!(state.document.text, "plate = 1\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unsaved_modules_next_to_the_script_are_reported_after_a_render() {
    let dir = folder("modules", &[("main.py", "x = 1\n"), ("part.py", "p = 1\n")]);
    let mut state = state_in(&dir);
    open(&mut state, &dir.join("main.py"));
    let main = state.document_generation;
    open(&mut state, &dir.join("part.py"));
    assert!(state.unsaved_modules(main).is_empty());
    state.document.text = "p = 2\n".into();
    assert_eq!(state.unsaved_modules(main), ["part.py"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_traceback_frame_in_a_module_opens_it() {
    let dir = folder("module-frame", &[("main.py", ""), ("part.py", "a\nb\nc\n")]);
    let mut state = state_in(&dir);
    open(&mut state, &dir.join("main.py"));
    state.execute(Action::OpenFileAt {
        path: dir.join("part.py"),
        line: 3,
    });
    assert_eq!(shown_name(&state), "part.py");
    assert_eq!(names(&state), ["main.py", "part.py"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_changed_on_disk_reloads_its_tab_unless_it_has_unsaved_edits() {
    let dir = folder("reload", &[("a.py", "a\n"), ("b.py", "b\n")]);
    let mut state = state_in(&dir);
    open(&mut state, &dir.join("a.py"));
    let a = state.document_generation;
    open(&mut state, &dir.join("b.py"));

    std::fs::write(dir.join("a.py"), "a from elsewhere\n").unwrap();
    assert_eq!(state.reload_tab(a), Some(ExternalUpdateKind::Reload));
    assert_eq!(state.document_of(a).unwrap().text, "a from elsewhere\n");

    state.document.text = "b edited here\n".into();
    std::fs::write(dir.join("b.py"), "b from elsewhere\n").unwrap();
    let b = state.document_generation;
    assert_eq!(state.reload_tab(b), Some(ExternalUpdateKind::Conflict));
    assert_eq!(state.document.text, "b edited here\n", "never lost");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Runs a frame's shortcut handling with `key` pressed.
fn press(state: &mut AppState, key: egui::Key, modifiers: egui::Modifiers) {
    let ctx = state.ctx.clone();
    let input = egui::RawInput {
        events: vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }],
        ..Default::default()
    };
    let mut output = ctx.run_ui(input, |_| state.begin_frame(&ctx));
    output.textures_delta.clear();
}

#[test]
fn shortcuts_wait_while_a_modal_dialog_is_open() {
    let dir = folder("modal", &[("a.py", "a\n"), ("b.py", "b\n")]);
    let mut state = state_in(&dir);
    open(&mut state, &dir.join("a.py"));
    open(&mut state, &dir.join("b.py"));
    let ctrl = egui::Modifiers::COMMAND;
    press(&mut state, egui::Key::Tab, ctrl);
    assert_eq!(std::mem::take(&mut state.actions), [Action::NextTab]);

    // Any modal question: nothing may change behind it.
    state.dialogs.info("Updates", "No updates available");
    press(&mut state, egui::Key::Tab, ctrl);
    press(&mut state, egui::Key::W, ctrl);
    press(&mut state, egui::Key::F5, egui::Modifiers::NONE);
    assert!(state.actions.is_empty(), "{:?}", state.actions);

    state.dialogs.message = None;
    press(&mut state, egui::Key::W, ctrl);
    assert_eq!(state.actions, [Action::CloseTab]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_conflict_is_resolved_on_its_own_document_whichever_tab_is_shown() {
    use crate::dialogs::{ExternalChange, ExternalChangeChoice};
    let dir = folder("conflict-tab", &[("a.py", "a = 1\n"), ("b.py", "b = 2\n")]);
    let file = dir.join("a.py");
    let mut state = state_in(&dir);
    open(&mut state, &file);
    let a = state.document_generation;
    state.document.text = "a = 10  # mine\n".into();
    open(&mut state, &dir.join("b.py"));
    let b = state.document_generation;

    let conflict = |state: &mut AppState| {
        assert_eq!(state.reload_tab(a), Some(ExternalUpdateKind::Conflict));
        let disk = state
            .document_of(a)
            .unwrap()
            .disk_version()
            .unwrap()
            .unwrap();
        state.execute(Action::ActivateTab(b));
        ExternalChange {
            document: a,
            path: file.clone(),
            disk,
            show_comparison: false,
            rerun_after_resolution: false,
            on_save: false,
        }
    };

    // Keeping my version works on a.py although b.py is shown.
    std::fs::write(&file, "a = 98  # theirs\n").unwrap();
    let change = conflict(&mut state);
    state.resolve_external_change(change, ExternalChangeChoice::KeepLocal);
    assert_eq!(state.document_generation, a, "its tab is shown");
    assert_eq!(state.document.text, "a = 10  # mine\n");
    state.execute(Action::Save);
    assert!(state.dialogs.external_change.is_none(), "already decided");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "a = 10  # mine\n");

    // Reloading replaces a.py's buffer, not the one shown.
    state.document.text = "a = 11  # mine again\n".into();
    std::fs::write(&file, "a = 99  # theirs\n").unwrap();
    let change = conflict(&mut state);
    state.resolve_external_change(change, ExternalChangeChoice::Reload);
    assert_eq!(state.document_of(a).unwrap().text, "a = 99  # theirs\n");
    assert_eq!(state.document_of(b).unwrap().text, "b = 2\n");

    // A conflict of a tab closed meanwhile is dropped.
    state.document.text = "a = 12\n".into();
    std::fs::write(&file, "a = 100\n").unwrap();
    let change = conflict(&mut state);
    state.close_now(a);
    state.resolve_external_change(change, ExternalChangeChoice::Reload);
    assert_eq!(names(&state), ["b.py"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn saving_never_replaces_a_version_another_program_wrote_unseen() {
    use crate::dialogs::ExternalChangeChoice;
    let dir = folder("save-conflict", &[("part.py", "width = 1\n")]);
    let file = dir.join("part.py");
    let mut state = state_in(&dir);
    open(&mut state, &file);
    assert!(
        !state.settings.editor.autoreload,
        "nothing watches the file"
    );

    // Another program saves; a habitual Ctrl+S must not undo it.
    std::fs::write(&file, "width = 2  # theirs\n").unwrap();
    state.execute(Action::Save);
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "width = 2  # theirs\n"
    );
    let change = state.dialogs.external_change.take().expect("asked");
    assert!(change.on_save);
    assert_eq!(change.disk.text, "width = 2  # theirs\n");

    // Cancel saves nothing.
    state.resolve_external_change(change, ExternalChangeChoice::Cancel);
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "width = 2  # theirs\n"
    );

    // Saving my version replaces it, keeping it in the local history.
    state.document.text = "width = 3  # mine\n".into();
    state.execute(Action::Save);
    let change = state.dialogs.external_change.take().expect("asked again");
    state.resolve_external_change(change, ExternalChangeChoice::KeepLocal);
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "width = 3  # mine\n"
    );
    assert!(!state.document.is_modified());
    let history = state.recovery.history(Some(&file)).unwrap();
    assert!(history.iter().any(|v| v.text == "width = 2  # theirs\n"));

    // Reloading takes the other version instead.
    std::fs::write(&file, "width = 4  # theirs again\n").unwrap();
    state.execute(Action::Save);
    let change = state.dialogs.external_change.take().expect("asked");
    state.resolve_external_change(change, ExternalChangeChoice::Reload);
    assert_eq!(state.document.text, "width = 4  # theirs again\n");
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "width = 4  # theirs again\n"
    );

    // Without a change elsewhere, saving asks nothing.
    state.document.text = "width = 5\n".into();
    state.execute(Action::Save);
    assert!(state.dialogs.external_change.is_none());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "width = 5\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn focus_checks_all_tabs_even_without_automatic_reload() {
    let dir = folder("focus-disk", &[("a.py", "a = 1\n"), ("b.py", "b = 1\n")]);
    let mut state = state_in(&dir);
    open(&mut state, &dir.join("a.py"));
    let a = generation_of(&state, "a.py");
    state.document.text = "a = 3  # local\n".into();
    open(&mut state, &dir.join("b.py"));
    assert!(!state.settings.editor.autoreload);

    std::fs::write(dir.join("a.py"), "a = 2  # disk\n").unwrap();
    std::fs::write(dir.join("b.py"), "b = 2  # disk\n").unwrap();
    state.check_disk_on_focus();
    assert_eq!(state.document_of(a).unwrap().text, "a = 3  # local\n");
    let change = state
        .dialogs
        .external_change
        .take()
        .expect("conflict in hidden tab");
    assert_eq!(change.document, a);
    assert_eq!(change.disk.text, "a = 2  # disk\n");
    let b = generation_of(&state, "b.py");
    assert_eq!(state.document_of(b).unwrap().text, "b = 2  # disk\n");

    state.resolve_external_change(change, crate::dialogs::ExternalChangeChoice::Cancel);
    std::fs::remove_file(dir.join("a.py")).unwrap();
    state.check_disk_on_focus();
    assert!(state.document_of(a).unwrap().is_missing_on_disk());
    assert!(
        state
            .tabs()
            .iter()
            .any(|tab| tab.name.contains("deleted on disk"))
    );
    std::fs::write(dir.join("a.py"), "a = 4\n").unwrap();
    state.check_disk_on_focus();
    assert!(!state.document_of(a).unwrap().is_missing_on_disk());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn local_history_restores_the_tab_that_opened_it() {
    let dir = folder(
        "history-dialog",
        &[("a.py", "a = 1\n"), ("b.py", "b = 1\n")],
    );
    let mut state = state_in(&dir);
    open(&mut state, &dir.join("a.py"));
    let a = state.document_generation;
    state.recovery.archive(&state.document).unwrap();
    state.document.text = "a = 2\n".into();
    state.show_local_history();
    let history = state.dialogs.history.take().expect("history dialog");
    assert_eq!(history.document, a);
    let old = history
        .versions
        .into_iter()
        .find(|version| version.text == "a = 1\n")
        .unwrap();

    open(&mut state, &dir.join("b.py"));
    let b = state.document_generation;
    state.restore_history(a, old);
    assert_eq!(state.document_generation, a);
    assert_eq!(state.document.text, "a = 1\n");
    assert_eq!(state.document_of(b).unwrap().text, "b = 1\n");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn completions_asked_for_in_another_tab_are_dropped() {
    use crate::app::execution::Pending;
    let dir = folder("completions", &[("a.py", "import cadquery as cq\ncq.\n")]);
    let mut state = state_in(&dir);
    state.execute(Action::NewFile);
    let untitled = state.document_generation;
    open(&mut state, &dir.join("a.py"));
    state.execution.track(
        5,
        Pending::Completion {
            automatic: false,
            document: untitled,
        },
    );
    let message: crate::worker::protocol::Message = serde_json::from_str(
        r#"{"type":"completions","id":5,"available":true,
            "items":[{"name":"Workplane","insert":"Workplane","kind":"class"}],"signatures":[]}"#,
    )
    .unwrap();
    state.status = None;
    state.handle_message(message);
    assert!(state.status.is_none(), "no message about it either");
    assert!(!state.execution.is_pending(5), "answered");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn recovered_work_returns_to_the_tabs_of_its_files() {
    let dir = folder("recover", &[("a.py", "a\n"), ("b.py", "b\n")]);
    let mut state = state_in(&dir);
    open(&mut state, &dir.join("a.py"));
    let snapshot = |path: Option<PathBuf>, text: &str| {
        let mut document = match &path {
            Some(path) => Document::open(path).unwrap(),
            None => Document::untitled(""),
        };
        document.text = text.into();
        let mut store = RecoveryStore::new(dir.join(".snapshots"));
        store.checkpoint(&document).unwrap();
        store.load_session().unwrap().remove(0)
    };
    let session = crate::dialogs::RecoverySession {
        documents: vec![
            snapshot(Some(dir.join("a.py")), "a recovered\n"),
            snapshot(None, "draft\n"),
        ],
        selected: 0,
    };
    state.resolve_recovery_session(session, true);
    assert_eq!(names(&state), ["a.py", "untitled"]);
    let a = generation_of(&state, "a.py");
    assert_eq!(state.document_of(a).unwrap().text, "a recovered\n");
    assert!(state.document_of(a).unwrap().is_modified());
    assert_eq!(state.document.text, "draft\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn renaming_or_deleting_files_keeps_the_tabs_in_step() {
    let dir = folder(
        "rename",
        &[
            ("main.py", "m\n"),
            ("parts/axle.py", "a\n"),
            ("parts/gear.py", "g\n"),
        ],
    );
    let mut state = state_in(&dir);
    state.open_project(&dir).unwrap();
    open(&mut state, &dir.join("parts/axle.py"));
    state.document.text = "a edited\n".into();
    open(&mut state, &dir.join("parts/gear.py"));

    state
        .rename_project_path(&dir.join("parts"), "components")
        .unwrap();
    let axle = generation_of(&state, "axle.py");
    let renamed = state.document_of(axle).unwrap();
    assert_eq!(
        renamed.path(),
        Some(dir.join("components/axle.py").as_path())
    );
    assert!(renamed.is_modified(), "unsaved edits stay unsaved");
    assert_eq!(
        state.settings.recent_files[0],
        dir.join("components/gear.py")
    );

    // Names that exist, or that are invalid, are refused with a reason.
    let error = state
        .rename_project_path(&dir.join("main.py"), "components")
        .unwrap_err();
    assert!(error.contains("already exists"), "{error}");

    state
        .create_project_file(&dir.join("components"), "bolt")
        .unwrap();
    assert_eq!(shown_name(&state), "bolt.py");
    assert!(dir.join("components/bolt.py").is_file());

    assert_eq!(state.unsaved_below(&dir.join("components")), ["axle.py"]);
    state.delete_project_path(&dir.join("components")).unwrap();
    assert!(!dir.join("components").exists());
    assert_eq!(names(&state), ["untitled"], "their tabs closed");
    // The deleted edits stay in the local history.
    let history = state
        .recovery
        .history(Some(&dir.join("components/axle.py")))
        .unwrap();
    assert!(history.iter().any(|version| version.text == "a edited\n"));
    assert!(
        state
            .delete_project_path(&dir)
            .unwrap_err()
            .contains("project folder")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_reopened_project_keeps_its_main_script_and_folders() {
    let dir = folder(
        "reopen",
        &[
            ("a.py", "show_object(a)\n"),
            ("b.py", "show_object(b)\n"),
            ("sub/c.py", ""),
        ],
    );
    let mut state = state_in(&dir);
    state.open_project(&dir).unwrap();
    assert_eq!(
        state.project.as_ref().unwrap().main,
        None,
        "not clear which"
    );
    state.execute(Action::SetMainScript(Some(dir.join("b.py"))));
    state.project.as_mut().unwrap().toggle(&dir.join("sub"));
    state.execute(Action::CloseFolder);
    assert!(state.project.is_none());

    state.execute(Action::OpenFolderPath(dir.clone()));
    let project = state.project.as_ref().unwrap();
    assert_eq!(project.main, Some(dir.join("b.py")));
    assert!(project.is_expanded(&dir.join("sub")));
    assert_eq!(state.settings.projects[0].root, project.root());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn named_views_follow_the_shown_tab() {
    let dir = folder("views", &[("a.py", ""), ("b.py", "")]);
    let mut state = state_in(&dir);
    open(&mut state, &dir.join("a.py"));
    state.execute(Action::SaveView(Some("front".into())));
    open(&mut state, &dir.join("b.py"));
    assert!(state.viewer.named_views.is_empty());
    state.execute(Action::PreviousTab);
    assert_eq!(state.viewer.named_views[0].name, "front");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_legacy_encoded_file_opens_in_its_own_tab() {
    let dir = folder("legacy-encoding", &[("a.py", "a\n")]);
    std::fs::write(dir.join("old.py"), b"# caf\xe9\n").unwrap();
    let mut state = state_in(&dir);
    open(&mut state, &dir.join("a.py"));
    open(&mut state, &dir.join("old.py"));
    assert_eq!(state.dialogs.legacy_encoding, Some(dir.join("old.py")));
    state.open_path_with(&dir.join("old.py"), Encoding::Windows1252);
    assert_eq!(names(&state), ["a.py", "old.py"]);
    assert_eq!(state.document.text, "# café\n");
    let _ = std::fs::remove_dir_all(&dir);
}

use crate::document::ExternalUpdate as ExternalUpdateKind;
