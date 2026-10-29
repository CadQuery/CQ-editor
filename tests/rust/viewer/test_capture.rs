use super::*;
use crate::viewer::camera::StandardView;
use glam::Vec3;

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cq-editor-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn image_sizes_follow_the_view_and_stay_within_the_limit() {
    let view = [800, 600];
    let at = |scale| CaptureOptions {
        scale,
        ..CaptureOptions::default()
    };
    assert_eq!(at(1).image_size(view), [800, 600]);
    assert_eq!(at(4).image_size(view), [3200, 2400]);
    assert_eq!(at(4).pixel_scale(view), 4.0);

    let custom = CaptureOptions {
        size: Some([1920, 1080]),
        ..CaptureOptions::default()
    };
    assert_eq!(custom.image_size(view), [1920, 1080]);
    assert_eq!(custom.pixel_scale(view), 1.8, "set by the heights");

    // Too large: both sides shrink together.
    let huge = at(16).image_size([3000, 1500]);
    assert_eq!(huge, [MAX_IMAGE_SIDE, MAX_IMAGE_SIDE / 2]);
    let degenerate = CaptureOptions {
        size: Some([0, 20_000]),
        ..CaptureOptions::default()
    };
    assert_eq!(degenerate.image_size(view), [1, MAX_IMAGE_SIDE]);
}

#[test]
fn transparent_pixels_get_straight_alpha_in_srgb() {
    let mut pixels = vec![
        0, 0, 0, 0, // background
        200, 100, 50, 255, // opaque
        0, 0, 0, 128, // half covered by black
        // White at 50% coverage over transparent black, premultiplied in
        // linear space: 0.5 linear is 188 in sRGB.
        188, 188, 188, 128,
    ];
    unpremultiply(&mut pixels);
    assert_eq!(&pixels[..12], [0, 0, 0, 0, 200, 100, 50, 255, 0, 0, 0, 128]);
    assert!(
        pixels[12..15].iter().all(|&c| c >= 253),
        "white again: {:?}",
        &pixels[12..]
    );
    assert_eq!(pixels[15], 128);
}

#[test]
fn the_turntable_turns_the_camera_once_around_the_vertical_axis() {
    let start = Pose {
        target: Vec3::new(1.0, 2.0, 3.0),
        rotation: StandardView::Front.rotation(),
        extent: 5.0,
    };
    let back = |pose: Pose| pose.rotation * Vec3::Z;
    assert_eq!(turntable_pose(start, 0, 8, false), start);

    // A quarter turn: same target, distance and height, eye 90° around Z.
    for clockwise in [false, true] {
        let quarter = turntable_pose(start, 2, 8, clockwise);
        assert_eq!(quarter.target, start.target);
        assert_eq!(quarter.extent, start.extent);
        let (from, to) = (back(start), back(quarter));
        assert!((from.z - to.z).abs() < 1e-5, "the eye stays at its height");
        assert!(from.dot(to).abs() < 1e-5, "a right angle");
        // Seen from above, the camera goes counterclockwise when the model
        // turns clockwise.
        let turn = from.cross(to).z;
        assert_eq!(turn > 0.0, clockwise);
    }
    // The last frame stops one step short of the first.
    let last = turntable_pose(start, 7, 8, false);
    assert!(back(last).angle_between(back(start)) > 0.7);
}

#[test]
fn frames_are_numbered_after_the_chosen_name() {
    assert_eq!(
        frame_path(Path::new("out/spin.png"), 0),
        Path::new("out/spin_0001.png")
    );
    assert_eq!(
        frame_path(Path::new("spin.png"), 71),
        Path::new("spin_0072.png")
    );
}

fn small_export(path: PathBuf, format: AnimationFormat, transparent: bool) -> TurntableExport {
    TurntableExport::start(
        path,
        CaptureOptions {
            transparent,
            ..CaptureOptions::default()
        },
        TurntableOptions {
            frames: 3,
            frames_per_second: 12,
            clockwise: false,
            format,
        },
        Pose {
            target: Vec3::ZERO,
            rotation: StandardView::Iso.rotation(),
            extent: 1.0,
        },
        [4, 2],
    )
    .unwrap()
}

