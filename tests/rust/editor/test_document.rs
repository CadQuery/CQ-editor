use super::*;

fn temporary_file(name: &str) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "cq-editor-document-test-{}-{}-{name}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn detects_line_endings() {
    assert_eq!(LineEnding::detect("a\r\nb"), LineEnding::CrLf);
    assert_eq!(LineEnding::detect("a\nb"), LineEnding::Lf);
    assert_eq!(LineEnding::detect("a\rb"), LineEnding::Cr);
}

#[test]
fn preserves_line_endings_and_tracks_modification() {
    let dir = std::env::temp_dir().join(format!("cq-editor-doc-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("script.py");
    std::fs::write(&path, "a = 1\r\nb = 2\r\n").unwrap();

    let mut doc = Document::open(&path).unwrap();
    assert_eq!(doc.text, "a = 1\nb = 2\n");
    assert!(!doc.is_modified());

    doc.text.push_str("c = 3\n");
    assert!(doc.is_modified());
    assert!(doc.save().unwrap());
    assert!(!doc.is_modified());
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "a = 1\r\nb = 2\r\nc = 3\r\n"
    );

    std::fs::write(&path, "x = 1\r\n").unwrap();
    let external = doc.disk_version().unwrap().unwrap();
    assert!(doc.reload(external).unwrap());
    assert_eq!(doc.text, "x = 1\n");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn examples_and_untitled_buffers_need_save_as() {
    let mut example = Document::example("Hello Box", "x = 1\n");
    assert!(!example.save().unwrap());
    assert_eq!(example.display_name(), "Hello Box (example)");
    assert!(!Document::untitled(DEFAULT_SCRIPT).is_modified());
}

#[test]
fn save_as_changes_an_example_into_an_editable_file() {
    let path = temporary_file("example.py");
    let mut doc = Document::example("A Box", "x = 1\r\ny = 2\r\n");
    assert_eq!(doc.text, "x = 1\ny = 2\n");
    doc.text.push_str("z = 3\n");
    doc.save_as(&path).unwrap();

    assert_eq!(doc.path(), Some(path.as_path()));
    assert_eq!(doc.example_title(), None);
    assert_eq!(
        doc.display_name(),
        path.file_name().unwrap().to_string_lossy()
    );
    assert!(!doc.is_modified());
    let eol = LineEnding::NATIVE.as_str();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        format!("x = 1{eol}y = 2{eol}z = 3{eol}")
    );
    assert!(doc.save().unwrap());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn opens_and_saves_classic_mac_line_endings() {
    let path = temporary_file("cr.py");
    std::fs::write(&path, "first\rsecond\r").unwrap();
    let mut doc = Document::open(&path).unwrap();
    assert_eq!(doc.line_ending(), LineEnding::Cr);
    assert_eq!(doc.text, "first\nsecond\n");
    doc.text.push_str("third\n");
    assert!(doc.save().unwrap());
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "first\rsecond\rthird\r"
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn preserves_a_utf8_bom_without_exposing_it_in_the_editor() {
    let path = temporary_file("bom.py");
    std::fs::write(&path, b"\xef\xbb\xbfx = 1\r\n").unwrap();
    let mut doc = Document::open(&path).unwrap();
    assert_eq!(doc.text, "x = 1\n");
    assert!(doc.has_utf8_bom());

    doc.text.push_str("y = 2\n");
    assert!(doc.save().unwrap());
    assert_eq!(
        std::fs::read(&path).unwrap(),
        b"\xef\xbb\xbfx = 1\r\ny = 2\r\n"
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn reload_refuses_to_discard_unsaved_changes() {
    let path = temporary_file("reload.py");
    std::fs::write(&path, "x = 1\n").unwrap();
    let mut doc = Document::open(&path).unwrap();
    let unchanged = doc.disk_version().unwrap().unwrap();
    assert!(!doc.reload(unchanged).unwrap());
    doc.text = "unsaved change".into();
    assert!(doc.is_modified());
    std::fs::write(&path, "external change\n").unwrap();

    let disk = doc.disk_version().unwrap().unwrap();
    let error = doc.reload(disk.clone()).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
    assert_eq!(doc.text, "unsaved change");
    assert!(doc.is_modified());

    assert!(!doc.disk_matches_saved(&disk));
    assert!(doc.apply_disk_version(disk).unwrap());
    assert_eq!(doc.text, "external change\n");
    assert!(!doc.is_modified());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn classifies_external_updates_without_mutating_the_buffer() {
    let path = temporary_file("external-update.py");
    std::fs::write(&path, "original\n").unwrap();
    let mut doc = Document::open(&path).unwrap();

    let unchanged = doc.disk_version().unwrap().unwrap();
    assert_eq!(
        doc.classify_external_update(&unchanged),
        ExternalUpdate::Unchanged
    );

    std::fs::write(&path, "external\n").unwrap();
    let external = doc.disk_version().unwrap().unwrap();
    assert_eq!(
        doc.classify_external_update(&external),
        ExternalUpdate::Reload
    );
    assert_eq!(doc.text, "original\n");

    doc.text = "local\n".into();
    assert_eq!(
        doc.classify_external_update(&external),
        ExternalUpdate::Conflict
    );
    assert_eq!(doc.text, "local\n");

    std::fs::write(&path, "local\n").unwrap();
    let matching = doc.disk_version().unwrap().unwrap();
    assert_eq!(
        doc.classify_external_update(&matching),
        ExternalUpdate::MatchesBuffer
    );
    assert_eq!(doc.text, "local\n");
    assert!(doc.is_modified());

    std::fs::remove_file(path).unwrap();
}

#[test]
fn a_save_finds_the_version_another_program_wrote() {
    let path = temporary_file("unseen.py");
    std::fs::write(&path, "original\n").unwrap();
    let mut doc = Document::open(&path).unwrap();
    assert!(doc.unseen_disk_version().unwrap().is_none());

    // Even an unedited buffer would replace it.
    std::fs::write(&path, "theirs\n").unwrap();
    let unseen = doc.unseen_disk_version().unwrap().unwrap();
    assert_eq!(unseen.text, "theirs\n");

    // Contents equal to the buffer lose nothing.
    doc.text = "theirs\n".into();
    assert!(doc.unseen_disk_version().unwrap().is_none());

    // Once seen and kept, the buffer stays modified and saving goes ahead.
    doc.text = "mine\n".into();
    doc.adopt_disk_baseline(&unseen);
    assert!(doc.is_modified());
    assert!(doc.unseen_disk_version().unwrap().is_none());
    assert!(doc.save().unwrap());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "mine\n");

    // A deleted file is simply written again.
    std::fs::remove_file(&path).unwrap();
    assert!(doc.unseen_disk_version().unwrap().is_none());
    assert!(
        Document::untitled("x")
            .unseen_disk_version()
            .unwrap()
            .is_none()
    );
}

#[test]
fn recovery_backup_preserves_edits_without_marking_them_saved() {
    let path = temporary_file("recovery.py");
    let recovery_dir = temporary_file("recovery-dir");
    std::fs::write(&path, "original\r\n").unwrap();
    let mut doc = Document::open(&path).unwrap();
    doc.text = "local edit\nsecond line\n".into();

    let backup = doc.create_recovery_backup(&recovery_dir).unwrap();
    assert_eq!(
        std::fs::read_to_string(&backup).unwrap(),
        "local edit\r\nsecond line\r\n"
    );
    assert_eq!(doc.path(), Some(path.as_path()));
    assert!(doc.is_modified());

    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir_all(recovery_dir).unwrap();
}

#[test]
fn saving_a_copy_does_not_change_document_identity_or_saved_state() {
    let path = temporary_file("source.py");
    let copy = temporary_file("copy.py");
    std::fs::write(&path, "original\n").unwrap();
    let mut doc = Document::open(&path).unwrap();
    doc.text = "local edit\n".into();

    doc.save_copy(&copy).unwrap();
    assert_eq!(std::fs::read_to_string(&copy).unwrap(), "local edit\n");
    assert_eq!(doc.path(), Some(path.as_path()));
    assert!(doc.is_modified());
    assert_eq!(
        doc.save_copy(&path).unwrap_err().kind(),
        std::io::ErrorKind::InvalidInput
    );

    std::fs::remove_file(path).unwrap();
    std::fs::remove_file(copy).unwrap();
}

#[test]
fn applying_a_disk_snapshot_rejects_another_document() {
    let first = temporary_file("first.py");
    let second = temporary_file("second.py");
    std::fs::write(&first, "first\n").unwrap();
    std::fs::write(&second, "second\n").unwrap();
    let snapshot = Document::open(&first)
        .unwrap()
        .disk_version()
        .unwrap()
        .unwrap();
    let mut doc = Document::open(&second).unwrap();

    assert_eq!(
        doc.apply_disk_version(snapshot).unwrap_err().kind(),
        std::io::ErrorKind::InvalidInput
    );
    assert_eq!(doc.text, "second\n");
    std::fs::remove_file(first).unwrap();
    std::fs::remove_file(second).unwrap();
}

#[test]
fn invalid_utf8_is_rejected_without_changing_the_file() {
    let path = temporary_file("invalid.py");
    // Latin-1 "café" is intentionally not valid UTF-8.
    let original = [b'c', b'a', b'f', 0xe9, b'\n'];
    std::fs::write(&path, original).unwrap();
    let error = Document::open(&path).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    assert!(error.to_string().contains("not valid UTF-8"));
    assert!(is_not_utf8(&error), "the GUI can offer another encoding");
    let missing = Document::open(&temporary_file("missing.py")).unwrap_err();
    assert!(!is_not_utf8(&missing));
    assert_eq!(std::fs::read(&path).unwrap(), original);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn windows_1252_files_open_on_request_and_are_saved_as_utf8() {
    let path = temporary_file("legacy.py");
    // "café € “x”" in Windows-1252, with Windows line endings.
    let original = b"s = 'caf\xe9 \x80 \x93x\x94'\r\nshow_object(s)\r\n";
    std::fs::write(&path, original).unwrap();

    let mut document = Document::open_with(&path, Encoding::Windows1252).unwrap();
    assert_eq!(document.text, "s = 'café € “x”'\nshow_object(s)\n");
    assert_eq!(document.encoding(), Encoding::Windows1252);
    assert_eq!(document.line_ending(), LineEnding::CrLf);
    assert!(!document.is_modified());
    // The file watcher keeps reading it the same way.
    let disk = document.disk_version().unwrap().unwrap();
    assert_eq!(
        document.classify_external_update(&disk),
        ExternalUpdate::Unchanged
    );
    assert_eq!(
        std::fs::read(&path).unwrap(),
        original,
        "opening never writes"
    );

    assert!(document.save().unwrap());
    assert_eq!(document.encoding(), Encoding::Utf8);
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "s = 'café € “x”'\r\nshow_object(s)\r\n"
    );
    assert!(!document.is_modified());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn windows_1252_decodes_every_byte() {
    let all: Vec<u8> = (0..=255).collect();
    let text = decode_windows_1252(&all);
    assert_eq!(text.chars().count(), 256);
    assert!(text.starts_with("\0\u{1}"));
    let high: Vec<char> = text.chars().skip(0x80).take(4).collect();
    assert_eq!(high, ['€', '\u{81}', '‚', 'ƒ']);
    assert_eq!(text.chars().nth(0xe9), Some('é'));
    assert_eq!(text.chars().last(), Some('ÿ'));
}

#[test]
fn a_recovered_legacy_file_keeps_its_encoding_for_the_watcher() {
    let path = temporary_file("recovered-legacy.py");
    std::fs::write(&path, b"x = '\xe9'\n").unwrap();
    let document = Document::recovered(
        "x = 'é'\ny = 1\n".to_owned(),
        Some(path.clone()),
        LineEnding::Lf,
        false,
    );
    assert_eq!(document.encoding(), Encoding::Windows1252);
    assert!(document.is_modified());
    assert!(document.disk_version().unwrap().is_some());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn interrupted_atomic_write_keeps_the_original_file_intact() {
    let directory = temporary_file("atomic-write-dir");
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("script.py");
    std::fs::write(&path, "original\n").unwrap();

    let error = atomic_write_with(&path, true, |file| {
        file.write_all(b"partial replacement")?;
        Err(std::io::Error::new(
            std::io::ErrorKind::Interrupted,
            "simulated interruption",
        ))
    })
    .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::Interrupted);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "original\n");
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);

    std::fs::remove_dir_all(directory).unwrap();
}

#[cfg(unix)]
#[test]
fn atomic_save_preserves_unix_permissions() {
    use std::os::unix::fs::PermissionsExt as _;

    let path = temporary_file("permissions.py");
    std::fs::write(&path, "original\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
    let mut doc = Document::open(&path).unwrap();
    doc.text = "replacement\n".into();
    doc.save().unwrap();

    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o640
    );
    std::fs::remove_file(path).unwrap();
}

#[cfg(windows)]
#[test]
fn atomic_save_preserves_windows_creation_time() {
    let path = temporary_file("windows-metadata.py");
    std::fs::write(&path, "original\n").unwrap();
    let created = std::fs::metadata(&path).unwrap().created().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    let mut doc = Document::open(&path).unwrap();
    doc.text = "replacement\n".into();
    doc.save().unwrap();

    assert_eq!(
        std::fs::metadata(&path).unwrap().created().unwrap(),
        created
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn missing_file_returns_an_error() {
    let path = temporary_file("absent.py");
    assert!(Document::open(&path).is_err());
}
