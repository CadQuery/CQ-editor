//! The ground grid and the scale bar (FEAT-C7). The grid is a floor on the
//! XY plane under the shown model: its height and spacing follow the model,
//! never the camera, so its lines stay put while the view turns and zooms.
//! The floor reaches as far as the view needs and fades into the distance;
//! lines that would crowd together on screen fade out instead of changing
//! spacing. The scale bar follows the zoom and shows a round length (1, 2 or
//! 5 × 10^k).

use glam::{Vec2, Vec3};

use super::camera::Camera;
use crate::scene::{Aabb, Mesh};
use crate::settings::Units;

/// Minor cells across the largest dimension of the model, at least.
const CELLS_ACROSS_MODEL: f32 = 20.0;
/// Major lines are every this many minor lines.
const MAJOR_EVERY: i32 = 5;
/// How far the floor reaches past the model's footprint, in model sizes,
/// at least.
const FLOOR_REACH: f32 = 1.5;
/// The fully visible rings of the floor cover the view at least this far
/// from the middle, in half diagonals of the view.
const VIEW_COVER: f32 = 1.4;
/// Views flatter than this (the sine of their elevation) see the floor as
/// if from this elevation: farther lines are faded out anyway.
const MIN_SLANT: f32 = 0.2;
/// Lines from the centre to each side, at most.
const MAX_LINES_PER_SIDE: i32 = 300;
/// The size taken for the model when nothing is shown, in model units.
const EMPTY_SCENE_SIZE: f32 = 100.0;
/// Lines closer than this on screen, in points, are not drawn...
const HIDDEN_SPACING: f32 = 3.0;
/// ...and lines this far apart are fully visible.
const CLEAR_SPACING: f32 = 15.0;
/// The scale bar is at least this long on screen, in points.
const MIN_SCALE_BAR: f32 = 60.0;
/// Opacity of the rings of the floor, from its centre outward.
pub const GRID_FADE: [f32; 4] = [1.0, 0.6, 0.25, 0.025];
/// Where each ring starts and ends, as a fraction of the floor's radius.
const RING_LIMITS: [f32; 5] = [0.0, 0.5, 0.7, 0.85, 1.0];

/// Model units per screen point at the point the camera looks at.
pub fn units_per_point(camera: &Camera, view_height: f32) -> f32 {
    let half_height = if camera.is_perspective() {
        camera.distance() * (camera.fov_y / 2.0).tan()
    } else {
        camera.extent
    };
    2.0 * half_height / view_height.max(1.0)
}

/// The smallest of 1, 2 and 5 × 10^k that is at least `length`.
pub fn round_length(length: f32) -> f32 {
    let length = if length.is_finite() && length > 0.0 {
        length
    } else {
        1.0
    };
    let exponent = length.log10().floor() as i32;
    [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .map(|mantissa| decimal(mantissa, exponent))
        .find(|&step| step >= length * (1.0 - 1e-5))
        .unwrap_or_else(|| decimal(1.0, exponent + 1))
}

/// `mantissa` × 10^`exponent`, as close to the decimal as an f32 gets
/// (`5 × 10⁻²` is the literal `0.05`, which f32 arithmetic would miss).
fn decimal(mantissa: f64, exponent: i32) -> f32 {
    (mantissa * 10f64.powi(exponent)) as f32
}

/// A length with its unit, without needless decimals: `2.5 mm`, `100 in`.
pub fn format_length(length: f32, units: Units) -> String {
    let text = format!("{:.6}", length);
    let text = text.trim_end_matches('0').trim_end_matches('.');
    match units.suffix() {
        "" => text.to_owned(),
        suffix => format!("{text} {suffix}"),
    }
}

/// How visible lines `spacing` apart are at `units_per_point`: 0 when they
/// would blur together on screen, 1 when they are well apart.
pub fn line_opacity(spacing: f32, units_per_point: f32) -> f32 {
    let points = spacing / units_per_point.max(f32::MIN_POSITIVE);
    ((points - HIDDEN_SPACING) / (CLEAR_SPACING - HIDDEN_SPACING)).clamp(0.0, 1.0)
}

/// The floor under the model: spacing, where it is centred and how far it
/// reaches. Two grids are drawn with the same lines when they are equal
/// apart from their opacities.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundGrid {
    /// Distance between minor lines, a round 1, 2 or 5 step.
    pub minor: f32,
    /// On a major line under the middle of the model, at its lowest point.
    pub center: Vec3,
    /// How far the floor reaches from its centre.
    pub radius: f32,
    /// How visible minor lines are at the current zoom (0 to 1).
    pub minor_opacity: f32,
    /// How visible major lines are at the current zoom (0 to 1).
    pub major_opacity: f32,
}

