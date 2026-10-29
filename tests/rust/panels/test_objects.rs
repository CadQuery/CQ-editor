use super::*;

/// Lays out a property sheet much taller than a pane of `height` points.
/// Returns the pane, the sheet and what is left for the tree.
fn layout(height: f32) -> (egui::Rect, egui::Rect, egui::Rect) {
    let ctx = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(300.0, height),
        )),
        ..Default::default()
    };
    let mut rects = (
        egui::Rect::NOTHING,
        egui::Rect::NOTHING,
        egui::Rect::NOTHING,
    );
    // Panels settle their size over passes, like real frames.
    for _ in 0..3 {
        let mut output = ctx.run_ui(input.clone(), |ui| {
            let pane = ui.available_rect_before_wrap();
            let sheet = property_sheet(ui, |ui| {
                for row in 0..40 {
                    ui.label(format!("Property {row}"));
                }
            });
            rects = (pane, sheet, ui.available_rect_before_wrap());
        });
        // No renderer here to upload the font atlas to.
        output.textures_delta.clear();
    }
    rects
}

#[test]
fn a_tall_property_sheet_scrolls_instead_of_covering_the_tree() {
    let (pane, sheet, tree) = layout(300.0);

    assert!(tree.height() >= TREE_MIN_HEIGHT - 0.5, "tree {tree:?}");
    assert!(
        sheet.height() <= pane.height() - TREE_MIN_HEIGHT + 0.5,
        "sheet {sheet:?}"
    );
    assert!(
        tree.max.y <= sheet.min.y + 0.5,
        "the tree ends above the sheet"
    );
}

#[test]
fn a_very_short_pane_keeps_most_of_its_height_for_the_tree() {
    let (pane, sheet, tree) = layout(100.0);

    assert!(
        sheet.height() <= pane.height() * 0.4 + 0.5,
        "sheet {sheet:?}"
    );
    assert!(tree.height() >= pane.height() * 0.6 - 0.5, "tree {tree:?}");
}

#[test]
fn the_property_sheet_shows_short_numbers_without_rounding_noise() {
    assert_eq!(compact(100.0), "100");
    assert_eq!(compact(2.5), "2.5");
    assert_eq!(compact(-6.938_893_9e-18), "0");
    assert_eq!(compact(std::f64::consts::PI), "3.1416");
    assert_eq!(compact(-12.25), "-12.25");
}

#[test]
fn object_name_filter_matches_nodes_and_subassemblies_without_changing_visibility() {
    fn sample_node(key: &str, name: &str, children: Vec<SceneNode>) -> SceneNode {
        SceneNode {
            key: key.to_owned(),
            name: name.to_owned(),
            kind: "shape".to_owned(),
            inspectable: false,
            parts: Vec::new(),
            children,
            visible: true,
            color: None,
            alpha: None,
            edge_color: None,
            finish: None,
            overrides: Default::default(),
            line: None,
            script: None,
        }
    }

    let bolt = sample_node("n-bolt", "M6_Bolt", Vec::new());
    let nut = sample_node("n-nut", "Hex_Nut", Vec::new());
    let subassembly = sample_node("n-fasteners", "Fasteners", vec![bolt, nut]);
    let housing = sample_node("n-housing", "Main_Housing", Vec::new());

    assert!(node_matches_filter(&subassembly, "bolt"));
    assert!(node_matches_filter(&subassembly, "FASTEN"));
    assert!(!node_matches_filter(&housing, "bolt"));
    assert!(subassembly.visible && housing.visible);
}
