use super::*;
use crate::scene::Mesh;
use crate::viewer::renderer::Style;
use std::sync::Arc;

fn item(mesh: Mesh, transform: Mat4, style: Style, layer: Layer) -> DrawItem {
    DrawItem {
        mesh: Arc::new(mesh),
        transform,
        style,
        layer,
    }
}

fn opaque() -> Style {
    Style {
        face_color: [1.0, 0.0, 0.0, 1.0],
        edge_color: [0.0; 4],
        highlight: [0.0; 4],
        edge_width: 1.0,
        point_size: 1.0,
        faces: true,
        edges: false,
        points: false,
        clipped: true,
        background_fill: false,
        hidden_edges: false,
        edge_min_angle: 0.0,
        silhouettes: false,
        surface: [0.0; 4],
    }
}

/// A triangle spanning x and y in -1..1 at height `z`.
fn triangle(z: f32) -> Mesh {
    Mesh::new(
        vec![[-1.0, -1.0, z], [1.0, -1.0, z], [0.0, 1.0, z]],
        vec![[0.0, 0.0, 1.0]; 3],
        vec![0, 1, 2],
        Vec::new(),
        Vec::new(),
    )
}

#[test]
fn the_shadow_lies_on_the_ground_under_the_model_with_room_for_its_blur() {
    let bounds = Aabb {
        min: Vec3::new(-1.0, -2.0, 3.0),
        max: Vec3::new(1.0, 2.0, 5.0),
    };
    let area = ShadowArea::around(bounds).unwrap();
    assert_eq!(area.ground, 3.0, "at the bottom of the model");
    // Wider than the model across, centred under it.
    assert!(area.side > 4.0);
    assert!((area.corner[0] + area.side * 0.5).abs() < 1e-5);
    assert!((area.corner[1] + area.side * 0.5).abs() < 1e-5);
    assert!(area.fade > 0.0);
    assert!(ShadowArea::around(Aabb::EMPTY).is_none());
}

#[test]
fn only_faces_that_would_darken_the_ground_cast_a_shadow() {
    let solid = item(triangle(0.0), Mat4::IDENTITY, opaque(), Layer::Scene);
    assert!(casts_shadow(&solid));
    let wire = item(
        Mesh::lines(vec![[0.0; 3], [1.0, 0.0, 0.0]]),
        Mat4::IDENTITY,
        opaque(),
        Layer::Scene,
    );
    assert!(!casts_shadow(&wire), "no faces");
    let ghost = Style {
        face_color: [1.0, 0.0, 0.0, 0.1],
        ..opaque()
    };
    assert!(!casts_shadow(&item(
        triangle(0.0),
        Mat4::IDENTITY,
        ghost,
        Layer::Scene
    )));
    let helper = item(triangle(0.0), Mat4::IDENTITY, opaque(), Layer::Overlay);
    assert!(!casts_shadow(&helper), "highlights and guides cast none");
    let outline = Style {
        faces: false,
        ..opaque()
    };
    assert!(!casts_shadow(&item(
        triangle(0.0),
        Mat4::IDENTITY,
        outline,
        Layer::Scene
    )));
}

#[test]
fn the_shadow_is_drawn_again_only_when_its_casters_move() {
    let shadow = GroundShadow {
        strength: 0.5,
        softness: 0.5,
    };
    let solid = item(triangle(1.0), Mat4::IDENTITY, opaque(), Layer::Scene);
    let guide = item(triangle(0.0), Mat4::IDENTITY, opaque(), Layer::Overlay);
    let items = [solid, guide];
    let area = ShadowArea::around(shadow_bounds(&items)).unwrap();
    let key = shadow_key(&items, &area, &shadow);
    assert_eq!(area.ground, 1.0, "the guide does not count");

    // The same items: the same shadow.
    assert_eq!(shadow_key(&items, &area, &shadow), key);
    // A moved caster or softer edges: drawn again.
    let moved = [item(
        triangle(1.0),
        Mat4::from_translation(Vec3::X),
        opaque(),
        Layer::Scene,
    )];
    assert_ne!(shadow_key(&moved, &area, &shadow), key);
    let softer = GroundShadow {
        softness: 0.9,
        ..shadow
    };
    assert_ne!(shadow_key(&items, &area, &softer), key);
    // A darker shadow is only the floor's uniform: nothing to redraw.
    let darker = GroundShadow {
        strength: 0.9,
        ..shadow
    };
    assert_eq!(shadow_key(&items, &area, &darker), key);
}

#[test]
fn the_shadow_shaders_are_valid_wgsl_matching_their_uniforms() {
    let source = include_str!("../../../src/viewer/shadow.wgsl");
    let module = naga::front::wgsl::parse_str(source)
        .unwrap_or_else(|error| panic!("{}", error.emit_to_string(source)));
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .unwrap_or_else(|error| panic!("invalid WGSL: {error:?}"));
    let entry_points: Vec<&str> = module
        .entry_points
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    for name in [
        "vs_occlusion",
        "fs_occlusion",
        "vs_fullscreen",
        "fs_blur",
        "vs_floor",
        "fs_floor",
    ] {
        assert!(entry_points.contains(&name), "{name}");
    }
    let mut layouter = naga::proc::Layouter::default();
    layouter.update(module.to_ctx()).unwrap();
    let size = |name: &str| {
        let (_, variable) = module
            .global_variables
            .iter()
            .find(|(_, variable)| variable.name.as_deref() == Some(name))
            .unwrap();
        layouter[variable.ty].size as usize
    };
    assert_eq!(size("shadow_view"), size_of::<ViewUniform>());
    assert_eq!(size("blur"), size_of::<BlurUniform>());
    assert_eq!(size("floor"), size_of::<FloorUniform>());
}
