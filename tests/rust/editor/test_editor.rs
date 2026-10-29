use super::*;

#[test]
fn finds_matches_with_and_without_case() {
    let text = "Box box BOX";
    assert_eq!(find_all(text, "box", true, false), vec![4..7]);
    assert_eq!(find_all(text, "box", false, false), vec![0..3, 4..7, 8..11]);
    assert_eq!(find_all("aé é", "É", false, false), vec![1..3, 4..6]);
    assert!(find_all(text, "", false, false).is_empty());
}

#[test]
fn whole_word_search_skips_parts_of_longer_names() {
    let text = "box = boxes(box_2) + box";
    assert_eq!(find_all(text, "box", true, false).len(), 4);
    assert_eq!(find_all(text, "box", true, true), vec![0..3, 21..24]);
    // Accented letters are name characters too.
    assert!(find_all("caixaé", "caixa", true, true).is_empty());
}

#[test]
fn the_first_search_step_goes_to_the_first_match_after_the_caret() {
    let matches = [4..7, 10..13, 20..23];
    // The query was just typed; nothing is selected yet.
    assert_eq!(next_match(&matches, &(0..0), true), Some(0));
    assert_eq!(next_match(&matches, &(4..7), true), Some(1));
    assert_eq!(next_match(&matches, &(20..23), true), Some(0), "wraps");
    assert_eq!(next_match(&matches, &(15..15), true), Some(2));
    assert_eq!(next_match(&matches, &(15..15), false), Some(1));
    assert_eq!(next_match(&matches, &(4..7), false), Some(2), "wraps");
    assert_eq!(next_match(&[], &(0..0), true), None);
}

#[test]
fn completion_is_offered_once_a_name_reaches_the_threshold() {
    assert!(completes_name("res = cq.Wor", 12, 3));
    assert!(!completes_name("res = cq.Wo", 11, 3), "too short");
    assert!(
        !completes_name("res = cq.Work", 13, 3),
        "already offered at 3"
    );
    assert!(!completes_name("x = 123", 7, 3), "a number");
    assert!(!completes_name("box", 3, 0), "turned off");
    assert!(!completes_name("boxes", 3, 3), "inside a longer name");
    assert!(!completes_name("# Exa", 5, 3), "a comment");
    assert!(
        !completes_name("name = 'wor", 11, 3),
        "a string being typed"
    );
    assert!(
        !completes_name("doc = '''one\ntwo", 16, 3),
        "a docstring's next line"
    );
    assert!(
        completes_name("x = 'a' + sho", 13, 3),
        "after a closed string"
    );
}

#[test]
fn typed_text_is_empty_after_moving_back() {
    assert_eq!(typed_since("cq.Wor", 3, 6), "Wor");
    assert_eq!(typed_since("cq.Wor", 3, 2), "");
}

#[test]
fn breakpoints_and_the_error_line_follow_inserted_and_deleted_lines() {
    let mut editor = CodeEditor::default();
    editor.sync_markers("a = 1\nb = 2\nc = 3\n");
    editor.breakpoints.extend([2, 3]);
    editor.error_line = Some(3);

    editor.sync_markers("import x\na = 1\nb = 2\nc = 3\n");
    assert_eq!(
        editor.breakpoints.iter().copied().collect::<Vec<_>>(),
        [3, 4]
    );
    assert_eq!(editor.error_line, Some(4));

    // Deleting the line under a breakpoint removes the breakpoint.
    editor.sync_markers("import x\na = 1\nc = 3\n");
    assert_eq!(editor.breakpoints.iter().copied().collect::<Vec<_>>(), [3]);
    assert_eq!(editor.error_line, Some(3));
}

#[test]
fn a_new_document_does_not_inherit_line_moves() {
    let mut editor = CodeEditor::default();
    editor.sync_markers("a\nb\nc\n");
    editor.reset();
    editor.breakpoints.insert(2);

    // The first text after a reset is the reference, not an edit.
    editor.sync_markers("x\n");
    assert_eq!(editor.breakpoints.iter().copied().collect::<Vec<_>>(), [2]);
}

