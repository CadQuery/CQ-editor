use super::*;
use crate::settings::Projection;
use crate::viewer::camera::{Pose, StandardView};

fn camera(projection: Projection, target: Vec3, extent: f32, view: StandardView) -> Camera {
    let mut camera = Camera {
        projection,
        ..Camera::default()
    };
    camera.set_pose(Pose {
        target,
        rotation: view.rotation(),
        extent,
    });
    camera
}

/// The DIN 912 M8x60 screw: the origin is under its head.
fn screw() -> Aabb {
    Aabb::from_points([Vec3::new(-6.5, -6.5, -60.0), Vec3::new(6.5, 6.5, 8.0)])
}

/// The floor for a model in the Iso view, `extent` units from the middle of
/// the view to its top.
fn floor(bounds: Aabb, extent: f32) -> GroundGrid {
    let camera = camera(
        Projection::Orthographic,
        Vec3::ZERO,
        extent,
        StandardView::Iso,
    );
    GroundGrid::under(bounds, &camera, VIEW)
}

const VIEW: Vec2 = Vec2::new(800.0, 600.0);

fn points(meshes: &[(Mesh, Mesh)]) -> impl Iterator<Item = Vec3> + '_ {
    meshes
        .iter()
        .flat_map(|(minor, major)| minor.edges.iter().chain(&major.edges))
        .map(|point| Vec3::from_array(*point))
}

#[test]
fn lengths_are_rounded_to_one_two_or_five_and_printed_plainly() {
    let rounded: Vec<f32> = [0.03, 0.7, 1.0, 1.2, 3.0, 7.0, 120.0]
        .into_iter()
        .map(round_length)
        .collect();
    assert_eq!(rounded, [0.05, 1.0, 1.0, 2.0, 5.0, 10.0, 200.0]);
    assert_eq!(round_length(0.0), 1.0);
    assert_eq!(round_length(f32::NAN), 1.0);

    assert_eq!(format_length(2.5, Units::Millimeters), "2.5 mm");
    assert_eq!(format_length(100.0, Units::Inches), "100 in");
    assert_eq!(format_length(0.05, Units::None), "0.05");
    assert_eq!(format_length(1000.0, Units::Meters), "1000 m");
}

#[test]
fn units_per_point_follow_the_zoom_in_both_projections() {
    let orthographic = camera(
        Projection::Orthographic,
        Vec3::ZERO,
        10.0,
        StandardView::Top,
    );
    assert!((units_per_point(&orthographic, 500.0) - 0.04).abs() < 1e-6);
    let perspective = camera(Projection::Perspective, Vec3::ZERO, 10.0, StandardView::Top);
    let expected = 2.0 * perspective.distance() * (perspective.fov_y / 2.0).tan() / 500.0;
    assert!((units_per_point(&perspective, 500.0) - expected).abs() < 1e-6);
}

#[test]
fn the_floor_lies_flat_under_the_model() {
    let grid = floor(screw(), 60.0);
    assert_eq!(grid.center.z, -60.0, "at the screw's tip, not at Z = 0");
    let meshes = grid.faded_meshes();
    assert!(points(&meshes).all(|point| point.z == -60.0));

    // It reaches well past the model on every side.
    let footprint = Aabb::from_points(points(&meshes));
    assert!(footprint.min.x < -60.0 && footprint.max.x > 60.0);
    assert!(footprint.min.y < -60.0 && footprint.max.y > 60.0);

    // A model above the origin stands on its own lowest point.
    let raised = Aabb::from_points([Vec3::new(40.0, 10.0, 5.0), Vec3::new(60.0, 30.0, 25.0)]);
    let grid = floor(raised, 60.0);
    assert_eq!(grid.center.z, 5.0);
    assert!((grid.center.truncate() - Vec2::new(50.0, 20.0)).length() <= grid.minor * 5.0);
}

#[test]
fn the_spacing_follows_the_model_and_not_the_zoom() {
    let grid = floor(screw(), 40.0);
    assert_eq!(grid.minor, 5.0, "68 mm long: 5 mm cells");
    let mut radius = 0.0;
    for extent in [0.5, 15.0, 40.0, 300.0, 12000.0] {
        let zoomed = floor(screw(), extent);
        assert_eq!((zoomed.minor, zoomed.center), (grid.minor, grid.center));
        assert!(zoomed.radius >= radius, "zooming out never shrinks it");
        radius = zoomed.radius;
    }

    for (size, minor) in [(10.0, 0.5), (68.0, 5.0), (100.0, 5.0), (1000.0, 50.0)] {
        let model = Aabb::from_points([Vec3::ZERO, Vec3::new(size * 0.5, size * 0.2, size)]);
        let grid = floor(model, size);
        assert_eq!(grid.minor, minor, "{size} units tall");
        assert_eq!(round_length(grid.minor), grid.minor);
        // Centred on a major line, so that every line has a round coordinate.
        let major = grid.minor * 5.0;
        let center = grid.center.truncate();
        assert!(((center / major).round() * major - center).length() < 1e-3);
        assert!(grid.radius <= grid.minor * 300.0);
    }
}

