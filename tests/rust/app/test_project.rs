use super::*;

fn temporary_folder(name: &str) -> PathBuf {
    let folder =
        std::env::temp_dir().join(format!("cq-editor-project-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

fn names(rows: &[Row]) -> Vec<(usize, &str)> {
    rows.iter()
        .map(|row| (row.depth, row.entry.name.as_str()))
        .collect()
}

#[test]
fn the_tree_lists_folders_first_and_hides_caches_and_hidden_files() {
    let root = temporary_folder("tree");
    std::fs::create_dir_all(root.join("parts/__pycache__")).unwrap();
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::write(root.join("parts/Bracket.py"), "").unwrap();
    std::fs::write(root.join("parts/axle.py"), "").unwrap();
    std::fs::write(root.join("parts/axle.cpython-312.pyc"), "").unwrap();
    std::fs::write(root.join("main.py"), "").unwrap();
    std::fs::write(root.join(".env"), "").unwrap();
    std::fs::write(root.join("Notes.md"), "").unwrap();

    let mut project = Project::open(&root).unwrap();
    assert_eq!(project.name(), root.file_name().unwrap().to_string_lossy());
    assert_eq!(
        names(&project.rows()),
        [(0, "parts"), (0, "main.py"), (0, "Notes.md")]
    );

    project.toggle(&root.join("parts"));
    let rows = project.rows();
    assert_eq!(
        names(&rows),
        [
            (0, "parts"),
            (1, "axle.py"),
            (1, "Bracket.py"),
            (0, "main.py"),
            (0, "Notes.md")
        ]
    );
    assert!(rows[0].expanded && rows[0].entry.is_dir);

    project.collapse_all();
    assert_eq!(project.rows().len(), 3);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn files_created_by_other_programs_appear_once_the_folder_changed() {
    let root = temporary_folder("listing");
    std::fs::write(root.join("a.py"), "").unwrap();
    let mut project = Project::open(&root).unwrap();
    assert_eq!(project.rows().len(), 1);

    std::fs::write(root.join("b.py"), "").unwrap();
    // Checked again once the interval passed, as the next frames would.
    for listing in project.listings.values_mut() {
        listing.checked -= RECHECK_INTERVAL;
        listing.modified = None;
    }
    assert_eq!(names(&project.rows()), [(0, "a.py"), (0, "b.py")]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn revealing_a_file_expands_the_folders_above_it() {
    let root = temporary_folder("reveal");
    std::fs::create_dir_all(root.join("a/b")).unwrap();
    std::fs::write(root.join("a/b/deep.py"), "").unwrap();
    let mut project = Project::open(&root).unwrap();
    project.reveal(&root.join("a/b/deep.py"));
    assert!(project.is_expanded(&root.join("a")));
    assert!(project.is_expanded(&root.join("a/b")));
    assert_eq!(names(&project.rows()), [(0, "a"), (1, "b"), (2, "deep.py")]);
    // Outside the project nothing changes.
    project.reveal(Path::new("/elsewhere/x.py"));
    assert_eq!(project.expanded().len(), 2);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_main_script_is_guessed_only_when_it_is_clear() {
    let root = temporary_folder("main");
    std::fs::write(root.join("helpers.py"), "def bolt(): ...\n").unwrap();
    std::fs::write(root.join("bracket.py"), "show_object(result)\n").unwrap();
    assert_eq!(guess_main(&root), Some(root.join("bracket.py")));

    std::fs::write(root.join("plate.py"), "show_object(plate)\n").unwrap();
    assert_eq!(guess_main(&root), None, "two scripts show objects");

    let named = root.join(format!(
        "{}.py",
        root.file_name().unwrap().to_string_lossy()
    ));
    std::fs::write(&named, "").unwrap();
    assert_eq!(guess_main(&root), Some(named), "named after the folder");

    std::fs::write(root.join("main.py"), "").unwrap();
    assert_eq!(guess_main(&root), Some(root.join("main.py")));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn names_that_some_systems_refuse_are_refused() {
    assert_eq!(check_name("  bracket.py "), Ok("bracket.py"));
    assert!(check_name("").is_err());
    assert!(check_name("..").is_err());
    assert!(check_name("parts/axle.py").is_err());
    assert!(check_name("a:b").is_err());
    assert!(check_name("trailing.").is_err());
    assert!(check_name("con.py").is_err());
    assert!(check_name("console.py").is_ok());
}

#[test]
fn files_and_folders_are_created_renamed_and_deleted_without_replacing_anything() {
    let root = temporary_folder("operations");
    let script = create_file(&root, "bracket").unwrap();
    assert_eq!(script, root.join("bracket.py"), "Python by default");
    assert!(script.is_file());
    let notes = create_file(&root, "notes.txt").unwrap();
    assert_eq!(notes, root.join("notes.txt"));
    let error = create_file(&root, "bracket.py").unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
    assert!(error.to_string().contains("bracket.py already exists"));

    let folder = create_folder(&root, "parts").unwrap();
    assert!(folder.is_dir());
    assert!(create_folder(&root, "parts").is_err());

    std::fs::write(&script, "x = 1\n").unwrap();
    assert!(rename(&script, "notes.txt").is_err(), "no replacing");
    let renamed = rename(&script, "plate.py").unwrap();
    assert_eq!(renamed, root.join("plate.py"));
    assert_eq!(std::fs::read_to_string(&renamed).unwrap(), "x = 1\n");
    assert!(!script.exists());
    // Only the case changes.
    let upper = rename(&renamed, "Plate.py").unwrap();
    assert_eq!(upper, root.join("Plate.py"));

    std::fs::write(folder.join("axle.py"), "").unwrap();
    delete(&folder).unwrap();
    assert!(!folder.exists());
    delete(&upper).unwrap();
    assert!(!upper.exists());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn changing_only_the_case_never_replaces_another_file() {
    let root = temporary_folder("case");
    std::fs::write(root.join("part.py"), "lower\n").unwrap();
    std::fs::write(root.join("Part.py"), "upper\n").unwrap();
    let files = std::fs::read_dir(&root).unwrap().count();

    let result = rename(&root.join("part.py"), "Part.py");
    if files == 2 {
        // Case matters here (Linux, case-sensitive folders): two files.
        let error = result.unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(
            std::fs::read_to_string(root.join("part.py")).unwrap(),
            "lower\n"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("Part.py")).unwrap(),
            "upper\n"
        );
    } else {
        // One file under either name: only its case changes.
        assert_eq!(result.unwrap(), root.join("Part.py"));
        let names: Vec<String> = std::fs::read_dir(&root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["Part.py"]);
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn paths_follow_a_renamed_folder() {
    let from = Path::new("/p/parts");
    let to = Path::new("/p/components");
    assert_eq!(relocated(from, from, to), Some(to.to_path_buf()));
    assert_eq!(
        relocated(Path::new("/p/parts/axle.py"), from, to),
        Some(PathBuf::from("/p/components/axle.py"))
    );
    assert_eq!(relocated(Path::new("/p/partsbin/x.py"), from, to), None);

    let root = temporary_folder("relocate");
    std::fs::create_dir_all(root.join("parts")).unwrap();
    let mut project = Project::open(&root).unwrap();
    project.main = Some(root.join("parts/main.py"));
    project.toggle(&root.join("parts"));
    project.relocate(&root.join("parts"), &root.join("components"));
    assert_eq!(project.main, Some(root.join("components/main.py")));
    assert!(project.is_expanded(&root.join("components")));

    project.forget(&root.join("components"));
    assert_eq!(project.main, None);
    assert!(project.expanded().is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn scripts_and_text_files_open_in_the_editor() {
    assert!(is_editable(Path::new("a/main.py")));
    assert!(is_editable(Path::new("README.MD")));
    assert!(!is_editable(Path::new("part.step")));
    assert!(!is_editable(Path::new("Makefile")));
    assert!(is_script(Path::new("main.PY")));
    assert!(!is_script(Path::new("notes.txt")));
}

#[test]
fn project_search_finds_files_inside_collapsed_folders_without_expanding_them() {
    let root = temporary_folder("search");
    std::fs::create_dir_all(root.join("assemblies/drive")).unwrap();
    std::fs::write(root.join("main.py"), "").unwrap();
    std::fs::write(root.join("gear.py"), "").unwrap();
    std::fs::write(root.join("assemblies/drive/gearbox.py"), "").unwrap();

    let mut project = Project::open(&root).unwrap();
    assert!(project.expanded().is_empty());

    let found = project.search_rows("gear");
    let found_paths: Vec<String> = found
        .iter()
        .map(|row| {
            project
                .relative(&row.entry.path)
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    assert_eq!(found_paths, ["assemblies/drive/gearbox.py", "gear.py"]);
    assert!(
        project.expanded().is_empty(),
        "searching does not mutate folder expansion state"
    );

    let by_folder = project.search_rows("drive/gear");
    assert_eq!(by_folder.len(), 1);
    assert_eq!(by_folder[0].entry.name, "gearbox.py");
    let _ = std::fs::remove_dir_all(&root);
}
