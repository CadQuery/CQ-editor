use super::*;

fn bounds(min: [f32; 3], max: [f32; 3]) -> Aabb {
    Aabb {
        min: Vec3::from_array(min),
        max: Vec3::from_array(max),
    }
}

fn section(axis: SectionAxis, offset: f32, flipped: bool) -> Section {
    Section {
        enabled: true,
        axis,
        offset,
        flipped,
        caps: true,
    }
}

#[test]
fn the_plane_removes_one_side_of_its_axis() {
    let top_off = section(SectionAxis::Z, 2.0, false).clip_plane().unwrap();
    assert!(top_off.removes(Vec3::new(0.0, 0.0, 2.5)));
    assert!(!top_off.removes(Vec3::new(5.0, -5.0, 1.5)));

    let bottom_off = section(SectionAxis::Z, 2.0, true).clip_plane().unwrap();
    assert!(!bottom_off.removes(Vec3::new(0.0, 0.0, 2.5)));
    assert!(bottom_off.removes(Vec3::new(0.0, 0.0, 1.5)));

    let x = section(SectionAxis::X, -1.0, false).clip_plane().unwrap();
    assert!(x.removes(Vec3::new(0.0, 0.0, 0.0)));
    assert!(!x.removes(Vec3::new(-2.0, 0.0, 0.0)));

    assert!(Section::default().clip_plane().is_none(), "off by default");
}

#[test]
fn rays_keep_only_the_part_on_the_kept_side() {
    let plane = section(SectionAxis::Z, 1.0, false).clip_plane().unwrap();
    // Looking down from z = 11: the first 10 units are removed.
    let (start, end) = plane
        .kept_range(Vec3::new(0.0, 0.0, 11.0), Vec3::NEG_Z)
        .unwrap();
    assert!((start - 10.0).abs() < 1e-5 && end == f32::INFINITY);
    // Looking up from below: kept until the plane.
    let (start, end) = plane.kept_range(Vec3::ZERO, Vec3::Z).unwrap();
    assert!(start == f32::NEG_INFINITY && (end - 1.0).abs() < 1e-6);
    // Parallel to the plane: all or nothing.
    assert!(
        plane
            .kept_range(Vec3::new(0.0, 0.0, 5.0), Vec3::X)
            .is_none()
    );
    assert_eq!(
        plane.kept_range(Vec3::ZERO, Vec3::X),
        Some((f32::NEG_INFINITY, f32::INFINITY))
    );
}

#[test]
fn the_plane_moves_into_the_model_when_needed() {
    let model = bounds([-5.0, -5.0, 0.0], [5.0, 5.0, 10.0]);
    let mut cut = section(SectionAxis::Z, 50.0, false);
    assert_eq!(cut.range(model), 0.0..=10.0);
    cut.center_in(model);
    assert_eq!(cut.offset, 5.0);
    // Inside already: left where the user put it.
    cut.offset = 2.0;
    cut.center_in(model);
    assert_eq!(cut.offset, 2.0);
    // A flat model still gets a usable range.
    let flat = bounds([0.0, 0.0, 3.0], [1.0, 1.0, 3.0]);
    assert_eq!(cut.range(flat), 2.0..=4.0);
}

#[test]
fn the_cap_quad_lies_on_the_plane_around_the_whole_cross_section() {
    let model = bounds([-5.0, -2.0, 0.0], [5.0, 2.0, 10.0]);
    for (axis, flipped) in [
        (SectionAxis::X, false),
        (SectionAxis::Y, true),
        (SectionAxis::Z, false),
    ] {
        let mut cut = section(axis, 0.0, flipped);
        cut.center_in(model);
        let plane = cut.renderer_plane(model).unwrap();
        for corner in plane.cap_corners {
            assert!((plane.normal.dot(corner) - plane.offset).abs() < 1e-4);
        }
        // Every corner of the model, projected on the plane, is inside.
        let center = (plane.cap_corners[0] + plane.cap_corners[2]) / 2.0;
        let half = (plane.cap_corners[1] - plane.cap_corners[0]).length() / 2.0;
        let u = (plane.cap_corners[1] - plane.cap_corners[0]).normalize();
        let v = (plane.cap_corners[3] - plane.cap_corners[0]).normalize();
        for i in 0..8 {
            let corner = Vec3::new(
                if i & 1 == 0 { model.min.x } else { model.max.x },
                if i & 2 == 0 { model.min.y } else { model.max.y },
                if i & 4 == 0 { model.min.z } else { model.max.z },
            );
            let offset = corner - center;
            assert!(offset.dot(u).abs() <= half && offset.dot(v).abs() <= half);
        }
    }
    let off = Section::default();
    assert!(off.renderer_plane(model).is_none());
}
