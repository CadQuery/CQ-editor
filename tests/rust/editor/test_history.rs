use super::*;

fn caret(at: usize) -> Selection {
    Selection::caret(at)
}

/// A history that has seen `text` with the caret at its end.
fn seen(text: &str) -> History {
    let mut history = History::default();
    history.record(
        text,
        &caret(text.len()),
        &caret(text.len()),
        EditKind::Typing,
        0.0,
    );
    history
}

/// Types `typed` one character at a time at the end of `text`.
fn type_text(history: &mut History, text: &mut String, typed: &str, mut time: f64) -> f64 {
    for c in typed.chars() {
        let before = caret(text.len());
        text.push(c);
        history.record(text, &before, &caret(text.len()), EditKind::Typing, time);
        time += 0.1;
    }
    time
}

#[test]
fn the_first_record_is_the_starting_point_not_a_step() {
    let history = seen("box = 1\n");
    assert!(!history.can_undo() && !history.can_redo());
}

#[test]
fn continuous_typing_is_one_step() {
    let mut text = String::from("x = ");
    let mut history = seen(&text);
    type_text(&mut history, &mut text, "cq.Workplane", 1.0);

    let (text, selection) = history.undo(&text).unwrap();
    assert_eq!(text, "x = ");
    assert_eq!(selection, caret(4));
    assert!(!history.can_undo());
}

#[test]
fn a_pause_a_caret_move_or_a_new_line_starts_a_new_step() {
    let mut text = String::new();
    let mut history = seen(&text);
    let time = type_text(&mut history, &mut text, "ab", 1.0);
    // A pause.
    let time = type_text(&mut history, &mut text, "cd", time + 5.0);
    // A new line, then more typing.
    let time = type_text(&mut history, &mut text, "\nef", time);
    // A caret move: the next character is typed elsewhere.
    let before = caret(0);
    text.insert(0, '#');
    history.record(&text, &before, &caret(1), EditKind::Typing, time);

    let mut undone = Vec::new();
    while let Some((previous, _)) = history.undo(&text) {
        text = previous;
        undone.push(text.clone());
    }
    assert_eq!(undone, ["abcd\nef", "abcd\n", "abcd", "ab", ""]);
}

#[test]
fn deleting_after_typing_is_a_step_of_its_own() {
    let mut text = String::from("box");
    let mut history = seen(&text);
    let time = type_text(&mut history, &mut text, "es", 1.0);
    for _ in 0..2 {
        let before = caret(text.len());
        text.pop();
        history.record(&text, &before, &caret(text.len()), EditKind::Typing, time);
    }

    let (text, _) = history.undo(&text).unwrap();
    assert_eq!(text, "boxes", "both backspaces at once");
    let (text, _) = history.undo(&text).unwrap();
    assert_eq!(text, "box");
}

#[test]
fn commands_are_single_steps_even_amid_typing() {
    let mut text = String::from("a");
    let mut history = seen(&text);
    let time = type_text(&mut history, &mut text, "bc", 1.0);
    // An indent, at once, between keystrokes.
    let before = caret(text.len());
    text.insert_str(0, "    ");
    history.record(&text, &before, &caret(text.len()), EditKind::Command, time);
    type_text(&mut history, &mut text, "de", time + 0.1);

    let (text, selection) = history.undo(&text).unwrap();
    assert_eq!(text, "    abc");
    let (text, selection_after_indent) = history.undo(&text).unwrap();
    assert_eq!(text, "abc", "the indent alone");
    assert_eq!(selection_after_indent, caret(3));
    assert_eq!(selection, caret(7));
    let (text, _) = history.undo(&text).unwrap();
    assert_eq!(text, "a");
}

#[test]
fn a_paste_is_one_step() {
    let mut text = String::from("ab");
    let mut history = seen(&text);
    let before = caret(1);
    text.insert_str(1, "pasted");
    history.record(&text, &before, &caret(7), EditKind::Typing, 1.0);
    type_text(&mut history, &mut text, "x", 1.1);

    let (text, _) = history.undo(&text).unwrap();
    assert_eq!(text, "apastedb");
    let (text, selection) = history.undo(&text).unwrap();
    assert_eq!(text, "ab");
    assert_eq!(selection, caret(1));
}