#[test]
fn an_empty_scene_has_a_floor_through_the_origin() {
    for bounds in [
        Aabb::EMPTY,
        Aabb::from_points([Vec3::new(3.0, 4.0, 5.0)]),
        Aabb::from_points([Vec3::ZERO, Vec3::new(f32::INFINITY, 1.0, 1.0)]),
    ] {
        let grid = floor(bounds, 20.0);
        assert_eq!(grid.center, Vec3::ZERO);
        assert_eq!(grid.minor, 5.0);
        assert!(grid.radius > 100.0);
    }
}

#[test]
fn the_floor_reaches_the_edge_of_the_view_in_whole_doublings() {
    let own = floor(screw(), 1.0).radius;
    for (extent, view) in [
        (300.0, StandardView::Top),
        (200.0, StandardView::Iso),
        (40.0, StandardView::Front),
    ] {
        let camera = camera(Projection::Orthographic, Vec3::ZERO, extent, view);
        let grid = GroundGrid::under(screw(), &camera, VIEW);
        let half_view = VIEW.length() * 0.5 * units_per_point(&camera, VIEW.y);
        assert!(grid.radius >= half_view, "{view:?} at {extent}");
        let doublings = (grid.radius / own).log2();
        assert!((doublings - doublings.round()).abs() < 1e-4);
    }
    // Never more lines than the cap, however far the view zooms out.
    let farthest = floor(screw(), 1e6);
    assert_eq!(farthest.radius, farthest.minor * 300.0);
}

#[test]
fn crowded_lines_fade_out_as_the_view_zooms_out() {
    // 600 points high: an extent of 15 is 0.05 units per point.
    let close = floor(screw(), 15.0);
    assert_eq!((close.minor_opacity, close.major_opacity), (1.0, 1.0));
    let far = floor(screw(), 240.0);
    assert!(far.minor_opacity < 0.5 && far.major_opacity == 1.0);
    let very_far = floor(screw(), 1500.0);
    assert_eq!(very_far.minor_opacity, 0.0);
    assert!(very_far.major_opacity < far.major_opacity);
    assert_eq!(line_opacity(1.0, 0.0), 1.0);
}

#[test]
fn the_floor_fades_into_a_circle_with_every_fifth_line_major() {
    let grid = floor(screw(), 60.0);
    let meshes = grid.faded_meshes();
    assert_eq!(meshes.len(), GRID_FADE.len());
    assert!(
        meshes
            .iter()
            .all(|(minor, major)| !minor.edges.is_empty() && !major.edges.is_empty())
    );
    for point in points(&meshes) {
        assert!((point - grid.center).length() <= grid.radius + 1e-3);
    }
    // Each line runs along X or Y at a multiple of the spacing.
    for (minor, major) in &meshes {
        for (lines, step) in [(&minor.edges, grid.minor), (&major.edges, grid.minor * 5.0)] {
            for pair in lines.chunks(2) {
                let (a, b) = (Vec3::from_array(pair[0]), Vec3::from_array(pair[1]));
                let across = if a.x == b.x { a.x } else { a.y };
                assert!(a.x == b.x || a.y == b.y);
                assert!(((across / step).round() * step - across).abs() < 1e-3);
            }
        }
    }
    let [low, high] = grid.corners();
    let footprint = Aabb::from_points(points(&meshes));
    assert!(low.x <= footprint.min.x && high.y >= footprint.max.y);
    assert_eq!((low.z, high.z), (-60.0, -60.0));
}

#[test]
fn the_scale_bar_shows_a_round_length_at_least_sixty_points_long() {
    for units_per_point in [0.0003, 0.04, 0.7, 13.0] {
        let bar = ScaleBar::new(units_per_point, Units::Millimeters);
        assert!(
            (60.0..150.0).contains(&bar.width),
            "{} points for {units_per_point}",
            bar.width
        );
        assert_eq!(round_length(bar.length), bar.length);
        assert!((bar.width * units_per_point - bar.length).abs() < bar.length * 1e-4);
        assert_eq!(bar.label, format_length(bar.length, Units::Millimeters));
    }
    assert_eq!(ScaleBar::new(0.1, Units::Inches).label, "10 in");
}