#[test]
fn completion_survives_edits_that_move_its_anchor_inside_a_character() {
    let mut editor = CodeEditor {
        completion: Completion {
            items: vec![CompletionItem {
                name: "box".to_owned(),
                insert: "box".to_owned(),
                kind: "function".to_owned(),
            }],
            // Byte 1 is inside "é" once the text below replaced the buffer.
            anchor: 1,
            open: true,
            ..Completion::default()
        },
        selection: Selection::caret(4),
        ..CodeEditor::default()
    };

    editor.close_popups_if_left("éé = 1");

    assert!(!editor.completion.open);
    assert_eq!(typed_since("éé", 1, 4), "");
    assert_eq!(typed_text("éé", 2, 4), Some("é"));
    assert_eq!(typed_text("abc", 1, 9), None);
}

fn document(text: &str) -> Document {
    Document::untitled(text)
}

#[test]
fn indent_comment_and_completion_are_undone_as_one_step_each() {
    let mut editor = CodeEditor::default();
    let mut doc = document("a = 1\nb = 2\n");

    editor.selection = Selection {
        anchor: 0,
        cursor: 9,
    };
    let indented = text_ops::indent(&doc.text, &editor.selection);
    editor.apply(&mut doc, indented);
    editor.toggle_comment(&mut doc);
    assert_eq!(doc.text, "    # a = 1\n    # b = 2\n");

    editor.completion = Completion {
        items: vec![CompletionItem {
            name: "box".to_owned(),
            insert: "box".to_owned(),
            kind: "function".to_owned(),
        }],
        anchor: doc.text.len(),
        open: true,
        ..Completion::default()
    };
    editor.selection = Selection::caret(doc.text.len());
    editor.accept_completion(&mut doc, 0);
    assert!(doc.text.ends_with("box"));

    editor.undo(&mut doc);
    assert_eq!(doc.text, "    # a = 1\n    # b = 2\n", "the completion");
    editor.undo(&mut doc);
    assert_eq!(doc.text, "    a = 1\n    b = 2\n", "the comment");
    editor.undo(&mut doc);
    assert_eq!(doc.text, "a = 1\nb = 2\n", "the indent");
    assert!(!editor.can_undo());

    editor.redo(&mut doc);
    assert_eq!(doc.text, "    a = 1\n    b = 2\n");
    assert!(editor.can_redo());
}

#[test]
fn inserted_text_replaces_the_selection_as_one_step() {
    let mut editor = CodeEditor::default();
    let mut doc = document("top = box.faces(\"<Z\")\n");

    // Replace the selector text, as Insert Selector does.
    let start = doc.text.find(".faces").unwrap();
    editor.selection = Selection {
        anchor: start,
        cursor: doc.text.len() - 1,
    };
    editor.insert_text(&mut doc, ".faces(\">Z\")");
    assert_eq!(doc.text, "top = box.faces(\">Z\")\n");
    assert_eq!(editor.selection, Selection::caret(doc.text.len() - 1));

    editor.undo(&mut doc);
    assert_eq!(doc.text, "top = box.faces(\"<Z\")\n");
    assert!(!editor.can_undo());
}

#[test]
fn undo_never_brings_back_another_document() {
    let mut editor = CodeEditor::default();
    let mut first = document("first");
    editor.selection = Selection::caret(5);
    editor.toggle_comment(&mut first);
    assert!(editor.can_undo());

    editor.reset();
    let mut second = document("second");
    editor.undo(&mut second);
    assert_eq!(second.text, "second");
    assert!(!editor.can_undo());
}

#[test]
fn a_reload_from_disk_is_an_undo_step_of_its_own() {
    let mut editor = CodeEditor::default();
    let mut doc = document("old");
    editor.record_outside_edits(&doc);
    doc.text = "new from disk".to_owned();

    editor.undo(&mut doc);
    assert_eq!(doc.text, "old");
    editor.redo(&mut doc);
    assert_eq!(doc.text, "new from disk");
}

#[test]
fn replace_takes_the_selected_match_and_replace_all_is_one_step() {
    let mut editor = CodeEditor::default();
    let mut doc = document("box(1)\nbox(2)\nbox(3)\n");
    editor.search.query = "box".to_owned();
    editor.search.replacement = "cylinder".to_owned();

    // Nothing selected yet: the first press only shows the match.
    editor.replace_current(&mut doc);
    assert_eq!(doc.text, "box(1)\nbox(2)\nbox(3)\n");
    let shown = editor.pending_selection.take().unwrap();
    assert_eq!(shown.range(), 0..3);
    // With the keyboard in the replace field, egui collapses the editor's
    // selection to its caret; the bar still knows which match it selected.
    editor.selection = Selection::caret(shown.cursor);

    editor.replace_current(&mut doc);
    assert_eq!(doc.text, "cylinder(1)\nbox(2)\nbox(3)\n");
    let next = editor.pending_selection.take().unwrap();
    assert_eq!(&doc.text[next.range()], "box");
    assert_eq!(next.range().start, 12, "then the next match");

    editor.selection = next;
    editor.replace_all(&mut doc);
    assert_eq!(doc.text, "cylinder(1)\ncylinder(2)\ncylinder(3)\n");
    editor.undo(&mut doc);
    assert_eq!(doc.text, "cylinder(1)\nbox(2)\nbox(3)\n");
}