#[test]
fn an_animated_png_holds_every_frame_and_loops() {
    let dir = scratch_dir("apng");
    let path = dir.join("spin.png");
    let mut export = small_export(path.clone(), AnimationFormat::AnimatedPng, true);
    assert_eq!(export.size(), [4, 2]);
    for frame in 0..3u8 {
        assert_eq!(export.progress(), (u32::from(frame), 3));
        let done = export
            .write_frame([frame * 80, 0, 0, 255].repeat(8))
            .unwrap();
        assert_eq!(done, frame == 2);
    }
    assert!(export.finish().unwrap().contains("3 frames"));

    let decoder = png::Decoder::new(std::io::BufReader::new(File::open(&path).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    let control = reader.info().animation_control.unwrap();
    assert_eq!((control.num_frames, control.num_plays), (3, 0));
    let mut buffer = vec![0; reader.output_buffer_size().unwrap()];
    let mut reds = Vec::new();
    for _ in 0..3 {
        reader.next_frame(&mut buffer).unwrap();
        reds.push(buffer[0]);
        let frame = reader.info().frame_control.unwrap();
        assert_eq!((frame.delay_num, frame.delay_den), (1, 12));
    }
    assert_eq!(reds, [0, 80, 160]);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn png_files_are_written_per_frame_and_a_cancel_removes_them() {
    let dir = scratch_dir("frames");
    let path = dir.join("spin.png");
    let mut export = small_export(path.clone(), AnimationFormat::PngFiles, false);
    for _ in 0..3 {
        export.write_frame([10, 20, 30, 255].repeat(8)).unwrap();
    }
    assert!(export.finish().unwrap().contains("spin_0001.png"));
    for index in 0..3 {
        let image = image::open(frame_path(&path, index)).unwrap().to_rgba8();
        assert_eq!(image.dimensions(), (4, 2));
        assert_eq!(image.get_pixel(0, 0).0, [10, 20, 30, 255]);
    }

    // Cancelled half-way: nothing is left behind, in either format.
    let cancelled = dir.join("cancelled.png");
    let mut export = small_export(cancelled.clone(), AnimationFormat::PngFiles, false);
    export.write_frame(vec![0; 32]).unwrap();
    assert!(frame_path(&cancelled, 0).exists());
    export.cancel();
    assert!(!frame_path(&cancelled, 0).exists());
    let mut export = small_export(cancelled.clone(), AnimationFormat::AnimatedPng, false);
    export.write_frame(vec![0; 32]).unwrap();
    export.cancel();
    assert!(!cancelled.exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_screenshot_does_not_interrupt_a_turntable_export() {
    let dir = scratch_dir("busy");
    let mut viewer = crate::viewer::Viewer::default();
    let turntable = TurntableOptions {
        frames: 4,
        ..TurntableOptions::default()
    };
    let path = dir.join("spin.png");
    viewer
        .start_turntable(path.clone(), CaptureOptions::default(), turntable)
        .unwrap();
    assert_eq!(viewer.turntable_progress(), Some((0, 4)));
    assert!(!viewer.request_image(dir.join("shot.png"), CaptureOptions::default()));
    assert_eq!(viewer.turntable_progress(), Some((0, 4)));
    viewer.cancel_turntable();
    assert_eq!(viewer.turntable_progress(), None);
    assert!(!path.exists(), "the partial animation is deleted");
    assert!(viewer.request_image(dir.join("shot.png"), CaptureOptions::default()));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn resolution_presets_and_clipboard_preparation_preserve_dimensions_and_aspect_ratio() {
    assert_eq!(ResolutionPreset::Hd.size(), [1280, 720]);
    assert_eq!(ResolutionPreset::FullHd.size(), [1920, 1080]);
    assert_eq!(ResolutionPreset::Qhd.size(), [2560, 1440]);
    assert_eq!(ResolutionPreset::Uhd4k.size(), [3840, 2160]);
    assert_eq!(ResolutionPreset::Square.size(), [1080, 1080]);
    assert_eq!(
        ResolutionPreset::from_size([1920, 1080]),
        Some(ResolutionPreset::FullHd)
    );
    assert_eq!(aspect_ratio_label([1920, 1080]), "16:9");
    assert_eq!(aspect_ratio_label([1080, 1080]), "1:1");

    let image = prepare_clipboard_image([2, 1], vec![188, 188, 188, 128, 10, 20, 30, 255], true);
    assert_eq!(image.size, [2, 1]);
    assert!(image.pixels[0].to_srgba_unmultiplied()[0] >= 253);
    assert_eq!(
        image.pixels[1],
        egui::Color32::from_rgba_unmultiplied(10, 20, 30, 255)
    );

    let mut viewer = crate::viewer::Viewer::default();
    assert!(viewer.request_clipboard_image(CaptureOptions::default()));
}
