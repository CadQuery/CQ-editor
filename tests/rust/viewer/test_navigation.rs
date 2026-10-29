use super::*;

#[test]
fn drag_motion_is_incremental_instead_of_reapplying_the_whole_drag() {
    let mut drag = Drag {
        mode: DragMode::Pan,
        start: Pos2::new(10.0, 20.0),
        last: Pos2::new(10.0, 20.0),
        pivot: None,
    };

    assert_eq!(drag.advance(Pos2::new(14.0, 23.0)), Vec2::new(4.0, 3.0));
    assert_eq!(drag.advance(Pos2::new(17.0, 25.0)), Vec2::new(3.0, 2.0));
    assert_eq!(drag.last, Pos2::new(17.0, 25.0));

    let mut incremental = Camera::default();
    incremental.pan(GVec2::new(4.0, 3.0), 600.0);
    incremental.pan(GVec2::new(3.0, 2.0), 600.0);
    let mut once = Camera::default();
    once.pan(GVec2::new(7.0, 5.0), 600.0);
    assert!((incremental.target - once.target).length() < 1e-5);
}

/// The original editor's gestures, except the right button: it orbits like
/// the left one instead of zooming.
#[test]
fn mouse_gestures_choose_their_navigation_modes() {
    use egui::{Modifiers, PointerButton};

    let mode = |modifiers, button| {
        requested_drag_mode(
            modifiers,
            button == PointerButton::Primary,
            button == PointerButton::Middle,
            button == PointerButton::Secondary,
        )
    };

    assert!(matches!(
        mode(Modifiers::NONE, PointerButton::Primary),
        Some(DragMode::Orbit)
    ));
    assert!(matches!(
        mode(Modifiers::NONE, PointerButton::Middle),
        Some(DragMode::Pan)
    ));
    assert!(matches!(
        mode(Modifiers::CTRL, PointerButton::Primary),
        Some(DragMode::Pan)
    ));
    assert!(matches!(
        mode(Modifiers::COMMAND, PointerButton::Primary),
        Some(DragMode::Pan)
    ));
    assert!(matches!(
        mode(Modifiers::NONE, PointerButton::Secondary),
        Some(DragMode::Orbit)
    ));
    assert!(matches!(
        mode(Modifiers::SHIFT, PointerButton::Primary),
        Some(DragMode::Zoom)
    ));
}

#[test]
fn zoom_percentage_is_relative_to_the_last_fit_extent() {
    let mut viewer = Viewer::default();
    viewer.camera.extent = 20.0;
    viewer.zoom_reference_extent = 20.0;

    viewer.set_zoom_percent(200.0);
    assert!((viewer.camera.extent - 10.0).abs() < 1e-5);
    assert!((viewer.zoom_percent() - 200.0).abs() < 1e-4);

    viewer.set_zoom_percent(50.0);
    assert!((viewer.camera.extent - 40.0).abs() < 1e-5);
    assert!((viewer.zoom_percent() - 50.0).abs() < 1e-4);
}

#[test]
fn zoom_controls_stay_inside_the_view_and_hide_when_it_is_too_small() {
    let view = Rect::from_min_size(Pos2::new(50.0, 80.0), Vec2::new(800.0, 600.0));
    let controls = zoom_controls_rect(view).unwrap();
    assert!(view.contains_rect(controls));
    assert_eq!(controls.left(), view.left() + ZOOM_CONTROLS_MARGIN);
    assert_eq!(controls.top(), view.top() + ZOOM_CONTROLS_MARGIN);

    let narrow = Rect::from_min_size(Pos2::ZERO, Vec2::new(180.0, 600.0));
    assert!(zoom_controls_rect(narrow).is_none());
}

