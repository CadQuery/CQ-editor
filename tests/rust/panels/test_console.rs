use super::*;

#[test]
fn completion_inserts_unique_matches_and_common_prefixes() {
    let mut console = ConsoleState {
        input: "cq.Work".to_owned(),
        ..Default::default()
    };
    console.complete(&[("Workplane".into(), "plane".into())], &[]);
    assert_eq!(console.input, "cq.Workplane");

    let mut console = ConsoleState::default();
    console.complete(
        &[
            ("box".into(), "ox".into()),
            ("bounding".into(), "ounding".into()),
        ],
        &[],
    );
    assert_eq!(console.input, "o");

    let mut console = ConsoleState::default();
    console.complete(
        &[("abc".into(), "abc".into()), ("xyz".into(), "xyz".into())],
        &[],
    );
    assert_eq!(console.entries[0].text, "abc  xyz\n");
}

fn picture(text: &str) -> DisplayOutput {
    DisplayOutput {
        id: 1,
        text: text.to_owned(),
        image: Some(DisplayImage {
            size: [1, 1],
            rgba: vec![255; 4],
        }),
    }
}

#[test]
fn rich_results_show_their_image_then_their_text() {
    let mut console = ConsoleState::default();
    console.push(EntryKind::Output, "printed ");
    console.push_display(picture("<Figure>"));
    console.push_display(DisplayOutput {
        id: 1,
        text: "42".to_owned(),
        image: None,
    });
    console.push(EntryKind::Output, "after");
    let entries: Vec<_> = console
        .entries
        .iter()
        .map(|e| (e.text.as_str(), e.image.is_some()))
        .collect();
    assert_eq!(
        entries,
        [
            ("printed ", false),
            ("<Figure>\n", true),
            ("42\n", false),
            ("after", false)
        ],
        "nothing is appended to an image's entry, nor after a result"
    );
    // An empty result adds nothing.
    console.push_display(DisplayOutput {
        id: 1,
        text: String::new(),
        image: None,
    });
    assert_eq!(console.entries.len(), 4);
}

#[test]
fn only_the_latest_images_are_kept() {
    let mut console = ConsoleState::default();
    for n in 0..MAX_IMAGES + 3 {
        console.push_display(picture(&n.to_string()));
    }
    let kept = console.entries.iter().filter(|e| e.image.is_some()).count();
    assert_eq!(kept, MAX_IMAGES);
    assert_eq!(console.entries[0].text, "[image]\n0\n");
    assert!(console.entries[3].image.is_some());
    assert!(console.entries.last().unwrap().image.is_some());
}

#[test]
fn the_image_memory_budget_drops_oldest_pixels() {
    let mut console = ConsoleState::default();
    console.push_display(picture("old"));
    console.push_display(picture("new"));

    console.trim_images(MAX_IMAGES, 4);

    assert!(console.entries[0].image.is_none());
    assert_eq!(console.entries[0].text, "[image]\nold\n");
    assert!(console.entries[1].image.is_some());
}

#[test]
fn images_are_uploaded_once_and_kept_between_frames() {
    let mut console = ConsoleState::default();
    console.push_display(DisplayOutput {
        id: 1,
        text: String::new(),
        image: Some(DisplayImage {
            size: [3, 2],
            rgba: vec![255; 24],
        }),
    });
    let ctx = egui::Context::default();
    let mut frame = || {
        let mut drawn = ctx.run_ui(egui::RawInput::default(), |ui| {
            let colors = Colors::new(ui.visuals());
            output(ui, &mut console, &colors);
        });
        let uploads = drawn
            .textures_delta
            .set
            .values()
            .flatten()
            .filter(|delta| delta.image.size() == [3, 2])
            .count();
        drawn.textures_delta.clear();
        uploads
    };
    assert_eq!(frame(), 1);
    assert_eq!(frame(), 0, "the texture is kept");
}

#[test]
fn history_search_finds_commands_cycles_matches_and_restores_draft_on_cancel() {
    let mut console = ConsoleState::default();
    console.record_input("a = cq.Workplane('XY').box(10, 10, 10)");
    console.record_input("show_object(a, name='box_a')");
    console.record_input("b = cq.Workplane('XY').cylinder(20, 5)");
    console.record_input("show_object(b, name='cyl_b')");
    assert_eq!(console.history().len(), 4);

    console.input = "draft_expr".to_owned();
    console.open_history_search();
    console.set_history_search_query("show_object");
    assert_eq!(console.history_matches("show_object"), vec![3, 1]);
    assert_eq!(console.input, "show_object(b, name='cyl_b')");

    console.step_history_search(true);
    assert_eq!(console.input, "show_object(a, name='box_a')");

    console.cancel_history_search();
    assert!(!console.search_open);
    assert_eq!(console.input, "draft_expr");

    console.open_history_search();
    console.set_history_search_query("cylinder");
    console.accept_history_search();
    assert!(!console.search_open);
    assert_eq!(console.input, "b = cq.Workplane('XY').cylinder(20, 5)");
}
