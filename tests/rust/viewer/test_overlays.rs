use super::*;
use crate::viewer::camera::StandardView;

#[test]
fn cube_has_26_facets_with_outward_normals() {
    let facets = facets();
    assert_eq!(facets.len(), 26);
    for facet in facets {
        for v in &facet.vertices {
            assert!(v.dot(facet.normal) > 0.0);
        }
    }
}

#[test]
fn clicking_the_front_face_looks_from_the_front() {
    let view = Rect::from_min_size(Pos2::ZERO, Vec2::splat(400.0));
    let rotation = StandardView::Front.rotation();
    let center = NavigationCube::center(view);
    let action = NavigationCube::default().click(view, rotation, center);
    assert_eq!(
        action,
        Some(NavAction::LookFrom {
            eye_dir: Vec3::NEG_Y,
            up: Vec3::Z
        })
    );
}

#[test]
fn tilting_to_the_top_keeps_orientation() {
    let front = StandardView::Front.rotation();
    assert_eq!(up_for(Vec3::Z, front), Vec3::Y);
    assert_eq!(up_for(Vec3::NEG_Z, front), Vec3::NEG_Y);
    let right = StandardView::Right.rotation();
    assert_eq!(up_for(Vec3::Z, right), Vec3::NEG_X);
    // From the iso view the regular top view comes out.
    assert_eq!(up_for(Vec3::Z, StandardView::Iso.rotation()), Vec3::Y);
}

#[test]
fn arrows_are_hit_on_both_sides() {
    let view = Rect::from_min_size(Pos2::ZERO, Vec2::splat(400.0));
    let center = NavigationCube::center(view);
    let radius = (ARROW_INNER_RADIUS + ARROW_OUTER_RADIUS) / 2.0;
    let angle = 64f32.to_radians();
    let right = center + Vec2::new(radius * angle.cos(), -radius * angle.sin());
    let left = center + Vec2::new(-radius * angle.cos(), -radius * angle.sin());
    let cube = NavigationCube::default();
    let rotation = StandardView::Iso.rotation();
    assert_eq!(
        cube.click(view, rotation, right),
        Some(NavAction::Roll(-FRAC_PI_4))
    );
    assert_eq!(
        cube.click(view, rotation, left),
        Some(NavAction::Roll(FRAC_PI_4))
    );
}