#[test]
fn breakpoints_are_found_again_by_their_line_text() {
    let mut editor = CodeEditor::default();
    let text = "import cadquery as cq\nbox = cq.Workplane().box(1, 1, 1)\nshow_object(box)\n";
    editor.sync_markers(text);
    editor.breakpoints.extend([2, 3]);
    let saved = editor.saved_breakpoints(text);
    assert_eq!(saved[0].text, "box = cq.Workplane().box(1, 1, 1)");

    // Next session: two lines were added above in another editor.
    let changed = format!("# a comment\n\n{text}");
    let mut editor = CodeEditor::default();
    editor.restore_breakpoints(&changed, &saved);
    assert_eq!(
        editor.breakpoints.iter().copied().collect::<Vec<_>>(),
        [4, 5]
    );
    // The restored lines are the reference for later edits.
    editor.sync_markers(&changed);
    assert_eq!(editor.breakpoints.len(), 2);
}

#[test]
fn documentation_is_shown_only_for_the_name_still_under_the_pointer() {
    let mut editor = CodeEditor::default();
    let documentation = Documentation {
        signatures: vec!["box(length: float)".into()],
        text: "Return a 3d box.".into(),
    };
    editor.hover.target = Some((4..7, 0.0));
    editor.hover.requested = Some(4..7);
    editor.show_documentation(documentation.clone());
    assert_eq!(editor.hover.text, Some(documentation.clone()));

    // The pointer moved on before the answer came.
    editor.hover = Hover {
        target: Some((10..12, 1.0)),
        requested: Some(4..7),
        text: None,
    };
    editor.show_documentation(documentation);
    assert_eq!(editor.hover.text, None);
}

#[test]
fn adjusting_a_literal_keeps_the_caret_on_its_text_and_the_focus() {
    let mut editor = CodeEditor::default();
    let mut doc = document("width = 40\nbox(width)\n");
    editor.selection = Selection::caret(doc.text.len() - 1);

    editor.adjust_text(&mut doc, 8..10, "125", 1.0);
    editor.adjust_text(&mut doc, 8..11, "7", 1.1);
    assert_eq!(doc.text, "width = 7\nbox(width)\n");
    // The caret stays at the end of "box(width)".
    assert_eq!(editor.selection, Selection::caret(doc.text.len() - 1));
    assert!(!editor.pending_focus, "the panel keeps the keyboard");

    // The same value again changes nothing.
    editor.adjust_text(&mut doc, 8..9, "7", 1.2);
    editor.undo(&mut doc);
    assert_eq!(doc.text, "width = 40\nbox(width)\n");
    assert!(!editor.can_undo());
}

fn timing(seconds: f64) -> LineTiming {
    LineTiming {
        seconds,
        hits: 1,
        operations: Vec::new(),
    }
}

#[test]
fn profile_times_are_written_for_the_gutter() {
    assert_eq!(format_seconds(0.0004), "<1 ms");
    assert_eq!(format_seconds(0.0123), "12 ms");
    assert_eq!(format_seconds(0.9994), "999 ms");
    assert_eq!(format_seconds(0.9996), "1.0 s");
    assert_eq!(format_seconds(2.345), "2.3 s");
    assert_eq!(format_seconds(73.2), "73 s");
}

#[test]
fn profile_times_land_on_the_edited_text_and_follow_later_edits() {
    let run_text = "a = 1\nb = slow()\nc = 3\n";
    let mut editor = CodeEditor::default();
    editor.sync_markers(run_text);
    // Edited while it ran: a line was added on top.
    let now = "import x\na = 1\nb = slow()\nc = 3\n";
    let profile = [(1, timing(0.001)), (2, timing(2.5)), (3, timing(0.002))];
    editor.set_line_profile(profile.into_iter().collect(), run_text, now);
    assert_eq!(
        editor.line_profile.keys().copied().collect::<Vec<_>>(),
        [2, 3, 4]
    );
    assert_eq!(editor.line_profile[&3].seconds, 2.5);

    // Like breakpoints: they move with the lines, and go with them.
    editor.sync_markers("import x\n\na = 1\nb = slow()\nc = 3\n");
    assert_eq!(
        editor.line_profile.keys().copied().collect::<Vec<_>>(),
        [3, 4, 5]
    );
    editor.sync_markers("import x\n\na = 1\nc = 3\n");
    assert_eq!(
        editor.line_profile.keys().copied().collect::<Vec<_>>(),
        [3, 4]
    );
    editor.reset();
    assert!(editor.line_profile.is_empty());
}

