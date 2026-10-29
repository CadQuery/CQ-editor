use super::*;

fn close(a: Vec3, b: Vec3) -> bool {
    (a - b).length() < 1e-4
}

fn looking(view: StandardView) -> Camera {
    Camera {
        rotation: view.rotation(),
        ..Camera::default()
    }
}

#[test]
fn standard_views_look_where_expected() {
    let mut camera = looking(StandardView::Top);
    assert!(close(camera.forward(), Vec3::NEG_Z));
    assert!(close(camera.up(), Vec3::Y));
    assert!(close(camera.right(), Vec3::X));

    camera.rotation = StandardView::Front.rotation();
    assert!(close(camera.forward(), Vec3::Y));
    assert!(close(camera.up(), Vec3::Z));

    // From below, X still points right.
    camera.rotation = StandardView::Bottom.rotation();
    assert!(close(camera.right(), Vec3::X));
}

#[test]
fn turntable_keeps_z_up() {
    let mut camera = looking(StandardView::Front);
    camera.orbit_turntable(Vec2::new(120.0, 0.0));
    assert!(close(camera.up(), Vec3::Z));
    camera.orbit_turntable(Vec2::new(0.0, 50.0));
    assert!(camera.up().z > 0.5);
    assert!(camera.right().z.abs() < 1e-5);
}

#[test]
fn orbit_keeps_a_picked_point_fixed_in_both_projections() {
    for projection in [Projection::Orthographic, Projection::Perspective] {
        let mut camera = Camera {
            projection,
            target: Vec3::new(4.0, -3.0, 7.0),
            ..Camera::default()
        };
        let pivot = camera.target + camera.right() * 5.0 + camera.up() * 2.0;
        let before = project_with(camera.view_projection(1.4), pivot).unwrap();
        let original_extent = camera.extent;
        camera.orbit_turntable_about(Vec2::new(38.0, -21.0), pivot);
        let after = project_with(camera.view_projection(1.4), pivot).unwrap();
        assert!(
            (before.truncate() - after.truncate()).length() < 1e-4,
            "{projection:?}"
        );
        assert_eq!(camera.extent, original_extent);

        let before = project_with(camera.view_projection(1.4), pivot).unwrap();
        camera.orbit_trackball_about(Vec2::new(-0.2, 0.1), Vec2::new(0.2, -0.1), pivot);
        let after = project_with(camera.view_projection(1.4), pivot).unwrap();
        assert!(
            (before.truncate() - after.truncate()).length() < 1e-4,
            "{projection:?}"
        );
    }
}

#[test]
fn zoom_keeps_the_anchor_fixed() {
    let mut camera = Camera::default();
    let anchor = Vec2::new(0.5, -0.25);
    let aspect = 1.5;
    let (before, _) = camera.ray(anchor, aspect);
    let before_on_plane =
        before + camera.forward() * (camera.target - before).dot(camera.forward());
    camera.zoom(0.5, anchor, aspect);
    let (after, _) = camera.ray(anchor, aspect);
    let after_on_plane = after + camera.forward() * (camera.target - after).dot(camera.forward());
    assert!(close(before_on_plane, after_on_plane));
    assert!((camera.extent - 10.0).abs() < 1e-4);
}

#[test]
fn fit_centers_and_contains_the_box() {
    let camera = looking(StandardView::Front);
    let bounds = Aabb {
        min: Vec3::new(10.0, -1.0, 0.0),
        max: Vec3::new(30.0, 1.0, 5.0),
    };
    let pose = camera.fit_pose(bounds, 2.0);
    assert!(close(pose.target, Vec3::new(20.0, 0.0, 2.5)));
    // 20 wide at aspect 2 needs a half height of at least 5.
    assert!(pose.extent >= 5.0 && pose.extent < 6.0);
}

