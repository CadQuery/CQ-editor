use super::*;

fn names(text: &str) -> Vec<String> {
    parameters(text).into_iter().map(|p| p.name).collect()
}

fn only(text: &str) -> Parameter {
    let mut found = parameters(text);
    assert_eq!(found.len(), 1, "{found:?}");
    found.remove(0)
}

#[test]
fn top_level_number_assignments_are_parameters() {
    let text = "import cadquery as cq\n\
                width = 40\n\
                height=12.5\n\
                wall: float = 2.0  # thickness\n\
                count: int = 3\n\
                offset = -1.25\n\
                big = 1_000\n\
                tiny = 1e-3\n\
                half = .5\n\
                whole = 5.\n\
                result = cq.Workplane().box(width, width, height)\n";
    let found = parameters(text);
    let summary: Vec<(&str, f64, bool, usize)> = found
        .iter()
        .map(|p| (p.name.as_str(), p.value, p.integer, p.decimals))
        .collect();
    assert_eq!(
        summary,
        [
            ("width", 40.0, true, 0),
            ("height", 12.5, false, 1),
            ("wall", 2.0, false, 1),
            ("count", 3.0, true, 0),
            ("offset", -1.25, false, 2),
            ("big", 1000.0, true, 0),
            ("tiny", 0.001, false, 0),
            ("half", 0.5, false, 1),
            ("whole", 5.0, false, 0),
        ]
    );
    assert_eq!(found[0].line, 2);
    assert_eq!(&text[found[0].literal.clone()], "40");
    assert_eq!(&text[found[4].literal.clone()], "-1.25");
    assert!(found[6].exponent);
}

#[test]
fn nested_continued_and_non_literal_lines_are_skipped() {
    let text = "def part(size=3):\n    depth = 4\n    return size\n\
                if True:\n    inner = 1\n\
                doc = \"\"\"\nquoted = 5\n\"\"\"\n\
                call = dict(\n    keyword = 6,\n)\n\
                joined = 1 + \\\n    2\n\
                after = 7\n\
                expression = 1 + 2\n\
                chained = other = 8\n\
                several = 1; more = 2\n\
                compared == 9\n\
                flag = True\n\
                hexadecimal = 0x10\n\
                imaginary = 2j\n\
                text = \"10\"\n\
                annotated: str = 3\n\
                __version__ = 1\n\
                last = 10";
    assert_eq!(names(text), ["after", "last"]);
}

#[test]
fn strings_and_comments_do_not_open_brackets() {
    let text = "label = \"(\"\nfirst = 1\nnote = 'a [ b'  # also ( here\nsecond = 2\n\
                raw = r\"\\\"(\"\nthird = 3\n";
    assert_eq!(names(text), ["first", "second", "third"]);
}

#[test]
fn a_comment_can_give_the_slider_range_and_step() {
    assert_eq!(only("width = 40  # [10, 100]").range, Some(10.0..=100.0));
    let stepped = only("hole = 2.5 # diameter [0.5, 5, 0.25]");
    assert_eq!(stepped.range, Some(0.5..=5.0));
    assert_eq!(stepped.step, Some(0.25));
    assert_eq!(only("n = 3 # 1..12 copies").range, Some(1.0..=12.0));
    // Not a range: kept as a plain comment.
    assert_eq!(only("x = 3 # [12, 1]").range, None);
    assert_eq!(only("x = 3 # see [docs]").range, None);
}

#[test]
fn new_values_keep_the_style_of_the_literal() {
    assert_eq!(only("n = 3").literal_for(4.6), "5");
    assert_eq!(only("w = 40.0").literal_for(41.25), "41.2");
    assert_eq!(only("w = 2.50").literal_for(3.0), "3.00");
    assert_eq!(only("w = 2.5 # [0, 5, 0.25]").literal_for(3.75), "3.75");
    assert_eq!(only("t = 1e-3").literal_for(0.0025), "2.5e-3");
    assert_eq!(only("o = -1.5").literal_for(-0.0), "0.0");
    assert_eq!(only("o = 1.5").literal_for(-2.0), "-2.0");
    // Within the commented range.
    assert_eq!(only("w = 40 # [10, 100]").literal_for(250.0), "100");
}

#[test]
fn drag_speed_follows_the_written_precision() {
    assert_eq!(only("n = 3").drag_speed(), 1.0);
    assert_eq!(only("w = 2.50").drag_speed(), 0.01);
    assert_eq!(only("w = 2.5 # [0, 5, 0.25]").drag_speed(), 0.25);
    assert_eq!(only("big = 4000").drag_speed(), 20.0);
}

#[test]
fn windows_line_endings_are_parsed() {
    let text = "a = 1\r\nb = 2.5\r\n";
    let found = parameters(text);
    assert_eq!(names(text), ["a", "b"]);
    assert_eq!(&text[found[1].literal.clone()], "2.5");
}
