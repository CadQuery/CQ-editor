use super::*;

fn sel(anchor: usize, cursor: usize) -> Selection {
    Selection { anchor, cursor }
}

#[test]
fn line_helpers() {
    let text = "ab\ncd\n\nef";
    assert_eq!(line_of(text, 4), 1);
    assert_eq!(line_start(text, 2), 6);
    assert_eq!(line_range(text, 3), 7..9);
    assert_eq!(line_column("é\nxyz", 5), (1, 2));
    assert_eq!(char_to_byte("aé b", 2), 3);
    assert_eq!(byte_to_char("aé b", 3), 2);
}

#[test]
fn comments_a_single_line_and_moves_the_caret() {
    let edited = toggle_comment("    x = 1\n", &Selection::caret(6));
    assert_eq!(edited.text, "    # x = 1\n");
    assert_eq!(edited.selection, Selection::caret(8));
    let back = toggle_comment(&edited.text, &edited.selection);
    assert_eq!(back.text, "    x = 1\n");
    assert_eq!(back.selection, Selection::caret(6));
}

#[test]
fn comments_blocks_at_the_common_indentation() {
    let text = "if a:\n    b = 1\n\n    # c\n";
    let edited = toggle_comment(text, &sel(0, text.len()));
    assert_eq!(edited.text, "# if a:\n#     b = 1\n\n#     # c\n");
    // All lines commented now, so toggling again uncomments them.
    let back = toggle_comment(&edited.text, &sel(0, edited.text.len()));
    assert_eq!(back.text, text);
}

#[test]
fn selection_ending_at_line_start_excludes_that_line() {
    let text = "a\nb\nc\n";
    let edited = toggle_comment(text, &sel(0, 4));
    assert_eq!(edited.text, "# a\n# b\nc\n");
}

#[test]
fn indents_to_tab_stops_or_whole_lines() {
    let edited = indent("ab", &Selection::caret(1));
    assert_eq!(edited.text, "a   b");
    assert_eq!(edited.selection, Selection::caret(4));

    let edited = indent("a\n\nb\n", &sel(0, 4));
    assert_eq!(edited.text, "    a\n\n    b\n");
    // Insertions at a selection end push it forward.
    assert_eq!(edited.selection, sel(4, 12));
}

#[test]
fn unindents_to_the_previous_tab_stop() {
    let edited = unindent("      a\n\tb\nc", &sel(0, 11));
    assert_eq!(edited.text, "    a\nb\nc");
    let edited = unindent("        x", &Selection::caret(9));
    assert_eq!(edited.text, "    x");
    assert_eq!(edited.selection, Selection::caret(5));
}

#[test]
fn newline_indentation() {
    assert_eq!(newline_with_indent("def f(x):", 9), "\n    ");
    assert_eq!(newline_with_indent("    x = 1", 9), "\n    ");
    assert_eq!(newline_with_indent("    return x", 12), "\n");
    assert_eq!(
        newline_with_indent("for c in 'a:b':  # loop:", 24),
        "\n    "
    );
    assert_eq!(newline_with_indent("x = 'a#:'", 9), "\n");
}

#[test]
fn unicode_offsets_round_trip_at_character_boundaries() {
    let text = "á中\n🙂x";
    for index in 0..=text.chars().count() {
        let byte = char_to_byte(text, index);
        assert_eq!(byte_to_char(text, byte), index);
    }
    assert_eq!(line_column(text, char_to_byte(text, 4)), (1, 1));
    assert_eq!(char_to_byte(text, 100), text.len());
}

#[test]
fn commenting_blank_lines_is_a_noop() {
    let text = "  \n\n";
    let selection = sel(0, text.len());
    assert_eq!(
        toggle_comment(text, &selection),
        Edited {
            text: text.into(),
            selection,
        }
    );
}

#[test]
fn uncommenting_hash_without_space_only_removes_hash() {
    let edited = toggle_comment("  #value\n", &Selection::caret(4));
    assert_eq!(edited.text, "  value\n");
    assert_eq!(edited.selection, Selection::caret(3));
}

#[test]
fn indent_replaces_single_line_selection_at_next_tab_stop() {
    let edited = indent("abCD", &sel(2, 4));
    assert_eq!(edited.text, "ab  ");
    assert_eq!(edited.selection, Selection::caret(4));
}

#[test]
fn unindent_leaves_unindented_text_unchanged() {
    let text = "first\nsecond";
    let selection = sel(0, text.len());
    assert_eq!(
        unindent(text, &selection),
        Edited {
            text: text.into(),
            selection,
        }
    );
}

#[test]
fn newline_dedents_after_control_flow_statements() {
    for keyword in ["pass", "break", "continue", "raise", "return"] {
        let line = format!("        {keyword}");
        assert_eq!(newline_with_indent(&line, line.len()), "\n    ");
    }
    assert_eq!(newline_with_indent("    return_value", 16), "\n    ");
}