/// The texts an editor paints for `document`.
fn painted_texts(editor: &mut CodeEditor, document: &mut Document) -> Vec<String> {
    let ctx = egui::Context::default();
    let colors = crate::theme::Palette::DARK.editor();
    let options = EditorOptions {
        font_size: 13.0,
        line_wrap: false,
        ruler: 0,
        highlight_current_line: true,
        auto_indent: true,
        completion_min_chars: 3,
        colors: &colors,
    };
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800.0, 400.0),
        )),
        ..Default::default()
    };
    let mut texts = Vec::new();
    // Twice: the first frame lays out the text the gutter follows.
    for _ in 0..2 {
        let mut output = ctx.run_ui(input.clone(), |ui| {
            editor.ui(ui, document, &options);
        });
        output.textures_delta.clear();
        texts.clear();
        fn collect(shape: &egui::Shape, texts: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(text) => texts.push(text.galley.text().to_owned()),
                egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| collect(s, texts)),
                _ => {}
            }
        }
        for clipped in &output.shapes {
            collect(&clipped.shape, &mut texts);
        }
    }
    texts
}

#[test]
fn the_gutter_shows_profiled_times_and_skips_the_fastest_lines() {
    let text = "import time\ntime.sleep(2.5)\nx = 1\n";
    let mut document = Document::untitled(text);
    let mut editor = CodeEditor::default();
    assert!(
        !painted_texts(&mut editor, &mut document)
            .iter()
            .any(|t| t.ends_with(" s"))
    );

    let profile = [(1, timing(0.0004)), (2, timing(2.5)), (3, timing(0.012))];
    editor.set_line_profile(profile.into_iter().collect(), text, text);
    let texts = painted_texts(&mut editor, &mut document);
    assert!(texts.iter().any(|t| t == "2.5 s"), "{texts:?}");
    assert!(texts.iter().any(|t| t == "12 ms"));
    assert!(
        !texts.iter().any(|t| t == "<1 ms"),
        "below a millisecond: blank"
    );
}

#[test]
fn moving_and_duplicating_lines_are_undoable_and_preserve_markers() {
    let initial = "a = 1\nb = 2\nc = 3\n";
    let mut document = Document::untitled(initial);
    let mut editor = CodeEditor::default();
    editor.breakpoints.insert(2);
    editor.error_line = Some(2);
    editor.debug_line = Some(3);
    editor.set_line_profile([(2, timing(1.2))].into_iter().collect(), initial, initial);

    editor.go_to_line(&document.text, 2);
    editor.selection = Selection::caret(text_ops::line_start(&document.text, 1));
    editor.move_lines(&mut document, false);
    assert_eq!(document.text, "a = 1\nc = 3\nb = 2\n");
    assert!(editor.breakpoints.contains(&3));
    assert_eq!(editor.error_line, Some(3));
    assert_eq!(editor.debug_line, Some(2));
    assert!(editor.line_profile.contains_key(&3));

    editor.undo(&mut document);
    assert_eq!(document.text, initial);
    assert!(editor.breakpoints.contains(&2));
    assert_eq!(editor.error_line, Some(2));
    assert_eq!(editor.debug_line, Some(3));
    assert!(editor.line_profile.contains_key(&2));

    editor.duplicate_line(&mut document);
    assert_eq!(document.text, "a = 1\nb = 2\nb = 2\nc = 3\n");
    editor.undo(&mut document);
    assert_eq!(document.text, initial);
}

#[test]
fn inserting_a_snippet_from_the_library_is_undoable() {
    let mut document = Document::untitled("");
    let mut editor = CodeEditor::default();
    let box_snippet = &crate::editor::snippets::SNIPPETS[1];
    editor.insert_snippet(&mut document, box_snippet);
    assert!(document.text.contains("import cadquery as cq"));
    assert!(document.text.contains(".box(40, 30, 10)"));
    editor.undo(&mut document);
    assert_eq!(document.text, "");
}
