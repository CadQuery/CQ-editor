//! Saving the view as images: screenshots of any size, with or without the
//! background, and turntable animations (FEAT-C5).

use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use glam::Quat;

use super::camera::Pose;

/// The largest side of a capture. Larger images than the GPU can draw at
/// once are drawn in tiles, so the limit is memory: 256 MB at 8192 × 8192.
pub const MAX_IMAGE_SIDE: u32 = 8192;
pub const MAX_TURNTABLE_FRAMES: u32 = 720;
pub const MAX_FRAMES_PER_SECOND: u32 = 60;

/// Standard resolution presets for screenshots and turntable exports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolutionPreset {
    Hd,
    FullHd,
    Qhd,
    Uhd4k,
    Square,
}

impl ResolutionPreset {
    pub const ALL: [Self; 5] = [Self::Hd, Self::FullHd, Self::Qhd, Self::Uhd4k, Self::Square];

    pub const fn size(self) -> [u32; 2] {
        match self {
            Self::Hd => [1280, 720],
            Self::FullHd => [1920, 1080],
            Self::Qhd => [2560, 1440],
            Self::Uhd4k => [3840, 2160],
            Self::Square => [1080, 1080],
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Hd => "1280 × 720 — HD (16:9)",
            Self::FullHd => "1920 × 1080 — Full HD (16:9)",
            Self::Qhd => "2560 × 1440 — QHD (16:9)",
            Self::Uhd4k => "3840 × 2160 — 4K (16:9)",
            Self::Square => "1080 × 1080 — Square (1:1)",
        }
    }

    pub const fn short_label(self) -> &'static str {
        match self {
            Self::Hd => "HD",
            Self::FullHd => "Full HD",
            Self::Qhd => "QHD",
            Self::Uhd4k => "4K",
            Self::Square => "1:1",
        }
    }

    #[cfg(test)]
    pub fn from_size(size: [u32; 2]) -> Option<Self> {
        Self::ALL.into_iter().find(|preset| preset.size() == size)
    }
}

/// Simplifies `[width, height]` to an aspect-ratio string such as `"16:9"` or `"1:1"`.
pub fn aspect_ratio_label([width, height]: [u32; 2]) -> String {
    let w = width.max(1);
    let h = height.max(1);
    let divisor = gcd(w, h);
    format!("{}:{}", w / divisor, h / divisor)
}

const fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

/// How large a capture is and whether it has the background.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaptureOptions {
    /// Exact size in pixels; `None` for the view's size times `scale`.
    pub size: Option<[u32; 2]>,
    pub scale: u32,
    /// Leaves the background transparent (PNG alpha).
    pub transparent: bool,
}

impl Default for CaptureOptions {
    fn default() -> Self {
        Self {
            size: None,
            scale: 1,
            transparent: false,
        }
    }
}

impl CaptureOptions {
    /// The size of the image for a view of `view` pixels. Both sides
    /// shrink by the same factor when one would exceed [`MAX_IMAGE_SIDE`].
    pub fn image_size(&self, view: [u32; 2]) -> [u32; 2] {
        let [width, height] = match self.size {
            Some(size) => size.map(|side| side.max(1)),
            None => view.map(|side| side.max(1).saturating_mul(self.scale.max(1))),
        };
        let largest = width.max(height);
        if largest <= MAX_IMAGE_SIDE {
            return [width, height];
        }
        let shrink = f64::from(MAX_IMAGE_SIDE) / f64::from(largest);
        [width, height]
            .map(|side| ((f64::from(side) * shrink).round() as u32).clamp(1, MAX_IMAGE_SIDE))
    }

    /// How much wider edges and points are drawn, so that the image looks
    /// like the view: what is shown is set by its height.
    pub fn pixel_scale(&self, view: [u32; 2]) -> f32 {
        self.image_size(view)[1] as f32 / view[1].max(1) as f32
    }
}

