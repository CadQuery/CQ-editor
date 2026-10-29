use super::*;

/// An image of `size` whose pixels say which eye and where: red is the
/// eye (10 left, 20 right), green and blue the coordinates.
fn coded(eye: u8, size: [u32; 2]) -> Vec<u8> {
    let mut image = Vec::new();
    for y in 0..size[1] {
        for x in 0..size[0] {
            image.extend([eye, x as u8, y as u8, 255]);
        }
    }
    image
}

fn at(image: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let start = ((y * width + x) * 4) as usize;
    <[u8; 4]>::try_from(&image[start..start + 4]).unwrap()
}

fn composed(mode: StereoMode, size: [u32; 2]) -> Vec<u8> {
    let eye = eye_size(mode, size);
    compose(mode, size, &coded(10, eye), &coded(20, eye))
}

#[test]
fn each_eye_is_drawn_at_the_size_its_half_or_whole_image_needs() {
    assert_eq!(eye_size(StereoMode::SideBySide, [9, 4]), [5, 4]);
    assert_eq!(eye_size(StereoMode::OverUnder, [9, 5]), [9, 3]);
    for mode in [
        StereoMode::QuadBuffer,
        StereoMode::Anaglyph,
        StereoMode::RowInterlaced,
        StereoMode::ColumnInterlaced,
        StereoMode::ChessBoard,
    ] {
        assert_eq!(eye_size(mode, [9, 5]), [9, 5]);
    }
    assert_eq!(eye_size(StereoMode::Anaglyph, [0, 0]), [1, 1]);
}

#[test]
fn interlaced_modes_alternate_the_eyes_pixel_by_pixel() {
    let size = [4, 4];
    let rows = composed(StereoMode::RowInterlaced, size);
    let columns = composed(StereoMode::ColumnInterlaced, size);
    let chess = composed(StereoMode::ChessBoard, size);
    for y in 0..4 {
        for x in 0..4 {
            let eye = |even: bool| if even { 10 } else { 20 };
            assert_eq!(at(&rows, 4, x, y), [eye(y % 2 == 0), x as u8, y as u8, 255]);
            assert_eq!(at(&columns, 4, x, y)[0], eye(x % 2 == 0));
            assert_eq!(at(&chess, 4, x, y)[0], eye((x + y) % 2 == 0));
        }
    }
}

#[test]
fn side_by_side_and_over_under_place_each_eye_in_its_half() {
    let image = composed(StereoMode::SideBySide, [9, 2]);
    assert_eq!(image.len(), 9 * 2 * 4);
    // Columns 0-4 are the left eye's, 5-8 the right eye's first four.
    assert_eq!(at(&image, 9, 4, 1), [10, 4, 1, 255]);
    assert_eq!(at(&image, 9, 5, 1), [20, 0, 1, 255]);
    assert_eq!(at(&image, 9, 8, 0), [20, 3, 0, 255]);

    let image = composed(StereoMode::OverUnder, [3, 5]);
    assert_eq!(at(&image, 3, 2, 2), [10, 2, 2, 255]);
    assert_eq!(at(&image, 3, 2, 3), [20, 2, 0, 255]);
    assert_eq!(at(&image, 3, 0, 4), [20, 0, 1, 255]);
}

#[test]
fn the_anaglyph_takes_red_from_the_left_eye_and_cyan_from_the_right() {
    let one = |pixel: [u8; 4]| pixel.to_vec();
    let mix = |left: [u8; 4], right: [u8; 4]| {
        compose(StereoMode::Anaglyph, [1, 1], &one(left), &one(right))
    };
    // White on the left: full red. The right eye gives green and blue.
    assert_eq!(
        mix([255, 255, 255, 255], [9, 40, 50, 255]),
        [255, 40, 50, 255]
    );
    // Red on the left gives no red: the optimized anaglyph reads the left
    // eye's green and blue, so red objects do not flicker.
    assert_eq!(mix([255, 0, 0, 255], [0, 0, 0, 0]), [0, 0, 0, 255]);
    // 0.7 · green, in linear light: green 255 is 0.7 linear, 218 in sRGB.
    assert_eq!(mix([0, 255, 0, 100], [0, 0, 0, 30])[0], 218);
    assert_eq!(mix([0, 255, 0, 100], [0, 0, 0, 30])[3], 100);
    // Quad buffer cannot be shown and composes as the anaglyph.
    let (left, right) = (coded(10, [3, 2]), coded(20, [3, 2]));
    assert_eq!(
        compose(StereoMode::QuadBuffer, [3, 2], &left, &right),
        compose(StereoMode::Anaglyph, [3, 2], &left, &right)
    );
    assert_eq!(StereoMode::QuadBuffer.shown(), StereoMode::Anaglyph);
    assert_eq!(
        shader_mode(StereoMode::QuadBuffer),
        shader_mode(StereoMode::Anaglyph)
    );
}
