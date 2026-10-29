//! The section plane of the 3D view: an axis-aligned plane that cuts the
//! model away on one side, with the cut faces filled.

use std::ops::RangeInclusive;

use glam::Vec3;

use super::renderer::SectionPlane;
use crate::scene::Aabb;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SectionAxis {
    X,
    Y,
    #[default]
    Z,
}

impl SectionAxis {
    pub const ALL: [Self; 3] = [Self::X, Self::Y, Self::Z];

    pub fn label(self) -> &'static str {
        match self {
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
        }
    }

    pub fn direction(self) -> Vec3 {
        match self {
            Self::X => Vec3::X,
            Self::Y => Vec3::Y,
            Self::Z => Vec3::Z,
        }
    }
}

/// A plane normal to an axis that removes the model on one side of it.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub enabled: bool,
    pub axis: SectionAxis,
    /// Position of the plane along the axis, in model units.
    pub offset: f32,
    /// Remove the other side: what lies below the plane instead of above.
    pub flipped: bool,
    /// Fill the cut faces of solids.
    pub caps: bool,
}

impl Default for Section {
    fn default() -> Self {
        Self {
            enabled: false,
            axis: SectionAxis::Z,
            offset: 0.0,
            flipped: false,
            caps: true,
        }
    }
}

/// The removed half-space: points with `normal · p > offset`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipPlane {
    pub normal: Vec3,
    pub offset: f32,
}

impl ClipPlane {
    pub fn removes(&self, point: Vec3) -> bool {
        self.normal.dot(point) > self.offset
    }

    /// The parameters `t` for which `origin + t * direction` is kept, as
    /// an inclusive range; `None` when the whole ray is removed.
    pub fn kept_range(&self, origin: Vec3, direction: Vec3) -> Option<(f32, f32)> {
        let start = self.normal.dot(origin) - self.offset;
        let rate = self.normal.dot(direction);
        if rate.abs() < 1e-12 {
            return (start <= 0.0).then_some((f32::NEG_INFINITY, f32::INFINITY));
        }
        let crossing = -start / rate;
        Some(if rate > 0.0 {
            (f32::NEG_INFINITY, crossing)
        } else {
            (crossing, f32::INFINITY)
        })
    }
}

impl Section {
    /// The plane while the section is on.
    pub fn clip_plane(&self) -> Option<ClipPlane> {
        self.enabled.then(|| {
            let direction = self.axis.direction();
            if self.flipped {
                ClipPlane {
                    normal: -direction,
                    offset: -self.offset,
                }
            } else {
                ClipPlane {
                    normal: direction,
                    offset: self.offset,
                }
            }
        })
    }

    /// Where the plane can usefully be: across the model along the axis.
    pub fn range(&self, bounds: Aabb) -> RangeInclusive<f32> {
        if bounds.is_empty() {
            return -10.0..=10.0;
        }
        let axis = self.axis.direction();
        let (low, high) = (bounds.min.dot(axis), bounds.max.dot(axis));
        if high - low < 1e-6 {
            low - 1.0..=high + 1.0
        } else {
            low..=high
        }
    }

    /// Moves the plane into the model when it is outside, e.g. when the
    /// section is turned on or the axis changes.
    pub fn center_in(&mut self, bounds: Aabb) {
        let range = self.range(bounds);
        if !range.contains(&self.offset) {
            self.offset = (range.start() + range.end()) / 2.0;
        }
    }

    /// The plane for the renderer, with a cap quad covering every place
    /// where it can cut the model.
    pub fn renderer_plane(&self, bounds: Aabb) -> Option<SectionPlane> {
        let plane = self.clip_plane()?;
        let (center, radius) = if bounds.is_empty() {
            (Vec3::ZERO, 10.0)
        } else {
            (bounds.center(), (bounds.diagonal() * 0.5).max(1e-3))
        };
        let on_plane = center - plane.normal * (plane.normal.dot(center) - plane.offset);
        let u = plane.normal.any_orthonormal_vector();
        let v = plane.normal.cross(u);
        let half = radius * 1.5;
        Some(SectionPlane {
            normal: plane.normal,
            offset: plane.offset,
            caps: self.caps,
            cap_corners: [
                on_plane - u * half - v * half,
                on_plane + u * half - v * half,
                on_plane + u * half + v * half,
                on_plane - u * half + v * half,
            ],
        })
    }
}

#[cfg(test)]
#[path = "../../tests/rust/viewer/test_section.rs"]
mod tests;
