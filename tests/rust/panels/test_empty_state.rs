use super::*;

#[test]
fn tab_content_cannot_paint_outside_its_assigned_body() {
    let ctx = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(500.0, 400.0));
    let body = egui::Rect::from_min_size(egui::pos2(300.0, 40.0), egui::vec2(180.0, 130.0));
    let input = egui::RawInput {
        screen_rect: Some(screen),
        ..Default::default()
    };
    let mut clipped = egui::Rect::NOTHING;

    let mut output = ctx.run_ui(input, |_root| {
        // Reproduce the fresh UI used for an egui_dock tab: its layout is
        // bounded by `body`, while its paint clip initially covers the screen.
        let mut tab = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("tab_clip_regression"),
            egui::UiBuilder::new().max_rect(body),
        );
        tab.set_clip_rect(screen);
        constrain_to_tab(&mut tab);
        clipped = tab.clip_rect();
    });
    output.textures_delta.clear();

    assert_eq!(clipped, body);
}

#[test]
fn empty_state_rows_are_centered_with_natural_spacing() {
    let ctx = egui::Context::default();
    // A first pass loads the fonts.
    let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
    output.textures_delta.clear();

    let style = egui::Style::default();
    let text = "Select a workplane in Objects to inspect it";
    let job = empty_state_job(&style, text, egui::Color32::GRAY, 150.0);
    let mut plain = job.clone();
    plain.halign = egui::Align::LEFT;
    let mut justified = plain.clone();
    justified.justify = true;
    let [centered, plain, justified] =
        [job, plain, justified].map(|job| ctx.fonts_mut(|fonts| fonts.layout_job(job)));
    let widths = |galley: &egui::Galley| -> Vec<f32> {
        galley.rows.iter().map(|row| row.rect().width()).collect()
    };

    assert!(centered.rows.len() > 1, "the text wraps in a narrow panel");
    for row in &centered.rows {
        let rect = row.rect();
        assert!(rect.center().x.abs() < 1.0, "row centered: {rect:?}");
    }
    // Each row keeps the width of ordinary text (without its trailing
    // space), where the old justified layout stretched it to the full width.
    let (centered, plain, justified) = (widths(&centered), widths(&plain), widths(&justified));
    for (row, (natural, plain)) in centered.iter().zip(&plain).enumerate() {
        assert!(natural <= plain, "row {row}: {natural} > {plain}");
    }
    assert!(
        centered[0] < justified[0] - 2.0,
        "{centered:?} vs {justified:?}"
    );
}
