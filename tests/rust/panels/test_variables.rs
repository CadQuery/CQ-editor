use super::*;

#[test]
fn the_value_column_keeps_room_in_narrow_panes() {
    // The default layout gives the panel about 280 points.
    for width in [180.0, 220.0, 280.0, 400.0, 900.0] {
        let (name, type_name) = column_widths(width);
        let value = width - name - type_name;
        assert!(
            value >= MIN_VALUE_WIDTH,
            "{width} wide: only {value} for values"
        );
    }
    // The former fixed widths (120 and 90) left 70 points at 280.
    let (name, type_name) = column_widths(280.0);
    assert!(280.0 - name - type_name > 120.0);
}

#[test]
fn clicking_the_text_of_a_row_selects_it() {
    let variable = Variable {
        name: "a_rather_long_variable_name".to_owned(),
        type_name: "Workplane".to_owned(),
        value: "x".repeat(500),
    };
    let rows = [&variable];
    let ctx = egui::Context::default();
    let mut selected = None;
    // Returns where the rows are.
    let run = |events: Vec<egui::Event>, selected: &mut Option<String>| {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(400.0, 200.0),
            )),
            events,
            ..Default::default()
        };
        let mut body = egui::Rect::NOTHING;
        let mut output = ctx.run_ui(input, |ui| body = table(ui, &rows, selected));
        output.textures_delta.clear();
        body
    };
    let body = run(Vec::new(), &mut selected);

    // On the name's text, which a selectable label used to take.
    let pos = body.left_top() + egui::vec2(12.0, 6.0);
    let button = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    run(vec![egui::Event::PointerMoved(pos)], &mut selected);
    run(vec![button(true)], &mut selected);
    run(vec![button(false)], &mut selected);
    assert_eq!(selected.as_deref(), Some("a_rather_long_variable_name"));
}

#[test]
fn the_rows_stay_above_the_detail_view_in_a_short_pane() {
    let variables: Vec<Variable> = (0..12)
        .map(|index| Variable {
            name: format!("value_{index}"),
            type_name: "str".to_owned(),
            value: "x".repeat(400),
        })
        .collect();
    let rows: Vec<&Variable> = variables.iter().collect();
    let mut selected = Some("value_1".to_owned());
    let ctx = egui::Context::default();
    // As short as the Variables pane of the default layout can get.
    let height = 130.0;
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(280.0, height),
        )),
        ..Default::default()
    };
    let mut rects = (egui::Rect::NOTHING, None);
    // Panels settle their size over passes, like real frames.
    for _ in 0..3 {
        let mut output = ctx.run_ui(input.clone(), |ui| {
            rects = table_and_detail(ui, &rows, &mut selected);
        });
        output.textures_delta.clear();
    }

    let (table, detail) = rects;
    let detail = detail.expect("the selected row has a detail view");
    // Its contents fit its size instead of growing it over the rows.
    assert!(
        detail.height() <= height * 0.6 + 0.5,
        "detail view {detail:?}"
    );
    assert!(
        table.max.y <= detail.min.y + 0.5,
        "the rows {table:?} end above the detail view {detail:?}"
    );
    assert!(table.height() > 0.0);
    assert_eq!(selected.as_deref(), Some("value_1"));
}