impl GroundGrid {
    /// The floor under a model with `bounds`, or around the origin when
    /// nothing is shown, for `camera` in a view `view` points wide and high.
    pub fn under(bounds: Aabb, camera: &Camera, view: Vec2) -> Self {
        let size = bounds.size();
        let shown = !bounds.is_empty() && size.is_finite() && size.max_element() > 0.0;
        let (largest, footprint, middle, floor) = if shown {
            (
                size.max_element(),
                size.truncate().length() * 0.5,
                bounds.center().truncate(),
                bounds.min.z,
            )
        } else {
            (EMPTY_SCENE_SIZE, 0.0, Vec2::ZERO, 0.0)
        };
        let minor = round_length(largest / CELLS_ACROSS_MODEL);
        let major = minor * MAJOR_EVERY as f32;
        // On a major line, so that every line keeps a round coordinate.
        let center = (middle / major).round() * major;
        let units_per_point = units_per_point(camera, view.y);
        // Far enough for the view, whatever it looks at, in doublings of the
        // model's own floor so that the lines are not rebuilt at every zoom.
        let own = footprint + largest * FLOOR_REACH + (middle - center).length();
        let slant = camera.forward().z.abs().max(MIN_SLANT);
        let needed = (camera.target.truncate() - center).length()
            + view.length() * 0.5 * units_per_point * VIEW_COVER / slant;
        let mut radius = own;
        while radius < needed && radius < minor * MAX_LINES_PER_SIDE as f32 {
            radius *= 2.0;
        }
        let radius = radius.min(minor * MAX_LINES_PER_SIDE as f32);
        Self {
            minor,
            center: center.extend(floor),
            radius,
            minor_opacity: line_opacity(minor, units_per_point),
            major_opacity: line_opacity(major, units_per_point),
        }
    }

    /// Whether `other` has the same lines, whatever their opacities.
    pub fn same_lines(&self, other: &Self) -> bool {
        Self {
            minor_opacity: other.minor_opacity,
            major_opacity: other.major_opacity,
            ..*self
        } == *other
    }

    /// The lines inside the floor's circle, split into rings so that its edge
    /// fades away instead of ending in a visible square. One minor/major pair
    /// per ring of [`GRID_FADE`].
    pub fn faded_meshes(&self) -> Vec<(Mesh, Mesh)> {
        let mut minor_rings = GRID_FADE.map(|_| Vec::new());
        let mut major_rings = GRID_FADE.map(|_| Vec::new());
        let limits = RING_LIMITS.map(|fraction| fraction * self.radius);
        let lines = (self.radius / self.minor).ceil() as i32;
        for index in -lines..=lines {
            let offset = index as f32 * self.minor;
            let is_major = index % MAJOR_EVERY == 0;
            for (across, along) in [(Vec3::X, Vec3::Y), (Vec3::Y, Vec3::X)] {
                for ring in 0..GRID_FADE.len() {
                    let outer = limits[ring + 1];
                    if offset.abs() >= outer {
                        continue;
                    }
                    let inner = limits[ring];
                    let low = (inner * inner - offset * offset).max(0.0).sqrt();
                    let high = (outer * outer - offset * offset).sqrt();
                    let points: &mut Vec<[f32; 3]> = if is_major {
                        &mut major_rings[ring]
                    } else {
                        &mut minor_rings[ring]
                    };
                    for (start, end) in [(-high, -low), (low, high)] {
                        if end - start > 1e-5 {
                            points.push((self.center + across * offset + along * start).to_array());
                            points.push((self.center + across * offset + along * end).to_array());
                        }
                    }
                }
            }
        }
        minor_rings
            .into_iter()
            .zip(major_rings)
            .map(|(minor, major)| (Mesh::lines(minor), Mesh::lines(major)))
            .collect()
    }

    /// Bounds of the floor, for the clipping planes.
    pub fn corners(&self) -> [Vec3; 2] {
        let reach = Vec3::new(self.radius, self.radius, 0.0);
        [self.center - reach, self.center + reach]
    }
}

/// A scale bar: a round length and how long it is on screen.
#[derive(Clone, Debug, PartialEq)]
pub struct ScaleBar {
    pub length: f32,
    /// In points.
    pub width: f32,
    pub label: String,
}

impl ScaleBar {
    /// The bar for a view with `units_per_point`, at least
    /// [`MIN_SCALE_BAR`] points long.
    pub fn new(units_per_point: f32, units: Units) -> Self {
        let length = round_length(MIN_SCALE_BAR * units_per_point);
        Self {
            length,
            width: length / units_per_point,
            label: format_length(length, units),
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rust/viewer/test_ground.rs"]
mod tests;