#[test]
fn projection_round_trips_through_rays() {
    for projection in Projection::ALL {
        let camera = Camera {
            projection,
            ..Camera::default()
        };
        let ndc = project_with(camera.view_projection(1.3), Vec3::new(3.0, 4.0, 5.0)).unwrap();
        assert!(
            ndc.z >= 0.0 && ndc.z <= 1.0,
            "{projection:?} depth {}",
            ndc.z
        );
        let (origin, dir) = camera.ray(ndc.truncate(), 1.3);
        let to_point = Vec3::new(3.0, 4.0, 5.0) - origin;
        let off_ray = to_point - dir * to_point.dot(dir);
        assert!(off_ray.length() < 1e-3, "{projection:?}");
    }
}

#[test]
fn animation_reaches_its_target() {
    let from = Camera::default().pose();
    let to = Pose {
        target: Vec3::ONE,
        rotation: StandardView::Top.rotation(),
        extent: 5.0,
    };
    let animation = Animation::new(from, to, 0.0, 0.25);
    let (mid, done) = animation.sample(0.125);
    assert!(!done && mid.extent < 20.0 && mid.extent > 5.0);
    let (end, done) = animation.sample(1.0);
    assert!(done && close(end.target, Vec3::ONE));
}

fn stereo_camera(projection: Projection) -> Camera {
    let mut camera = Camera {
        projection,
        ..Camera::default()
    };
    camera.set_pose(Pose {
        target: Vec3::new(1.0, 2.0, 3.0),
        rotation: StandardView::Iso.rotation(),
        extent: 10.0,
    });
    camera
}

#[test]
fn the_eyes_of_the_stereo_pair_agree_on_the_target_plane() {
    let camera = stereo_camera(Projection::Stereo);
    let (left, right) = (
        camera.eye_position(Eye::Left),
        camera.eye_position(Eye::Right),
    );
    assert!(((left - right).length() - STEREO_SEPARATION * camera.distance()).abs() < 1e-3);
    assert!(close((left + right) / 2.0, camera.eye()));
    assert!((right - left).normalize().dot(camera.right()) > 0.999);

    let x = |eye: Eye, point: Vec3| {
        project_with(camera.eye_view_projection(1.5, eye), point)
            .unwrap()
            .x
    };
    // No parallax at the target, or anywhere on its plane.
    let on_plane = camera.target + camera.right() * 4.0 + camera.up() * 2.0;
    for point in [camera.target, on_plane] {
        assert!((x(Eye::Left, point) - x(Eye::Right, point)).abs() < 1e-4);
        assert!((x(Eye::Left, point) - x(Eye::Center, point)).abs() < 1e-4);
    }
    // Nearer points stand out of the screen (crossed disparity: the left
    // eye sees them further right), farther ones sink behind it.
    let near = camera.target - camera.forward() * camera.distance() * 0.5;
    let far = camera.target + camera.forward() * camera.distance();
    assert!(x(Eye::Left, near) > x(Eye::Right, near) + 1e-3);
    assert!(x(Eye::Left, far) < x(Eye::Right, far) - 1e-3);
    // Stereo picks and culls from the centre.
    assert_eq!(camera.mono_eye(), Eye::Center);
    assert!(camera.is_perspective());
}

#[test]
fn mono_eye_projections_draw_and_pick_from_one_eye() {
    for (projection, eye) in [
        (Projection::MonoLeftEye, Eye::Left),
        (Projection::MonoRightEye, Eye::Right),
    ] {
        let camera = stereo_camera(projection);
        assert_eq!(camera.mono_eye(), eye);
        assert_eq!(
            camera.view_projection(1.5),
            camera.eye_view_projection(1.5, eye)
        );
        // A pick ray leaves the eye and meets what the eye draws there.
        let ndc = Vec2::new(0.3, -0.2);
        let (origin, direction) = camera.ray(ndc, 1.5);
        assert!(close(origin, camera.eye_position(eye)));
        let point = origin + direction * camera.distance() * 1.7;
        let projected = project_with(camera.view_projection(1.5), point).unwrap();
        assert!(
            (projected.truncate() - ndc).length() < 1e-4,
            "{projected:?}"
        );
    }
    let orthographic = stereo_camera(Projection::Orthographic);
    assert_eq!(orthographic.mono_eye(), Eye::Center);
    assert!(!orthographic.is_perspective());
}