#[test]
fn newline_ignores_colons_inside_comments_and_strings() {
    let comment = "    value = 1  # note:";
    assert_eq!(newline_with_indent(comment, comment.len()), "\n    ");
    let string = "    label = 'x:'";
    assert_eq!(newline_with_indent(string, string.len()), "\n    ");
    let block = "    if ready: # note";
    assert_eq!(newline_with_indent(block, block.len()), "\n        ");
}

#[test]
fn line_map_keeps_lines_while_typing_inside_a_line() {
    let map = LineMap::new("a\nb\nc", "a\nbx\nc");
    assert_eq!([1, 2, 3].map(|l| map.map(l)), [Some(1), Some(2), Some(3)]);
}

#[test]
fn line_map_moves_lines_below_inserted_lines() {
    let map = LineMap::new("a\nb\nc", "a\nnew\nb\nc");
    assert_eq!([1, 2, 3].map(|l| map.map(l)), [Some(1), Some(3), Some(4)]);

    // Enter at the start of a line pushes it down; in its middle it stays.
    assert_eq!(LineMap::new("x\ny", "x\n\ny").map(2), Some(3));
    let split = LineMap::new("x = 1\ny", "x =\n 1\ny");
    assert_eq!((split.map(1), split.map(2)), (Some(1), Some(3)));
}

#[test]
fn line_map_follows_the_statement_rather_than_similar_text() {
    // The new line resembles the old one; the old line still moves down.
    let map = LineMap::new("y\nx = 1\nz", "y\nx = 0\nx = 1\nz");
    assert_eq!(map.map(2), Some(3));
}

#[test]
fn line_map_drops_deleted_lines_and_keeps_merged_ones() {
    let deleted = LineMap::new("a\nb\nc\nd", "a\nd");
    assert_eq!(
        [1, 2, 3, 4].map(|l| deleted.map(l)),
        [Some(1), None, None, Some(2)]
    );

    let merged = LineMap::new("a\nb\nc", "ab\nc");
    assert_eq!(
        [1, 2, 3].map(|l| merged.map(l)),
        [Some(1), Some(1), Some(2)]
    );
}

#[test]
fn line_map_keeps_positions_when_a_block_is_indented() {
    let map = LineMap::new("if x:\na\nb\nc", "if x:\n    a\n    b\nc");
    assert_eq!(
        [1, 2, 3, 4].map(|l| map.map(l)),
        [Some(1), Some(2), Some(3), Some(4)]
    );
}

#[test]
fn names_are_found_around_any_of_their_characters() {
    let text = "r = cq.Workplane().box(1)";
    assert_eq!(word_at(text, 7), Some(7..16));
    assert_eq!(word_at(text, 15), Some(7..16));
    assert_eq!(word_at(text, 6), None, "the dot");
    assert_eq!(word_at(text, text.len()), None);
    assert_eq!(word_at("área = 1", 2), Some(0..5), "accents are letters");
}

#[test]
fn the_name_before_the_caret_counts_only_at_its_end() {
    assert_eq!(name_before("x = cq.Wor", 10), "Wor");
    assert_eq!(name_before("x = cq.Workplane", 9), "", "inside the name");
    assert_eq!(name_before("x = ", 4), "");
    assert_eq!(name_before("éé", 1), "", "not a character boundary");
}

#[test]
fn go_to_line_accepts_a_line_and_an_optional_column() {
    let text = "first\nsecond line\nthird";
    assert_eq!(go_to_target(text, "2"), Some((1, 6)));
    assert_eq!(go_to_target(text, " 2:8 "), Some((1, 13)));
    assert_eq!(go_to_target(text, "2:99"), Some((1, 17)), "end of the line");
    assert_eq!(go_to_target(text, "40"), Some((2, 18)), "the last line");
    assert_eq!(go_to_target(text, "0"), None);
    assert_eq!(go_to_target(text, "two"), None);
    assert_eq!(go_to_target(text, ""), None);
}

#[test]
fn replacing_ranges_keeps_the_selection_on_the_same_text() {
    let edited = replace_ranges("a b a", &sel(4, 5), &[0..1, 4..5], "xyz");
    assert_eq!(edited.text, "xyz b xyz");
    assert_eq!(
        edited.selection,
        sel(6, 9),
        "the replacement of the selection"
    );
}

#[test]
fn breakpoints_are_relocated_by_the_text_of_their_line() {
    let text = "a = 1\nb = 2\n\nb = 2\nc = 3";
    let saved = |line: usize, content: &str| (line, content.to_owned());
    // Unchanged line.
    assert_eq!(relocate_lines(text, &[saved(5, "c = 3")]), [5].into());
    // Moved: the nearest line with that text wins.
    assert_eq!(relocate_lines(text, &[saved(5, "b = 2")]), [4].into());
    assert_eq!(relocate_lines(text, &[saved(1, "b = 2")]), [2].into());
    // Gone, or a blank line.
    assert!(relocate_lines(text, &[saved(1, "d = 4"), saved(3, "")]).is_empty());
}