/// The renderer blends over a transparent black background, which leaves
/// premultiplied colors; PNG stores straight alpha. Colors are sRGB, so
/// they are divided in linear space.
pub fn unpremultiply(pixels: &mut [u8]) {
    let linear: Vec<f32> = (0..=255u8).map(srgb_to_linear).collect();
    for pixel in pixels.as_chunks_mut::<4>().0 {
        let alpha = pixel[3];
        if alpha == 0 || alpha == 255 {
            continue;
        }
        let alpha = f32::from(alpha) / 255.0;
        for channel in &mut pixel[..3] {
            *channel = linear_to_srgb((linear[usize::from(*channel)] / alpha).min(1.0));
        }
    }
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

/// Saves a rendered image as PNG, straightening the alpha of a capture
/// with a transparent background.
pub fn save_png(
    path: &Path,
    size: [u32; 2],
    mut pixels: Vec<u8>,
    transparent: bool,
) -> Result<(), String> {
    if transparent {
        unpremultiply(&mut pixels);
    }
    image::save_buffer(
        path,
        &pixels,
        size[0],
        size[1],
        image::ExtendedColorType::Rgba8,
    )
    .map_err(|error| format!("Cannot write {}: {error}", path.display()))
}

/// Prepares a rendered RGBA buffer for the system clipboard, straightening
/// premultiplied alpha when `transparent` is set.
pub fn prepare_clipboard_image(
    size: [u32; 2],
    mut pixels: Vec<u8>,
    transparent: bool,
) -> egui::ColorImage {
    if transparent {
        unpremultiply(&mut pixels);
    }
    egui::ColorImage::from_rgba_unmultiplied([size[0] as usize, size[1] as usize], &pixels)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AnimationFormat {
    /// One animated PNG, which browsers play.
    #[default]
    AnimatedPng,
    /// One PNG per frame, `name_0001.png` and on, for video tools.
    PngFiles,
}

/// One turn of the camera around the model.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TurntableOptions {
    pub frames: u32,
    pub frames_per_second: u32,
    /// The model turns clockwise seen from above (the camera goes the
    /// other way round).
    pub clockwise: bool,
    pub format: AnimationFormat,
}

impl Default for TurntableOptions {
    fn default() -> Self {
        Self {
            frames: 72,
            frames_per_second: 24,
            clockwise: false,
            format: AnimationFormat::AnimatedPng,
        }
    }
}

/// The camera of frame `index` of `frames`: `start` turned about the
/// vertical (Z) axis through the point it looks at.
pub fn turntable_pose(start: Pose, index: u32, frames: u32, clockwise: bool) -> Pose {
    let turn = std::f32::consts::TAU * index as f32 / frames.max(1) as f32;
    // Turning the camera one way turns the model the other way.
    let angle = if clockwise { turn } else { -turn };
    Pose {
        rotation: (Quat::from_rotation_z(angle) * start.rotation).normalize(),
        ..start
    }
}

/// `turntable.png` → `turntable_0001.png` for the first frame.
pub fn frame_path(path: &Path, index: u32) -> PathBuf {
    let stem = path
        .file_stem()
        .map_or_else(|| "frame".into(), |stem| stem.to_string_lossy());
    path.with_file_name(format!("{stem}_{:04}.png", index + 1))
}

enum Output {
    Animated(png::Writer<BufWriter<File>>),
    Files(Vec<PathBuf>),
}

/// A turntable animation being written, one frame at a time so that the
/// application stays responsive and the export can be cancelled.
pub struct TurntableExport {
    path: PathBuf,
    turntable: TurntableOptions,
    transparent: bool,
    start: Pose,
    size: [u32; 2],
    pixel_scale: f32,
    next: u32,
    output: Output,
}

impl TurntableExport {
    /// Starts an export of the view of `view` pixels seen from `start`.
    pub fn start(
        path: PathBuf,
        options: CaptureOptions,
        turntable: TurntableOptions,
        start: Pose,
        view: [u32; 2],
    ) -> Result<Self, String> {
        let turntable = TurntableOptions {
            frames: turntable.frames.clamp(1, MAX_TURNTABLE_FRAMES),
            frames_per_second: turntable.frames_per_second.clamp(1, MAX_FRAMES_PER_SECOND),
            ..turntable
        };
        let size = options.image_size(view);
        let output = match turntable.format {
            AnimationFormat::AnimatedPng => {
                let error =
                    |error: png::EncodingError| format!("Cannot write {}: {error}", path.display());
                let file = File::create(&path)
                    .map_err(|error| format!("Cannot create {}: {error}", path.display()))?;
                let mut encoder = png::Encoder::new(BufWriter::new(file), size[0], size[1]);
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                // Played in a loop, each frame replacing the previous one
                // (which matters with a transparent background).
                encoder.set_animated(turntable.frames, 0).map_err(error)?;
                encoder
                    .set_frame_delay(1, turntable.frames_per_second as u16)
                    .map_err(error)?;
                encoder.set_blend_op(png::BlendOp::Source).map_err(error)?;
                encoder
                    .set_dispose_op(png::DisposeOp::Background)
                    .map_err(error)?;
                Output::Animated(encoder.write_header().map_err(error)?)
            }
            AnimationFormat::PngFiles => Output::Files(Vec::new()),
        };
        Ok(Self {
            path,
            turntable,
            transparent: options.transparent,
            start,
            size,
            pixel_scale: options.pixel_scale(view),
            next: 0,
            output,
        })
    }

    /// Frames written and frames in all.
    pub fn progress(&self) -> (u32, u32) {
        (self.next, self.turntable.frames)
    }

    pub fn size(&self) -> [u32; 2] {
        self.size
    }

    pub fn pixel_scale(&self) -> f32 {
        self.pixel_scale
    }

    pub fn transparent(&self) -> bool {
        self.transparent
    }

    /// The camera of the next frame to write.
    pub fn next_pose(&self) -> Pose {
        turntable_pose(
            self.start,
            self.next,
            self.turntable.frames,
            self.turntable.clockwise,
        )
    }

    /// Writes the next frame and tells whether it was the last one.
    pub fn write_frame(&mut self, mut pixels: Vec<u8>) -> Result<bool, String> {
        if self.transparent {
            unpremultiply(&mut pixels);
        }
        match &mut self.output {
            Output::Animated(writer) => writer
                .write_image_data(&pixels)
                .map_err(|error| format!("Cannot write {}: {error}", self.path.display()))?,
            Output::Files(written) => {
                let path = frame_path(&self.path, self.next);
                image::save_buffer(
                    &path,
                    &pixels,
                    self.size[0],
                    self.size[1],
                    image::ExtendedColorType::Rgba8,
                )
                .map_err(|error| format!("Cannot write {}: {error}", path.display()))?;
                written.push(path);
            }
        }
        self.next += 1;
        Ok(self.next >= self.turntable.frames)
    }

    /// Completes the file once every frame is written, and says where it is.
    pub fn finish(self) -> Result<String, String> {
        let frames = self.turntable.frames;
        match self.output {
            Output::Animated(writer) => {
                writer
                    .finish()
                    .map_err(|error| format!("Cannot write {}: {error}", self.path.display()))?;
                Ok(format!(
                    "Turntable saved to {} ({frames} frames)",
                    self.path.display()
                ))
            }
            Output::Files(_) => Ok(format!(
                "Turntable saved as {frames} images, {} and on",
                frame_path(&self.path, 0).display()
            )),
        }
    }

    /// Stops and removes what was written.
    pub fn cancel(self) {
        match self.output {
            Output::Animated(writer) => {
                drop(writer);
                let _ = std::fs::remove_file(&self.path);
            }
            Output::Files(written) => {
                for path in written {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rust/viewer/test_capture.rs"]
mod tests;
