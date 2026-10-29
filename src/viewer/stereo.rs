//! Composing the two eyes of the stereo projection into one image, as the
//! original editor's stereo modes did (FEAT-C8).
//!
//! The live view composes on the GPU (`stereo.wgsl`); captures compose here,
//! from images drawn the usual way (tiles included). The tests check that
//! both give the same pixels.

use crate::settings::StereoMode;

/// The size each eye is drawn at for an image of `size` pixels: half of it
/// side by side or over-under, all of it otherwise.
pub fn eye_size(mode: StereoMode, size: [u32; 2]) -> [u32; 2] {
    let [width, height] = size.map(|side| side.max(1));
    match mode.shown() {
        StereoMode::SideBySide => [width.div_ceil(2), height],
        StereoMode::OverUnder => [width, height.div_ceil(2)],
        _ => [width, height],
    }
}

/// The mode's number in `stereo.wgsl`.
pub fn shader_mode(mode: StereoMode) -> u32 {
    match mode.shown() {
        StereoMode::QuadBuffer | StereoMode::Anaglyph => 1,
        StereoMode::RowInterlaced => 2,
        StereoMode::ColumnInterlaced => 3,
        StereoMode::ChessBoard => 4,
        StereoMode::SideBySide => 5,
        StereoMode::OverUnder => 6,
    }
}

/// Composes `left` and `right`, tightly packed sRGB RGBA images of
/// `eye_size(mode, size)`, into one of `size`.
pub fn compose(mode: StereoMode, size: [u32; 2], left: &[u8], right: &[u8]) -> Vec<u8> {
    let [width, height] = size.map(|side| side.max(1));
    let [eye_width, eye_height] = eye_size(mode, size);
    let pixel = |image: &[u8], x: u32, y: u32| {
        let start = ((y * eye_width + x) * 4) as usize;
        <[u8; 4]>::try_from(&image[start..start + 4]).expect("in the image")
    };
    let linear: Vec<f32> = (0..=255u8).map(srgb_to_linear).collect();
    let mut image = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let rgba = match mode.shown() {
                StereoMode::QuadBuffer | StereoMode::Anaglyph => {
                    let (l, r) = (pixel(left, x, y), pixel(right, x, y));
                    // Optimized red-cyan, in linear light: the left eye's red
                    // channel is made of its green and blue, so red objects
                    // do not flicker between the eyes.
                    let red = 0.7 * linear[usize::from(l[1])] + 0.3 * linear[usize::from(l[2])];
                    [linear_to_srgb(red), r[1], r[2], l[3].max(r[3])]
                }
                StereoMode::RowInterlaced => pixel(if y % 2 == 0 { left } else { right }, x, y),
                StereoMode::ColumnInterlaced => pixel(if x % 2 == 0 { left } else { right }, x, y),
                StereoMode::ChessBoard => pixel(if (x + y) % 2 == 0 { left } else { right }, x, y),
                StereoMode::SideBySide if x < eye_width => pixel(left, x, y),
                StereoMode::SideBySide => pixel(right, x - eye_width, y),
                StereoMode::OverUnder if y < eye_height => pixel(left, x, y),
                StereoMode::OverUnder => pixel(right, x, y - eye_height),
            };
            image.extend(rgba);
        }
    }
    image
}

fn srgb_to_linear(c: u8) -> f32 {
    let c = f32::from(c) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f32) -> u8 {
    let c = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (c * 255.0).round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
#[path = "../../tests/rust/viewer/test_stereo.rs"]
mod tests;
