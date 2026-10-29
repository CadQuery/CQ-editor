use super::*;
use egui_phosphor::regular as icon;

#[test]
fn headings_render_icons_like_body_text() {
    let definitions = fonts();
    let heading_family = &definitions.families[&FontFamily::Name(SEMIBOLD.into())];
    // Letters still come from the semibold face, icons from Phosphor.
    assert_eq!(heading_family[0], "inter-semibold");
    assert!(heading_family.iter().any(|font| font == "phosphor"));

    let ctx = egui::Context::default();
    ctx.set_fonts(definitions);
    let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
    // No renderer here to upload the font atlas to.
    output.textures_delta.clear();

    let glyph = icon::GEAR_SIX.chars().next().unwrap();
    let heading = FontId::new(17.0, FontFamily::Name(SEMIBOLD.into()));
    ctx.fonts_mut(|fonts| {
        assert!(fonts.has_glyph(&FontId::proportional(13.0), glyph));
        assert!(fonts.has_glyph(&heading, glyph), "dialog titles show icons");
    });
}
