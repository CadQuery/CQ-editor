use super::*;

#[test]
fn recent_files_are_unique_and_bounded() {
    let mut settings = Settings::default();
    for i in 0..15 {
        settings.add_recent_file(Path::new(&format!("{i}.py")));
    }
    settings.add_recent_file(Path::new("12.py"));
    assert_eq!(settings.recent_files.len(), MAX_RECENT_FILES);
    assert_eq!(settings.recent_files[0], Path::new("12.py"));
    assert_eq!(
        settings
            .recent_files
            .iter()
            .filter(|p| *p == Path::new("12.py"))
            .count(),
        1
    );
}

#[test]
fn missing_fields_take_defaults() {
    let settings: Settings = serde_json::from_str(r#"{"viewer": {"show_edges": false}}"#).unwrap();
    assert!(!settings.viewer.show_edges);
    assert!(settings.viewer.fit_automatically);
    assert_eq!(settings.editor, Editor::default());
}

#[test]
fn preferences_round_trip_through_json() {
    let mut settings = Settings::default();
    settings.general.theme = Theme::Dark;
    settings.general.python = "python-custom".into();
    settings.viewer.projection = Projection::Perspective;
    settings.viewer.orbit_method = OrbitMethod::Trackball;
    settings.editor.autoreload = true;
    settings.objects.preserve_properties = true;
    settings.execution.reload_imported_modules = false;
    settings.add_recent_file(Path::new("part.py"));
    settings.open_files = vec!["part.py".into(), "parts/axle.py".into()];
    settings.project = Some("project".into());
    settings.remember_project(ProjectState {
        root: "project".into(),
        main: Some("project/main.py".into()),
        expanded: vec!["project/parts".into()],
    });

    let json = serde_json::to_string(&settings).unwrap();
    let loaded: Settings = serde_json::from_str(&json).unwrap();
    assert_eq!(loaded, settings);
}

#[test]
fn execution_and_scene_settings_are_sent_to_worker() {
    let mut settings = Settings::default();
    settings.execution.reload_cq = true;
    settings.execution.add_script_dir_to_path = false;
    settings.execution.change_working_dir = false;
    settings.execution.reload_imported_modules = false;
    settings.viewer.linear_tolerance = 0.005;
    settings.viewer.angular_tolerance = 0.4;
    settings.objects.merge_assemblies = true;

    assert_eq!(
        settings.run_settings(),
        RunSettings {
            reload_cq: true,
            add_script_dir_to_path: false,
            change_working_dir: false,
            reload_imported_modules: false,
        }
    );
    assert_eq!(
        settings.scene_settings(),
        SceneSettings {
            linear_tolerance: 0.005,
            angular_tolerance: 0.4,
            merge_assemblies: true,
            units: "millimeter".into(),
        }
    );
}

#[test]
fn breakpoints_are_remembered_per_file_and_bounded() {
    let mut settings = Settings::default();
    let breakpoint = |line: usize| SavedBreakpoint {
        line,
        text: format!("line {line}"),
    };
    settings.remember_breakpoints(Path::new("a.py"), vec![breakpoint(3)]);
    settings.remember_breakpoints(Path::new("b.py"), vec![breakpoint(7), breakpoint(9)]);
    assert_eq!(settings.breakpoints_of(Path::new("a.py")), [breakpoint(3)]);
    assert_eq!(
        settings.breakpoints[0].path,
        Path::new("b.py"),
        "the latest first"
    );

    // Clearing every breakpoint forgets the file.
    settings.remember_breakpoints(Path::new("a.py"), Vec::new());
    assert!(settings.breakpoints_of(Path::new("a.py")).is_empty());
    assert_eq!(settings.breakpoints.len(), 1);

    for file in 0..MAX_BREAKPOINT_FILES + 5 {
        let path = format!("{file}.py");
        settings.remember_breakpoints(Path::new(&path), vec![breakpoint(1)]);
    }
    assert_eq!(settings.breakpoints.len(), MAX_BREAKPOINT_FILES);

    let json = serde_json::to_string(&settings).unwrap();
    let loaded: Settings = serde_json::from_str(&json).unwrap();
    assert_eq!(loaded.breakpoints, settings.breakpoints);
}

#[test]
fn reopening_a_recent_file_moves_it_to_the_front() {
    let mut settings = Settings::default();
    settings.add_recent_file(Path::new("first.py"));
    settings.add_recent_file(Path::new("second.py"));
    settings.add_recent_file(Path::new("first.py"));
    assert_eq!(
        settings.recent_files,
        vec![PathBuf::from("first.py"), PathBuf::from("second.py")]
    );
}

#[test]
fn named_views_are_remembered_per_file_and_bounded() {
    let mut settings = Settings::default();
    let view = |name: &str| NamedView {
        name: name.into(),
        rotation: [0.0, 0.0, 0.0, 1.0],
        extent: 5.0,
        ..NamedView::default()
    };
    settings.remember_views(Path::new("a.py"), vec![view("front")]);
    settings.remember_views(Path::new("b.py"), vec![view("top"), view("detail")]);
    assert_eq!(settings.views_of(Path::new("a.py")), [view("front")]);
    assert_eq!(
        settings.views[0].path,
        Path::new("b.py"),
        "the latest first"
    );

    // Deleting every view forgets the file.
    settings.remember_views(Path::new("a.py"), Vec::new());
    assert!(settings.views_of(Path::new("a.py")).is_empty());
    assert_eq!(settings.views.len(), 1);

    for file in 0..MAX_VIEW_FILES + 5 {
        let path = format!("{file}.py");
        settings.remember_views(Path::new(&path), vec![view("v")]);
    }
    assert_eq!(settings.views.len(), MAX_VIEW_FILES);

    // Saved settings without views, or with older view entries, still load.
    let old: Settings = serde_json::from_str(r#"{"viewer": {}}"#).unwrap();
    assert!(old.views.is_empty());
    let partial: Settings =
        serde_json::from_str(r#"{"views": [{"path": "c.py", "views": [{"name": "x"}]}]}"#).unwrap();
    assert_eq!(partial.views_of(Path::new("c.py"))[0].name, "x");
}

#[test]
fn the_original_projection_and_stereo_names_are_kept() {
    let settings: Settings = serde_json::from_str(
        r#"{"viewer": {"projection": "MonoLeftEye", "stereo_mode": "OverUnder"}}"#,
    )
    .unwrap();
    assert_eq!(settings.viewer.projection, Projection::MonoLeftEye);
    assert_eq!(settings.viewer.stereo_mode, StereoMode::OverUnder);
    for (projection, name) in Projection::ALL.into_iter().zip([
        "Orthographic",
        "Perspective",
        "Stereo",
        "MonoLeftEye",
        "MonoRightEye",
    ]) {
        assert_eq!(serde_json::to_value(projection).unwrap(), name);
    }
    for (mode, name) in StereoMode::ALL.into_iter().zip([
        "QuadBuffer",
        "Anaglyph",
        "RowInterlaced",
        "ColumnInterlaced",
        "ChessBoard",
        "SideBySide",
        "OverUnder",
    ]) {
        assert_eq!(serde_json::to_value(mode).unwrap(), name);
    }
}

#[test]
fn project_folders_are_remembered_most_recent_first_and_bounded() {
    let mut settings = Settings::default();
    for i in 0..MAX_PROJECTS + 3 {
        settings.remember_project(ProjectState {
            root: format!("p{i}").into(),
            ..ProjectState::default()
        });
    }
    settings.remember_project(ProjectState {
        root: "p5".into(),
        main: Some("p5/main.py".into()),
        expanded: Vec::new(),
    });
    assert_eq!(settings.projects.len(), MAX_PROJECTS);
    assert_eq!(settings.projects[0].root, Path::new("p5"));
    let p5 = settings.project_state(Path::new("p5")).unwrap();
    assert_eq!(p5.main.as_deref(), Some(Path::new("p5/main.py")));
    assert_eq!(
        settings
            .projects
            .iter()
            .filter(|p| p.root == Path::new("p5"))
            .count(),
        1
    );
}

#[test]
fn what_is_remembered_of_a_file_follows_its_rename() {
    let mut settings = Settings::default();
    let old = Path::new("/p/parts/axle.py");
    settings.add_recent_file(old);
    settings.open_files = vec![old.into(), "/p/main.py".into()];
    settings.last_file = Some(old.into());
    settings.remember_breakpoints(
        old,
        vec![SavedBreakpoint {
            line: 2,
            text: "x = 1".into(),
        }],
    );
    settings.remember_project(ProjectState {
        root: "/p".into(),
        main: Some(old.into()),
        expanded: vec!["/p/parts".into()],
    });

    settings.relocate(Path::new("/p/parts"), Path::new("/p/components"));
    let new = Path::new("/p/components/axle.py");
    assert_eq!(settings.recent_files[0], new);
    assert_eq!(settings.open_files[0], new);
    assert_eq!(settings.open_files[1], Path::new("/p/main.py"));
    assert_eq!(settings.last_file.as_deref(), Some(new));
    assert_eq!(settings.breakpoints_of(new).len(), 1);
    let project = settings.project_state(Path::new("/p")).unwrap();
    assert_eq!(project.main.as_deref(), Some(new));
    assert_eq!(project.expanded, [PathBuf::from("/p/components")]);
}
