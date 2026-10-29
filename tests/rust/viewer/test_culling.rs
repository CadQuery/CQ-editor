use super::*;
use crate::settings::Projection;
use crate::viewer::camera::{Pose, StandardView};

fn cube_at(center: Vec3, half: f32) -> Aabb {
    Aabb {
        min: center - Vec3::splat(half),
        max: center + Vec3::splat(half),
    }
}

fn top_camera(projection: Projection) -> Camera {
    let mut camera = Camera {
        projection,
        scene_radius: 50.0,
        ..Camera::default()
    };
    camera.set_pose(Pose {
        target: Vec3::ZERO,
        rotation: StandardView::Top.rotation(),
        extent: 10.0,
    });
    camera
}

#[test]
fn the_frustum_keeps_what_may_be_seen_and_drops_the_rest() {
    for projection in [Projection::Orthographic, Projection::Perspective] {
        let camera = top_camera(projection);
        let frustum = Frustum::new(camera.view_projection(1.0), Vec2::ZERO);
        assert!(
            frustum.intersects(cube_at(Vec3::ZERO, 1.0)),
            "{projection:?}"
        );
        // Straddling the right edge of the view.
        let edge = if projection == Projection::Orthographic {
            10.0
        } else {
            camera.distance() * (camera.fov_y / 2.0).tan()
        };
        assert!(frustum.intersects(cube_at(Vec3::new(edge, 0.0, 0.0), 0.5)));
        assert!(!frustum.intersects(cube_at(Vec3::new(edge + 2.0, 0.0, 0.0), 0.5)));
        assert!(!frustum.intersects(cube_at(Vec3::new(0.0, -3.0 * edge, 0.0), 1.0)));
        // Behind the camera.
        assert!(!frustum.intersects(cube_at(camera.eye() + Vec3::Z * 500.0, 1.0)));
        assert!(!frustum.intersects(Aabb::EMPTY));
    }
}

#[test]
fn large_model_mode_and_detail_follow_their_thresholds() {
    assert!(!LargeModels::Automatic.active(LARGE_MODEL_TRIANGLES - 1));
    assert!(LargeModels::Automatic.active(LARGE_MODEL_TRIANGLES));
    assert!(LargeModels::Always.active(0));
    assert!(!LargeModels::Off.active(usize::MAX));

    assert_eq!(Detail::for_size(500.0), Detail::Full);
    assert_eq!(Detail::for_size(SMALL_PART_PX), Detail::Full);
    assert_eq!(Detail::for_size(40.0), Detail::Simplified);
    assert_eq!(Detail::for_size(2.0), Detail::Faint);
    assert_eq!(Detail::for_size(f32::INFINITY), Detail::Full);
}

#[test]
fn parts_are_measured_in_pixels_on_screen() {
    // Orthographic: 20 units high on 1000 pixels, 50 pixels per unit.
    let camera = top_camera(Projection::Orthographic);
    let part = cube_at(Vec3::new(3.0, 0.0, 0.0), 1.0);
    let size = projected_size(&camera, 1000.0, part);
    assert!((size - part.diagonal() * 50.0).abs() < 1e-3, "{size}");

    // Perspective: smaller further away, huge around the eye.
    let camera = top_camera(Projection::Perspective);
    let near = projected_size(&camera, 1000.0, cube_at(Vec3::ZERO, 1.0));
    let far = projected_size(&camera, 1000.0, cube_at(Vec3::new(0.0, 0.0, -100.0), 1.0));
    assert!(far < near / 2.0, "{near} then {far}");
    assert_eq!(
        projected_size(&camera, 1000.0, cube_at(camera.eye(), 1.0)),
        f32::INFINITY
    );
    assert_eq!(projected_size(&camera, 1000.0, Aabb::EMPTY), 0.0);
}

#[test]
fn a_margin_keeps_what_borders_the_view() {
    let camera = top_camera(Projection::Orthographic);
    let just_outside = cube_at(Vec3::new(10.6, 0.0, 0.0), 0.5);
    let tight = Frustum::new(camera.view_projection(1.0), Vec2::ZERO);
    assert!(!tight.intersects(just_outside));
    // 2% of the half-width is 0.2 units here.
    let widened = Frustum::new(camera.view_projection(1.0), Vec2::splat(0.02));
    assert!(widened.intersects(just_outside));
    assert!(!widened.intersects(cube_at(Vec3::new(11.0, 0.0, 0.0), 0.5)));
}