#[test]
fn redo_reapplies_and_a_new_edit_drops_it() {
    let mut text = String::from("abc");
    let mut history = seen(&text);
    let before = caret(3);
    text = "# abc".to_owned();
    history.record(&text, &before, &caret(5), EditKind::Command, 1.0);

    let (undone, _) = history.undo(&text).unwrap();
    assert_eq!(undone, "abc");
    let (redone, selection) = history.redo(&undone).unwrap();
    assert_eq!(redone, "# abc");
    assert_eq!(selection, caret(5));

    let (mut text, _) = history.undo(&redone).unwrap();
    assert!(history.can_redo());
    type_text(&mut history, &mut text, "d", 2.0);
    assert!(!history.can_redo());
}

#[test]
fn typing_after_an_undo_starts_a_new_step() {
    let mut text = String::new();
    let mut history = seen(&text);
    type_text(&mut history, &mut text, "ab", 1.0);
    type_text(&mut history, &mut text, "c", 9.0);
    let (mut text, _) = history.undo(&text).unwrap();
    type_text(&mut history, &mut text, "x", 9.1);

    let (text, _) = history.undo(&text).unwrap();
    assert_eq!(text, "ab");
}

#[test]
fn undo_refuses_a_text_it_has_not_seen() {
    let mut text = String::from("a");
    let mut history = seen(&text);
    type_text(&mut history, &mut text, "b", 1.0);

    assert!(history.undo("changed elsewhere").is_none());
    assert!(history.can_undo());
}

#[test]
fn multibyte_characters_are_never_split() {
    let mut text = String::from("ação");
    let mut history = seen(&text);
    // "ã" → "á": both start with the same byte.
    let before = caret(3);
    text = "açáo".to_owned();
    history.record(&text, &before, &caret(5), EditKind::Command, 1.0);
    assert_eq!(
        diff("ação", "açáo"),
        Change {
            start: 3,
            removed: "ã".into(),
            inserted: "á".into()
        }
    );

    let (text, _) = history.undo(&text).unwrap();
    assert_eq!(text, "ação");
}

#[test]
fn clear_forgets_the_previous_document() {
    let mut text = String::from("first file");
    let mut history = seen(&text);
    type_text(&mut history, &mut text, "!", 1.0);
    history.clear();
    let other = "second file";
    history.record(other, &caret(0), &caret(0), EditKind::Command, 2.0);

    assert!(!history.can_undo());
    assert!(history.undo(other).is_none());
}

#[test]
fn old_steps_are_dropped_beyond_the_limit() {
    let mut text = String::new();
    let mut history = seen(&text);
    for step in 0..MAX_STEPS + 10 {
        let before = caret(text.len());
        text.push('x');
        history.record(
            &text,
            &before,
            &caret(text.len()),
            EditKind::Command,
            step as f64,
        );
    }
    let mut count = 0;
    while let Some((previous, _)) = history.undo(&text) {
        text = previous;
        count += 1;
    }
    assert_eq!(count, MAX_STEPS);
    assert_eq!(text.len(), 10);
}

/// Sets the literal at `start` to `value` as an adjustment.
fn adjust(
    history: &mut History,
    text: &mut String,
    start: usize,
    end: usize,
    value: &str,
    time: f64,
) -> usize {
    text.replace_range(start..end, value);
    history.record(text, &caret(0), &caret(0), EditKind::Adjust(start), time);
    start + value.len()
}

#[test]
fn successive_values_of_one_literal_are_one_step() {
    let mut text = String::from("width = 40\nheight = 12\n");
    let mut history = seen(&text);
    history.record(&text, &caret(0), &caret(0), EditKind::Command, 0.0);
    let mut end = 10;
    for (value, time) in [("41", 1.0), ("45", 1.2), ("100", 1.4), ("90", 1.6)] {
        end = adjust(&mut history, &mut text, 8, end, value, time);
    }
    assert_eq!(text, "width = 90\nheight = 12\n");

    let (undone, _) = history.undo(&text).unwrap();
    assert_eq!(undone, "width = 40\nheight = 12\n");
    assert!(!history.can_undo());
}

#[test]
fn another_literal_or_a_pause_starts_a_new_adjustment_step() {
    let mut text = String::from("width = 40\nheight = 12\n");
    let mut history = seen(&text);
    adjust(&mut history, &mut text, 8, 10, "41", 1.0);
    adjust(&mut history, &mut text, 20, 22, "13", 1.1);
    adjust(&mut history, &mut text, 20, 22, "14", 9.0);

    let (text, _) = history.undo(&text).unwrap();
    assert_eq!(text, "width = 41\nheight = 13\n", "after the pause");
    let (text, _) = history.undo(&text).unwrap();
    assert_eq!(text, "width = 41\nheight = 12\n", "the other literal");
    let (text, _) = history.undo(&text).unwrap();
    assert_eq!(text, "width = 40\nheight = 12\n");
}
