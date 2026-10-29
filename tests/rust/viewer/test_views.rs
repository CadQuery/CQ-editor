use super::*;
use crate::viewer::camera::StandardView;

fn viewer_at(view: StandardView, target: Vec3, extent: f32) -> Viewer {
    let mut viewer = Viewer::default();
    viewer.camera.set_pose(Pose {
        target,
        rotation: view.rotation(),
        extent,
    });
    viewer
}

#[test]
fn views_are_saved_named_and_replaced_by_name() {
    let mut viewer = viewer_at(StandardView::Top, Vec3::new(1.0, 2.0, 3.0), 7.0);
    assert_eq!(viewer.save_view(None, Projection::Orthographic), "View 1");
    assert_eq!(
        viewer.save_view(Some("  Detail  "), Projection::Perspective),
        "Detail"
    );
    assert_eq!(
        viewer.save_view(Some(" "), Projection::Orthographic),
        "View 2"
    );
    let names: Vec<_> = viewer.named_views.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(names, ["View 1", "Detail", "View 2"]);

    let detail = &viewer.named_views[1];
    assert_eq!(detail.target, [1.0, 2.0, 3.0]);
    assert_eq!(detail.extent, 7.0);
    assert_eq!(detail.projection, Projection::Perspective);

    // The same name replaces that view where it is.
    viewer.camera.extent = 2.0;
    viewer.save_view(Some("Detail"), Projection::Orthographic);
    assert_eq!(viewer.named_views.len(), 3);
    assert_eq!(viewer.named_views[1].extent, 2.0);
    assert_eq!(viewer.named_views[1].projection, Projection::Orthographic);

    // A freed name is reused.
    assert_eq!(viewer.delete_view(0).unwrap().name, "View 1");
    assert!(viewer.delete_view(5).is_none());
    assert_eq!(viewer.save_view(None, Projection::Orthographic), "View 1");
}

#[test]
fn going_to_a_view_restores_its_camera_and_projection() {
    let mut viewer = viewer_at(StandardView::Front, Vec3::new(5.0, 0.0, 0.0), 3.0);
    viewer.save_view(Some("front"), Projection::Perspective);
    viewer.set_standard_view(StandardView::Iso, false, 0.0);
    viewer.camera.target = Vec3::ZERO;
    viewer.camera.extent = 50.0;

    assert_eq!(
        viewer.go_to_view(0, false, 0.0),
        Some(Projection::Perspective)
    );
    let pose = viewer.camera.pose();
    assert_eq!(pose.target, Vec3::new(5.0, 0.0, 0.0));
    assert_eq!(pose.extent, 3.0);
    assert!(pose.rotation.angle_between(StandardView::Front.rotation()) < 1e-4);
    assert_eq!(viewer.go_to_view(1, false, 0.0), None);

    // Animated, the view is reached at the end and saved as the target.
    viewer.set_standard_view(StandardView::Top, false, 0.0);
    assert!(viewer.go_to_view(0, true, 10.0).is_some());
    let (pose, done) = viewer.animation.as_ref().unwrap().sample(10.0 + 1.0);
    assert!(done && pose.target == Vec3::new(5.0, 0.0, 0.0));
    viewer.update_view(0, Projection::Orthographic);
    assert_eq!(viewer.named_views[0].target, [5.0, 0.0, 0.0]);
    assert_eq!(viewer.named_views[0].name, "front");
    assert!(viewer.update_view(3, Projection::Orthographic).is_none());
}

#[test]
fn damaged_views_from_the_settings_file_are_refused() {
    let mut viewer = Viewer::default();
    let before = viewer.camera.pose();
    for damaged in [
        NamedView {
            name: "zero rotation".into(),
            extent: 1.0,
            ..NamedView::default()
        },
        NamedView {
            name: "no extent".into(),
            rotation: [0.0, 0.0, 0.0, 1.0],
            ..NamedView::default()
        },
        NamedView {
            name: "not a number".into(),
            rotation: [0.0, 0.0, 0.0, 1.0],
            target: [f32::NAN, 0.0, 0.0],
            extent: 1.0,
            ..NamedView::default()
        },
    ] {
        viewer.named_views = vec![damaged];
        assert_eq!(viewer.go_to_view(0, false, 0.0), None);
        assert_eq!(viewer.camera.pose().target, before.target);
    }
}