#[test]
fn a_line_of_the_run_is_found_again_after_edits() {
    let ran = "import cadquery as cq\nplate = cq.Workplane().box(4, 4, 1)\nhole = plate.hole(1)\n";
    // Unchanged.
    assert_eq!(locate_line(ran, 2, ran), Some(Located::Unchanged(2)));
    // Lines added above move it.
    let added = format!("# A plate\n\n{ran}");
    assert_eq!(locate_line(ran, 2, &added), Some(Located::Moved(4)));
    // Its own text was edited: where the edit left it.
    let edited = ran.replace("box(4, 4, 1)", "box(5, 4, 1)");
    assert_eq!(locate_line(ran, 2, &edited), Some(Located::Changed(2)));
    // Moved elsewhere by a cut and paste: found by its text.
    let pasted =
        "import cadquery as cq\nhole = plate.hole(1)\nplate = cq.Workplane().box(4, 4, 1)\n";
    assert_eq!(locate_line(ran, 2, pasted), Some(Located::Moved(3)));
    // Deleted with its neighbours.
    assert_eq!(locate_line(ran, 2, "import cadquery as cq\n"), None);
    // Not a line of the run.
    assert_eq!(locate_line(ran, 0, ran), None);
    assert_eq!(locate_line(ran, 99, ran), None);
}

#[test]
fn a_repeated_line_is_found_nearest_to_where_it_was() {
    let ran = "a = f()\nb = 1\na = f()\nc = 2\n";
    let edited = "a = f()\nb = 10\nx = 0\na = f()\nc = 2\n";
    assert_eq!(locate_line(ran, 3, edited), Some(Located::Moved(4)));
    assert_eq!(locate_line(ran, 1, edited), Some(Located::Unchanged(1)));
}

#[test]
fn duplicating_lines_copies_the_line_or_block_and_keeps_the_selection_on_the_copy() {
    let edited = duplicate_lines("a = 1\nb = 2\n", &Selection::caret(2));
    assert_eq!(edited.text, "a = 1\na = 1\nb = 2\n");
    assert_eq!(edited.selection, Selection::caret(8));

    let block = "first\nsecond\nthird";
    let edited = duplicate_lines(block, &sel(1, 9));
    assert_eq!(edited.text, "first\nsecond\nfirst\nsecond\nthird");
    assert_eq!(&edited.text[edited.selection.range()], "irst\nsec");
}

#[test]
fn moving_lines_up_and_down_swaps_neighbours_and_preserves_selection() {
    let text = "line 1\nline 2\nline 3";
    let up = move_lines_up(text, &Selection::caret(9));
    assert_eq!(up.text, "line 2\nline 1\nline 3");
    assert_eq!(up.selection, Selection::caret(2));

    let down = move_lines_down(&up.text, &up.selection);
    assert_eq!(down.text, text);
    assert_eq!(down.selection, Selection::caret(9));

    // At the boundaries nothing changes.
    assert_eq!(move_lines_up(text, &Selection::caret(0)).text, text);
    assert_eq!(move_lines_down(text, &Selection::caret(16)).text, text);

    // Moving a multi-line block down.
    let block_down = move_lines_down("a\nb\nc\nd", &sel(0, 3));
    assert_eq!(block_down.text, "c\na\nb\nd");
    assert_eq!(&block_down.text[block_down.selection.range()], "a\nb");
}

#[test]
fn line_map_tracks_rotated_lines_when_moving_up_or_down() {
    let before = "alpha\nbeta\ngamma\ndelta\n";
    let moved_up = "alpha\ngamma\nbeta\ndelta\n";
    let map = LineMap::new(before, moved_up);
    assert_eq!(map.map(1), Some(1));
    assert_eq!(map.map(2), Some(3));
    assert_eq!(map.map(3), Some(2));
    assert_eq!(map.map(4), Some(4));
}

#[test]
fn inserting_snippets_adds_import_once_and_indents_inner_blocks() {
    let empty = insert_snippet(
        "",
        &Selection::caret(0),
        "box = cq.Workplane(\"XY\").box(10, 10, 10)",
        true,
    );
    assert_eq!(
        empty.text,
        "import cadquery as cq\n\nbox = cq.Workplane(\"XY\").box(10, 10, 10)"
    );

    let with_import = "import cadquery as cq\n\ndef build():\n    ";
    let indented = insert_snippet(
        with_import,
        &Selection::caret(with_import.len()),
        "wp = (\n    cq.Workplane(\"XY\")\n    .box(10, 10, 10)\n)",
        true,
    );
    assert_eq!(
        indented.text,
        "import cadquery as cq\n\ndef build():\n    wp = (\n        cq.Workplane(\"XY\")\n        .box(10, 10, 10)\n    )"
    );
    assert_eq!(indented.text.matches("import cadquery").count(), 1);
}