#[test]
fn zoom_overlay_buttons_are_interactive_over_the_viewer() {
    fn frame(ctx: &egui::Context, viewer: &mut Viewer, events: Vec<egui::Event>) -> Rect {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
            events,
            ..Default::default()
        };
        let mut controls = Rect::NOTHING;
        let mut output = ctx.run_ui(input, |ui| {
            let view = ui.available_rect_before_wrap();
            ui.allocate_rect(view, Sense::click_and_drag());
            controls = zoom_controls_rect(view).unwrap();
            viewer.show_zoom_controls(ui, view, controls);
        });
        // This headless test has no renderer to upload the font atlas to.
        output.textures_delta.clear();
        controls
    }

    let ctx = egui::Context::default();
    let mut viewer = Viewer::default();
    let controls = frame(&ctx, &mut viewer, Vec::new());
    // First button: frame margin (6, 5), then a 28-point button.
    let zoom_out = controls.min + Vec2::new(20.0, 19.0);
    frame(
        &ctx,
        &mut viewer,
        vec![
            egui::Event::PointerMoved(zoom_out),
            egui::Event::PointerButton {
                pos: zoom_out,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    frame(
        &ctx,
        &mut viewer,
        vec![egui::Event::PointerButton {
            pos: zoom_out,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );

    assert!((viewer.zoom_percent() - 80.0).abs() < 1e-4);
    assert!((viewer.camera.extent - 25.0).abs() < 1e-4);
}

#[test]
fn pressing_the_view_uses_a_closed_hand_cursor_for_navigation() {
    fn frame(
        ctx: &egui::Context,
        viewer: &mut Viewer,
        events: Vec<egui::Event>,
    ) -> egui::CursorIcon {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
            events,
            ..Default::default()
        };
        let scene = Scene::default();
        let settings = crate::settings::Viewer::default();
        let colors = crate::theme::Palette::DARK.viewer();
        let mut output = ctx.run_ui(input, |ui| {
            let rect = ui.available_rect_before_wrap();
            let response = ui.allocate_rect(rect, Sense::click_and_drag());
            viewer.handle_input(
                ui,
                &response,
                rect,
                &zoom_controls_rect(rect).into_iter().collect::<Vec<_>>(),
                &ViewInput {
                    scene: &scene,
                    settings: &settings,
                    rendering: &crate::settings::Rendering::default(),
                    colors: &colors,
                    busy: None,
                    empty_hint: None,
                    meshing: None,
                },
                0.0,
            );
        });
        output.textures_delta.clear();
        output.platform_output.cursor_icon
    }

    let ctx = egui::Context::default();
    let mut viewer = Viewer::default();
    frame(&ctx, &mut viewer, Vec::new());
    let position = Pos2::new(400.0, 300.0);
    let cursor = frame(
        &ctx,
        &mut viewer,
        vec![
            egui::Event::PointerMoved(position),
            egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );

    assert_eq!(cursor, egui::CursorIcon::Grabbing);
    assert_eq!(
        requested_drag_mode(egui::Modifiers::NONE, false, false, true).map(DragMode::cursor),
        Some(egui::CursorIcon::Grabbing)
    );
}

#[test]
fn orbit_changes_only_the_view_direction() {
    let mut camera = Camera::default();
    let before = camera.pose();

    camera.orbit_turntable(GVec2::new(24.0, -12.0));

    assert_eq!(camera.target, before.target);
    assert_eq!(camera.extent, before.extent);
    assert_ne!(camera.rotation, before.rotation);
}

#[test]
fn control_bars_stack_up_from_the_bottom_without_overlapping() {
    let view = Rect::from_min_size(Pos2::new(100.0, 50.0), Vec2::new(900.0, 600.0));
    let zoom = zoom_controls_rect(view).unwrap();
    let bars: Vec<Rect> = (0..3)
        .map(|row| bottom_bar_rect(view, row, 400.0).unwrap())
        .collect();
    assert!((bars[0].bottom() - (view.bottom() - ZOOM_CONTROLS_MARGIN)).abs() < 1e-3);
    for (row, bar) in bars.iter().enumerate() {
        assert!(view.contains_rect(*bar));
        assert!(!bar.intersects(zoom), "bar {row} under the zoom controls");
        if row > 0 {
            assert!(
                bar.bottom() < bars[row - 1].top(),
                "bar {row} above the previous"
            );
        }
    }
    // Narrow views clamp the width, short ones drop the upper bars.
    let narrow = Rect::from_min_size(Pos2::ZERO, Vec2::new(300.0, 600.0));
    assert!(
        bottom_bar_rect(narrow, 0, 430.0).unwrap().width() <= 300.0 - 2.0 * ZOOM_CONTROLS_MARGIN
    );
    let short = Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 150.0));
    assert!(bottom_bar_rect(short, 0, 400.0).is_some());
    assert!(bottom_bar_rect(short, 1, 400.0).is_none());
}

fn red_look(edges: Option<bool>) -> PartLook {
    PartLook {
        face_color: [1.0, 0.0, 0.0, 0.8],
        edge_color: [0.0, 0.0, 0.0, 1.0],
        highlight: [0.0; 4],
        has_faces: true,
        emphasized: false,
        edges,
        finish: settings::Finish::Default,
    }
}

#[test]
fn the_edge_style_chooses_the_edges_of_faces_outside_wireframe() {
    use DisplayMode::{HiddenLine, Shaded, Wireframe, XRay};
    use settings::{EdgeStyle, Rendering};
    let settings = settings::Viewer::default();
    let with = |edges| Rendering {
        edges,
        sharp_angle: 25.0,
        ..Rendering::default()
    };

    for mode in [Shaded, XRay, HiddenLine] {
        let all = mode.style(red_look(None), &settings, &with(EdgeStyle::All));
        assert_eq!((all.edge_min_angle, all.silhouettes), (0.0, false));
        let sharp = mode.style(red_look(None), &settings, &with(EdgeStyle::Sharp));
        assert_eq!((sharp.edge_min_angle, sharp.silhouettes), (25.0, false));
        let outline = mode.style(red_look(None), &settings, &with(EdgeStyle::Outline));
        assert_eq!((outline.edge_min_angle, outline.silhouettes), (25.0, true));
    }
    // Wireframe draws every edge, and a wire has no faces to judge by.
    let wireframe = Wireframe.style(red_look(None), &settings, &with(EdgeStyle::Outline));
    assert_eq!(
        (wireframe.edge_min_angle, wireframe.silhouettes),
        (0.0, false)
    );
    let wire = PartLook {
        has_faces: false,
        ..red_look(None)
    };
    let wire = Shaded.style(wire, &settings, &with(EdgeStyle::Outline));
    assert_eq!((wire.edge_min_angle, wire.silhouettes), (0.0, false));
}

#[test]
fn display_modes_and_object_edges_shape_how_parts_are_drawn() {
    use DisplayMode::{HiddenLine, Shaded, Wireframe, XRay};
    let settings = settings::Viewer::default();
    let no_edges = settings::Viewer {
        show_edges: false,
        ..settings.clone()
    };

    // Shaded and X-ray follow the object's own choice, then the preference.
    for mode in [Shaded, XRay] {
        assert!(
            mode.style(red_look(None), &settings, &Default::default())
                .edges
        );
        assert!(
            !mode
                .style(red_look(Some(false)), &settings, &Default::default())
                .edges
        );
        assert!(
            !mode
                .style(red_look(None), &no_edges, &Default::default())
                .edges
        );
        assert!(
            mode.style(red_look(Some(true)), &no_edges, &Default::default())
                .edges
        );
    }
    // Edges are the drawing in wireframe and hidden-line modes, and all
    // there is of a wire.
    for mode in [Wireframe, HiddenLine] {
        assert!(
            mode.style(red_look(Some(false)), &no_edges, &Default::default())
                .edges
        );
    }
    let wire = PartLook {
        has_faces: false,
        ..red_look(Some(false))
    };
    assert!(Shaded.style(wire, &no_edges, &Default::default()).edges);

    let shaded = Shaded.style(red_look(None), &settings, &Default::default());
    assert!(shaded.faces && !shaded.background_fill && !shaded.hidden_edges);
    assert_eq!(shaded.face_color[3], 0.8);
    assert!(
        !Wireframe
            .style(red_look(None), &settings, &Default::default())
            .faces
    );
    assert_eq!(
        XRay.style(red_look(None), &settings, &Default::default())
            .face_color[3],
        XRAY_OPACITY
    );

    let hidden_line = HiddenLine.style(red_look(None), &settings, &Default::default());
    assert!(hidden_line.faces && hidden_line.background_fill && hidden_line.hidden_edges);
    assert_eq!(
        hidden_line.face_color[3], 1.0,
        "opaque, to hide what is behind"
    );
    let solid_only = settings::Viewer {
        hidden_edges: false,
        ..settings.clone()
    };
    assert!(
        !HiddenLine
            .style(red_look(None), &solid_only, &Default::default())
            .hidden_edges
    );
}

/// A grid of `side` × `side` vertices: 2 (side - 1)² triangles, enough to be
/// simplified when `side` is large.
fn fine_plate(side: u32) -> Mesh {
    let mut positions = Vec::new();
    for y in 0..side {
        for x in 0..side {
            positions.push([x as f32 / side as f32, y as f32 / side as f32, 0.0]);
        }
    }
    let mut indices = Vec::new();
    for y in 0..side - 1 {
        for x in 0..side - 1 {
            let a = y * side + x;
            indices.extend([a, a + 1, a + side, a + 1, a + side + 1, a + side]);
        }
    }
    let normals = vec![[0.0, 0.0, 1.0]; positions.len()];
    let edges = vec![[0.0; 3], [1.0, 0.0, 0.0]];
    Mesh::new(positions, normals, indices, edges, Vec::new())
}

#[test]
fn an_orbit_turns_around_the_model_point_under_the_pointer() {
    use crate::scene::{Part, SceneNode};
    use crate::viewer::camera::Pose;
    // A 0.75-unit plate on z = 0, from the origin.
    let node = SceneNode {
        key: "plate".into(),
        name: "plate".into(),
        kind: "Workplane".into(),
        inspectable: false,
        parts: vec![Part {
            mesh: Arc::new(fine_plate(4)),
            transform: GMat4::IDENTITY,
            color: None,
            alpha: None,
            face_attributes: Vec::new(),
            material: None,
            render_mesh: None,
        }],
        children: Vec::new(),
        visible: true,
        color: None,
        alpha: None,
        edge_color: None,
        finish: None,
        overrides: Default::default(),
        line: None,
        script: None,
    };
    let mut scene = Scene::default();
    scene.append(vec![node], None);
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    let mut viewer = Viewer {
        aspect: 800.0 / 600.0,
        ..Viewer::default()
    };
    let top = |target| Pose {
        target,
        rotation: StandardView::Top.rotation(),
        extent: 2.0,
    };
    let (far, plate) = (Vec3::new(100.0, 100.0, 0.0), Vec3::new(0.375, 0.375, 0.0));

    // Pressed while the view still moves from far away to the plate: the
    // move ends first, and the point is picked in the view the drag turns.
    viewer.camera.set_pose(top(far));
    viewer.animation = Some(Animation::new(top(far), top(plate), 0.0, ANIMATION_SECONDS));
    // 30 points right of the middle: 0.2 units at 2 units per 300 points.
    viewer.start_drag(
        &scene,
        rect,
        DragMode::Orbit,
        rect.center() + Vec2::new(30.0, 0.0),
    );
    assert!(viewer.animation.is_none());
    assert_eq!(viewer.camera.target, plate);
    let pivot = viewer.drag.as_ref().unwrap().pivot.unwrap();
    assert!(
        (pivot - Vec3::new(0.575, 0.375, 0.0)).length() < 1e-3,
        "{pivot}"
    );

    // Over empty space it turns around the middle of the view; a pan has none.
    viewer.start_drag(&scene, rect, DragMode::Orbit, rect.min + Vec2::splat(5.0));
    assert_eq!(viewer.drag.as_ref().unwrap().pivot, Some(plate));
    viewer.start_drag(&scene, rect, DragMode::Pan, rect.center());
    assert_eq!(viewer.drag.as_ref().unwrap().pivot, None);
}

#[test]
fn large_models_draw_small_parts_simplified_but_never_the_selection() {
    use crate::scene::{Part, SceneNode};
    let mesh = Arc::new(fine_plate(80));
    let simplified_id = mesh.simplified().expect("fine enough").id;
    let node = SceneNode {
        key: "plate".into(),
        name: "plate".into(),
        kind: "Workplane".into(),
        inspectable: false,
        parts: vec![Part {
            mesh: mesh.clone(),
            transform: GMat4::IDENTITY,
            color: None,
            alpha: None,
            face_attributes: Vec::new(),
            material: None,
            render_mesh: None,
        }],
        children: Vec::new(),
        visible: true,
        color: None,
        alpha: None,
        edge_color: None,
        finish: None,
        overrides: Default::default(),
        line: None,
        script: None,
    };
    let mut scene = Scene::default();
    scene.append(vec![node], None);
    let settings = crate::settings::Viewer::default();
    let colors = crate::theme::Palette::DARK.viewer();
    let mut viewer = Viewer {
        view_px: [1000, 1000],
        ..Viewer::default()
    };
    let drawn = |viewer: &mut Viewer, scene: &Scene, simplify: bool| {
        let input = ViewInput {
            scene,
            settings: &settings,
            rendering: &crate::settings::Rendering::default(),
            colors: &colors,
            busy: None,
            empty_hint: None,
            meshing: None,
        };
        let item = viewer.draw_items(&input, simplify).remove(0);
        (item.mesh.id, item.style.edges)
    };

    // The 1.4-unit plate seen 2 units high: hundreds of pixels, in full.
    viewer.camera.extent = 1.0;
    assert_eq!(drawn(&mut viewer, &scene, true), (mesh.id, true));
    // 50 units high: about 28 pixels, simplified with its edges.
    viewer.camera.extent = 25.0;
    assert_eq!(drawn(&mut viewer, &scene, true), (simplified_id, true));
    assert_eq!(drawn(&mut viewer, &scene, false), (mesh.id, true));
    // 1000 units high: a speck, without edges.
    viewer.camera.extent = 500.0;
    assert_eq!(drawn(&mut viewer, &scene, true), (simplified_id, false));
    // Selected, it is drawn as it is.
    scene.select_only("plate");
    assert_eq!(drawn(&mut viewer, &scene, true), (mesh.id, true));
}

#[test]
fn presentation_shows_the_model_alone_in_its_own_material_and_lights() {
    use crate::scene::{Part, SceneNode};
    use crate::settings::{EdgeStyle, Finish, Rendering};
    let part = |name: &str, finish: Option<Finish>| SceneNode {
        key: name.into(),
        name: name.into(),
        kind: "Workplane".into(),
        inspectable: false,
        parts: vec![Part {
            mesh: Arc::new(fine_plate(4)),
            transform: GMat4::IDENTITY,
            color: None,
            alpha: None,
            face_attributes: Vec::new(),
            material: None,
            render_mesh: None,
        }],
        children: Vec::new(),
        visible: true,
        color: None,
        alpha: None,
        edge_color: None,
        finish,
        overrides: Default::default(),
        line: None,
        script: None,
    };
    let mut scene = Scene::default();
    scene.append(
        vec![
            part("plain", None),
            part("steel", Some(Finish::BrushedSteel)),
        ],
        None,
    );
    let settings = crate::settings::Viewer::default();
    let colors = crate::theme::Palette::DARK.viewer();
    let mut viewer = Viewer::default();
    viewer.update_ground(
        true,
        scene.visible_bounds(),
        Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)),
    );
    let mut rendering = Rendering::default();
    rendering.presentation.material = Finish::Aluminum;
    let items = |viewer: &mut Viewer, rendering: &Rendering| {
        let input = ViewInput {
            scene: &scene,
            settings: &settings,
            rendering,
            colors: &colors,
            busy: None,
            empty_hint: None,
            meshing: None,
        };
        viewer.draw_items(&input, false)
    };

    // Shaded: the plain shading, every edge, the axes with the origin dot
    // (two points) and the grid.
    let shaded = items(&mut viewer, &rendering);
    assert_eq!(shaded.len(), 2 + 3 + 2 + GRID_FADE.len() * 2);
    assert_eq!(shaded[0].style.surface, Finish::Default.surface());
    assert!(shaded[0].style.edges);

    viewer.display_mode = DisplayMode::Presentation;
    let presented = items(&mut viewer, &rendering);
    assert_eq!(presented.len(), 2, "no axes, no grid");
    let (plain, steel) = (&presented[0].style, &presented[1].style);
    assert_eq!(plain.surface, Finish::Aluminum.surface(), "its default");
    assert_eq!(steel.surface, Finish::BrushedSteel.surface(), "its own");
    // The default material's color for an object without one.
    let aluminum = Finish::Aluminum.natural_color().unwrap();
    assert_eq!(plain.face_color, super::rgb_to_linear(aluminum, 1.0));
    assert!(!plain.edges && !steel.edges, "no edges by default");

    rendering.presentation.show_edges = true;
    rendering.presentation.helpers = true;
    rendering.presentation.edges = EdgeStyle::Outline;
    let outlined = items(&mut viewer, &rendering);
    assert_eq!(outlined.len(), 2 + 3 + 2 + GRID_FADE.len() * 2);
    assert!(outlined[0].style.edges && outlined[0].style.silhouettes);

    // Its own lights and background; Shaded keeps the others.
    let input = ViewInput {
        scene: &scene,
        settings: &settings,
        rendering: &rendering,
        colors: &colors,
        busy: None,
        empty_hint: None,
        meshing: None,
    };
    let presented = viewer.backdrop(&input);
    assert_eq!(presented.lighting, rendering.presentation.lighting);
    assert_ne!(presented.lighting, rendering.lighting);
    let presentation = &rendering.presentation;
    assert_eq!(
        presented.top,
        super::linear_rgba(presentation.background_top, 1.0)
    );
    assert_eq!(
        presented.bottom,
        super::linear_rgba(presentation.background_bottom, 1.0)
    );
    assert_eq!(
        presented.shadow,
        Some(super::GroundShadow {
            strength: presentation.shadow_strength,
            softness: presentation.shadow_softness,
        })
    );
    viewer.display_mode = DisplayMode::Shaded;
    let shaded = viewer.backdrop(&input);
    assert_eq!(shaded.lighting, rendering.lighting);
    let plain = super::linear_rgba(settings.background, 1.0);
    assert_eq!((shaded.top, shaded.bottom), (plain, plain));
    assert_eq!(shaded.shadow, None, "only Presentation has a ground shadow");
}

