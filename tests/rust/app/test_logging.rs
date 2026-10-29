use super::*;

#[test]
fn strips_color_codes() {
    assert_eq!(strip_ansi("\x1b[31mred\x1b[0m text"), "red text");
    assert_eq!(strip_ansi("plain"), "plain");
}

#[test]
fn splits_multiline_messages() {
    let before = with_entries(|e| e.len());
    push(Level::Info, "test", "one\ntwo\r\n");
    let lines: Vec<String> =
        with_entries(|e| e.iter().skip(before).map(|e| e.text.clone()).collect());
    // Other tests log concurrently into the same global log.
    assert!(
        lines.windows(2).any(|pair| pair == ["one", "two"]),
        "{lines:?}"
    );
}

fn temporary_directory(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "cq-editor-logging-test-{}-{name}",
        std::process::id()
    ))
}

#[test]
fn the_log_file_rotates_and_keeps_one_previous_file() {
    let directory = temporary_directory("rotate");
    let _ = std::fs::remove_dir_all(&directory);
    let mut file = LogFile::open(&directory).unwrap();
    let line = format!("{}\n", "x".repeat(1023));
    for _ in 0..MAX_FILE_BYTES / 1024 + 10 {
        file.write_line(&line, false);
    }
    file.flush();

    let size = |name| std::fs::metadata(directory.join(name)).unwrap().len();
    assert!(size(PREVIOUS_LOG_FILE) >= MAX_FILE_BYTES);
    assert!((1..MAX_FILE_BYTES).contains(&size(LOG_FILE)));

    // A new session appends to the current file.
    drop(file);
    let reopened = LogFile::open(&directory).unwrap();
    assert_eq!(reopened.size, size(LOG_FILE));
    drop(reopened);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn messages_reach_the_log_file_with_their_date_and_level() {
    let directory = temporary_directory("push");
    let _ = std::fs::remove_dir_all(&directory);
    lock().file = Some(LogFile::open(&directory).unwrap());
    push(
        Level::Warn,
        "python",
        "worker \x1b[33mwarning\x1b[0m\nsecond line",
    );
    let file = lock().file.take();
    drop(file);

    let text = std::fs::read_to_string(directory.join(LOG_FILE)).unwrap();
    let lines: Vec<&str> = text.lines().filter(|l| l.contains("python:")).collect();
    assert_eq!(lines.len(), 2, "{text}");
    assert!(lines[0].ends_with("WARN python: worker warning"));
    assert!(lines[1].ends_with("WARN python: second line"));
    // "YYYY-MM-DD HH:MM:SS " in front.
    assert_eq!(lines[0].as_bytes()[4], b'-');
    assert_eq!(lines[0].as_bytes()[19], b' ');
    std::fs::remove_dir_all(directory).unwrap();
}
