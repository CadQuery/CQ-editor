use super::*;

fn temporary_directory(name: &str) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "cq-editor-recovery-test-{}-{}-{name}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn checkpoint_restores_unsaved_work_after_a_simulated_crash() {
    let root = temporary_directory("crash");
    let source = root.join("source.py");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(&source, "saved\n").unwrap();
    let mut document = Document::open(&source).unwrap();
    document.text = "unsaved work\n".into();

    let mut running_store = RecoveryStore::new(root.join("store"));
    assert!(running_store.checkpoint(&document).unwrap());
    drop(running_store);

    let restarted_store = RecoveryStore::new(root.join("store"));
    let mut session = restarted_store.load_session().unwrap();
    assert_eq!(session.len(), 1);
    let snapshot = session.remove(0);
    assert_eq!(snapshot.path.as_deref(), Some(source.as_path()));
    assert_eq!(snapshot.text, "unsaved work\n");
    let recovered = snapshot.into_document();
    assert_eq!(recovered.text, "unsaved work\n");
    assert!(recovered.is_modified());

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn checkpoint_recovers_an_unsaved_untitled_document() {
    let root = temporary_directory("untitled");
    let mut document = Document::untitled("initial\n");
    document.text = "draft without a file\n".into();

    let mut running_store = RecoveryStore::new(root.clone());
    running_store.checkpoint(&document).unwrap();
    drop(running_store);

    let restarted_store = RecoveryStore::new(root.clone());
    let recovered = restarted_store.load_session().unwrap()[0]
        .clone()
        .into_document();
    assert_eq!(recovered.path(), None);
    assert_eq!(recovered.text, "draft without a file\n");
    assert!(recovered.is_modified());

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn checkpoints_are_deduplicated_and_available_as_local_history() {
    let root = temporary_directory("history");
    let source = root.join("source.py");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(&source, "saved\n").unwrap();
    let mut document = Document::open(&source).unwrap();
    let mut store = RecoveryStore::new(root.join("store"));

    document.text = "version one\n".into();
    assert!(store.checkpoint(&document).unwrap());
    assert!(!store.checkpoint(&document).unwrap());
    document.text = "version two\n".into();
    assert!(store.checkpoint(&document).unwrap());

    let history = store.history(Some(&source)).unwrap();
    assert_eq!(history.len(), 3);
    assert_eq!(history[0].text, "version two\n");
    assert!(history.iter().any(|entry| entry.text == "saved\n"));
    assert!(history.iter().any(|entry| entry.text == "version one\n"));
    assert!(history.iter().any(|entry| entry.text == "version two\n"));

    store.clear_session().unwrap();
    assert!(store.load_session().unwrap().is_empty());
    assert_eq!(store.history(Some(&source)).unwrap().len(), 3);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn local_history_is_bounded_per_document() {
    let root = temporary_directory("bounded");
    let source = root.join("source.py");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(&source, "saved\n").unwrap();
    let mut document = Document::open(&source).unwrap();
    let mut store = RecoveryStore::new(root.join("store"));

    for version in 0..MAX_HISTORY_PER_DOCUMENT + 5 {
        document.text = format!("version {version}\n");
        assert!(store.checkpoint(&document).unwrap());
    }

    assert_eq!(
        store.history(Some(&source)).unwrap().len(),
        MAX_HISTORY_PER_DOCUMENT
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_unmodified_document_clears_only_the_active_session() {
    let root = temporary_directory("clean");
    let source = root.join("source.py");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(&source, "saved\n").unwrap();
    let mut document = Document::open(&source).unwrap();
    let mut store = RecoveryStore::new(root.join("store"));

    document.text = "unsaved\n".into();
    store.checkpoint(&document).unwrap();
    let disk = document.disk_version().unwrap().unwrap();
    document.apply_disk_version(disk).unwrap();
    assert!(!store.checkpoint(&document).unwrap());
    assert!(store.load_session().unwrap().is_empty());
    assert_eq!(store.history(Some(&source)).unwrap().len(), 2);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn the_session_keeps_every_unsaved_tab_and_versions_only_what_changed() {
    let root = temporary_directory("tabs");
    let (first_path, mut first) = saved_document(&root, "first.py");
    let (second_path, mut second) = saved_document(&root, "second.py");
    let clean = Document::untitled("");
    let mut store = RecoveryStore::new(root.join("store"));

    first.text = "first edit\n".into();
    second.text = "second edit\n".into();
    assert!(store.checkpoint_all(&[&first, &clean, &second]).unwrap());
    assert!(!store.checkpoint_all(&[&first, &clean, &second]).unwrap());
    second.text = "second edit again\n".into();
    assert!(store.checkpoint_all(&[&first, &clean, &second]).unwrap());

    let session = RecoveryStore::new(root.join("store"))
        .load_session()
        .unwrap();
    let texts: Vec<&str> = session.iter().map(|s| s.text.as_str()).collect();
    assert_eq!(
        texts,
        ["first edit\n", "second edit again\n"],
        "in tab order"
    );
    assert_eq!(session[0].path.as_deref(), Some(first_path.as_path()));

    // The unchanged document got no second version: a baseline and an edit.
    assert_eq!(store.history(Some(&first_path)).unwrap().len(), 2);
    assert_eq!(store.history(Some(&second_path)).unwrap().len(), 3);

    // Saving one keeps the other in the session.
    let disk = first.disk_version().unwrap().unwrap();
    first.apply_disk_version(disk).unwrap();
    assert!(store.checkpoint_all(&[&first, &second]).unwrap());
    let session = store.load_session().unwrap();
    assert_eq!(session.len(), 1);
    assert_eq!(session[0].path.as_deref(), Some(second_path.as_path()));

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_session_of_an_earlier_release_is_still_recovered() {
    let root = temporary_directory("legacy-session");
    std::fs::create_dir_all(&root).unwrap();
    let legacy = snapshot(None, "draft\n", 1);
    std::fs::write(
        root.join("active-session.json"),
        serde_json::to_vec(&legacy).unwrap(),
    )
    .unwrap();
    let session = RecoveryStore::new(root.clone()).load_session().unwrap();
    assert_eq!(session.len(), 1);
    assert_eq!(session[0].text, "draft\n");
    std::fs::remove_dir_all(root).unwrap();
}

fn saved_document(root: &Path, name: &str) -> (PathBuf, Document) {
    std::fs::create_dir_all(root).unwrap();
    let source = root.join(name);
    std::fs::write(&source, "saved\n").unwrap();
    let document = Document::open(&source).unwrap();
    (source, document)
}

fn write_version(directory: &Path, snapshot: &RecoverySnapshot) -> PathBuf {
    std::fs::create_dir_all(directory).unwrap();
    let path = directory.join(version_name(snapshot.timestamp_ms, 0));
    std::fs::write(&path, serde_json::to_vec(snapshot).unwrap()).unwrap();
    path
}

fn snapshot(path: Option<&Path>, text: &str, timestamp_ms: u64) -> RecoverySnapshot {
    RecoverySnapshot {
        schema: SCHEMA_VERSION,
        timestamp_ms,
        path: path.map(Path::to_path_buf),
        text: text.to_owned(),
        line_ending: LineEnding::Lf,
        utf8_bom: false,
    }
}

#[test]
fn each_document_keeps_its_history_in_its_own_folder() {
    let root = temporary_directory("folders");
    let (first_path, mut first) = saved_document(&root, "first.py");
    let (second_path, mut second) = saved_document(&root, "second.py");
    let mut untitled = Document::untitled("");
    let mut store = RecoveryStore::new(root.join("store"));

    // Like the app, which clears the session whenever the document changes.
    first.text = "first edit\n".into();
    store.checkpoint(&first).unwrap();
    store.clear_session().unwrap();
    second.text = "second edit\n".into();
    store.checkpoint(&second).unwrap();
    store.clear_session().unwrap();
    untitled.text = "draft\n".into();
    store.checkpoint(&untitled).unwrap();

    let history = root.join("store").join("history");
    let folder = |path: Option<&Path>| history.join(document_folder(path));
    // A baseline and an edit each; a checkpoint only lists its own folder.
    assert_eq!(version_files(&folder(Some(&first_path))).unwrap().len(), 2);
    assert_eq!(version_files(&folder(Some(&second_path))).unwrap().len(), 2);
    assert_eq!(version_files(&folder(None)).unwrap().len(), 1);
    assert!(version_files(&history).unwrap().is_empty(), "no flat files");

    let mut texts = |path| -> Vec<String> {
        store
            .history(path)
            .unwrap()
            .into_iter()
            .map(|version| version.text)
            .collect()
    };
    assert_eq!(texts(Some(&first_path)), ["first edit\n", "saved\n"]);
    assert_eq!(texts(None), ["draft\n"]);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn folder_names_are_stable_across_releases() {
    // The paths of the system's file dialogs, as earlier releases hashed them.
    #[cfg(not(windows))]
    let (native, folder) = ("scripts/part.py", "doc-58d2002fe837b7f2");
    #[cfg(windows)]
    let (native, folder) = ("scripts\\part.py", "doc-d3fbd233f36653b9");
    assert_eq!(document_folder(Some(Path::new(native))), folder);
    assert_eq!(document_folder(None), "untitled");
    assert_eq!(
        version_timestamp("version-1700000000000-3.json"),
        Some(1_700_000_000_000)
    );
    assert_eq!(version_timestamp("notes.json"), None);
}

#[test]
fn a_file_has_one_history_however_its_path_is_written() {
    let written = Path::new("scripts").join("part.py");
    assert_eq!(
        document_folder(Some(Path::new("scripts/part.py"))),
        document_folder(Some(&written))
    );
    assert_eq!(
        document_folder(Some(Path::new("scripts/./part.py"))),
        document_folder(Some(&written))
    );
    #[cfg(windows)]
    assert_eq!(
        document_folder(Some(Path::new(r"C:/Users/me/part.py"))),
        document_folder(Some(Path::new(r"C:\Users\me\part.py")))
    );
}

#[test]
fn flat_history_of_earlier_releases_is_migrated() {
    let root = temporary_directory("migrate");
    let (source, document) = saved_document(&root, "legacy.py");
    let history = root.join("store").join("history");
    let old = write_version(
        &history,
        &snapshot(Some(&source), "old version\n", timestamp_ms()),
    );

    let mut store = RecoveryStore::new(root.join("store"));
    let versions = store.history(document.path()).unwrap();
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].text, "old version\n");
    assert!(!old.exists(), "moved into the document's folder");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn old_versions_and_histories_of_deleted_files_expire() {
    let root = temporary_directory("expire");
    let store = RecoveryStore::new(root.clone());
    let now = timestamp_ms();
    let expired_ms = now - MAX_HISTORY_AGE.as_millis() as u64 - 1;
    let deleted = Path::new("deleted-long-ago.py");
    let kept = Path::new("still-edited.py");
    let deleted_dir = store.document_dir(Some(deleted));
    let kept_dir = store.document_dir(Some(kept));
    write_version(&deleted_dir, &snapshot(Some(deleted), "gone\n", expired_ms));
    let old = write_version(&kept_dir, &snapshot(Some(kept), "old\n", expired_ms));
    let recent = write_version(&kept_dir, &snapshot(Some(kept), "recent\n", now));

    store.prune_all(now).unwrap();

    assert!(!deleted_dir.exists(), "an emptied folder is removed");
    assert!(!old.exists());
    assert!(recent.exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn the_oldest_versions_go_first_beyond_the_size_budget() {
    let root = temporary_directory("budget");
    let mut store = RecoveryStore::new(root.clone());
    let now = timestamp_ms();
    let text = "x".repeat(1000);
    let files: Vec<PathBuf> = (0..6)
        .map(|age| {
            let path = PathBuf::from(format!("part-{age}.py"));
            let version = snapshot(Some(&path), &text, now - 1000 * age);
            write_version(&store.document_dir(Some(&path)), &version)
        })
        .collect();
    let size = std::fs::metadata(&files[0]).unwrap().len();
    store.max_history_bytes = 3 * size;

    store.prune_all(now).unwrap();

    let remaining: Vec<bool> = files.iter().map(|path| path.exists()).collect();
    assert_eq!(remaining, [true, true, true, false, false, false]);
    std::fs::remove_dir_all(root).unwrap();
}