#[test]
fn the_ground_grid_follows_its_setting_and_the_model() {
    let mut viewer = Viewer::default();
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    let screw = Aabb::from_points([Vec3::new(-6.5, -6.5, -60.0), Vec3::new(6.5, 6.5, 8.0)]);
    viewer.update_ground(false, screw, rect);
    assert!(viewer.ground.is_none());

    viewer.update_ground(true, screw, rect);
    let (grid, rings) = viewer.ground.clone().unwrap();
    assert_eq!(grid.center.z, -60.0, "a floor under the model");

    viewer.update_ground(true, screw, rect);
    let kept = viewer.ground.as_ref().unwrap();
    assert!(
        Arc::ptr_eq(&kept.1[0].0, &rings[0].0),
        "same lines, same meshes"
    );

    // Turning and zooming leave the floor where it is: at most it reaches
    // farther, and zooming out fades the lines that crowd together.
    for view in [StandardView::Front, StandardView::Right, StandardView::Top] {
        viewer.camera.rotation = view.rotation();
        viewer.update_ground(true, screw, rect);
        let turned = viewer.ground.as_ref().unwrap().0;
        assert_eq!((turned.minor, turned.center), (grid.minor, grid.center));
        assert!(turned.radius >= grid.radius);
    }
    viewer.camera.rotation = StandardView::Iso.rotation();
    viewer.camera.extent *= 20.0;
    viewer.update_ground(true, screw, rect);
    let far = viewer.ground.as_ref().unwrap().0;
    assert_eq!((far.minor, far.center), (grid.minor, grid.center));
    assert!(
        far.radius > grid.radius,
        "the floor reaches the edge of the view"
    );
    assert!(far.minor_opacity < grid.minor_opacity);
    viewer.camera.extent /= 20.0;
    viewer.update_ground(true, screw, rect);
    assert_eq!(viewer.ground.as_ref().unwrap().0, grid);

    // Another model gets its own floor.
    let plate = Aabb::from_points([Vec3::ZERO, Vec3::new(400.0, 300.0, 10.0)]);
    viewer.update_ground(true, plate, rect);
    let (moved, _) = viewer.ground.as_ref().unwrap();
    assert_eq!(moved.center.z, 0.0);
    assert!(moved.minor > grid.minor);
    viewer.update_ground(true, screw, rect);

    // The clipping planes enclose it, and its lines fade toward the edge.
    let scene = Scene::default();
    viewer.update_scene_extent(&scene, screw);
    for corner in grid.corners() {
        let distance = (corner - viewer.camera.scene_center).length();
        assert!(distance <= viewer.camera.scene_radius + 1e-3);
    }
    let settings = crate::settings::Viewer::default();
    let colors = crate::theme::Palette::DARK.viewer();
    let input = ViewInput {
        scene: &scene,
        settings: &settings,
        rendering: &crate::settings::Rendering::default(),
        colors: &colors,
        busy: None,
        empty_hint: None,
        meshing: None,
    };
    let items = viewer.draw_items(&input, false);
    let z_axis = viewer.axis_meshes.as_ref().unwrap().1[2].id;
    let z_points = &viewer.axis_meshes.as_ref().unwrap().1[2].edges;
    assert_eq!(z_points[0], Vec3::ZERO.to_array());
    assert!((z_points[1][2] - viewer.camera.extent * 0.45).abs() < 1e-5);
    let z_item = items.iter().find(|item| item.mesh.id == z_axis).unwrap();
    assert_eq!(z_item.layer, Layer::Scene);
    assert!(
        z_item.style.hidden_edges,
        "the Z axis is dashed where a model built around the origin hides it"
    );
    let origin: Vec<_> = items
        .iter()
        .filter(|item| item.mesh.id == viewer.origin_mesh.id)
        .collect();
    assert_eq!(origin.len(), 2, "a light dot in a dark ring");
    assert!(
        origin
            .iter()
            .all(|item| { item.layer == Layer::Overlay && item.style.points && !item.style.edges })
    );
    assert!(origin[0].style.point_size > origin[1].style.point_size);
    let grid_items: Vec<_> = items
        .iter()
        .filter(|item| {
            viewer
                .ground
                .as_ref()
                .unwrap()
                .1
                .iter()
                .any(|(minor, major)| item.mesh.id == minor.id || item.mesh.id == major.id)
        })
        .collect();
    assert_eq!(grid_items.len(), GRID_FADE.len() * 2);
    assert!(grid_items.iter().all(|item| {
        item.layer == Layer::Reference && !item.style.clipped && !item.style.faces
    }));
    let major_opacity: Vec<f32> = viewer
        .ground
        .as_ref()
        .unwrap()
        .1
        .iter()
        .map(|(_, major)| {
            items
                .iter()
                .find(|item| item.mesh.id == major.id)
                .unwrap()
                .style
                .edge_color[3]
        })
        .collect();
    assert!(major_opacity.windows(2).all(|pair| pair[0] > pair[1]));
}
