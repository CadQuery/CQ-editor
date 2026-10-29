//! What is drawn of a large model: parts outside the view are skipped and
//! parts drawn small use a simplified mesh (FEAT-C6).

use glam::{Mat4, Vec2, Vec3, Vec4};

use super::camera::Camera;
use crate::scene::Aabb;
use crate::settings::LargeModels;

/// From this many triangles shown, `LargeModels::Automatic` simplifies.
pub const LARGE_MODEL_TRIANGLES: usize = 2_000_000;
/// Parts smaller than this on screen, in pixels, use a simplified mesh.
const SMALL_PART_PX: f32 = 80.0;
/// Parts smaller than this are drawn without their edges and vertices.
const TINY_PART_PX: f32 = 6.0;

/// The planes bounding what a view-projection matrix shows (WebGPU clip
/// space: depth 0 to 1).
#[derive(Clone, Copy, Debug)]
pub struct Frustum {
    /// `dot(plane.xyz, p) + plane.w >= 0` inside.
    planes: [Vec4; 6],
}

impl Frustum {
    /// The view widened by `margin` on each side, in normalized device
    /// coordinates: edges and points are widened on screen, so their
    /// geometry may lie just outside the view and still show.
    pub fn new(view_projection: Mat4, margin: Vec2) -> Self {
        let [x, y, z, w] = [0, 1, 2, 3].map(|row| view_projection.row(row));
        let (wide, high) = (w * (1.0 + margin.x), w * (1.0 + margin.y));
        Self {
            planes: [wide + x, wide - x, high + y, high - y, z, w - z],
        }
    }

    /// Whether some of `bounds` may be in view. It can say yes for a box
    /// near a corner that is not, never no for one that is.
    pub fn intersects(&self, bounds: Aabb) -> bool {
        !bounds.is_empty()
            && self.planes.iter().all(|plane| {
                // The corner furthest along the plane's normal.
                let normal = plane.truncate();
                let corner = Vec3::select(normal.cmpge(Vec3::ZERO), bounds.max, bounds.min);
                normal.dot(corner) + plane.w >= 0.0
            })
    }
}

impl LargeModels {
    /// Whether small parts are simplified with `triangles` shown.
    pub fn active(self, triangles: usize) -> bool {
        match self {
            LargeModels::Automatic => triangles >= LARGE_MODEL_TRIANGLES,
            LargeModels::Always => true,
            LargeModels::Off => false,
        }
    }
}

/// How much of a part to draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Detail {
    Full,
    /// The simplified mesh, with its edges and vertices.
    Simplified,
    /// The simplified mesh alone.
    Faint,
}

impl Detail {
    /// The detail for a part `size_px` across on screen.
    pub fn for_size(size_px: f32) -> Self {
        if size_px < TINY_PART_PX {
            Detail::Faint
        } else if size_px < SMALL_PART_PX {
            Detail::Simplified
        } else {
            Detail::Full
        }
    }
}

/// How many pixels across `bounds` (world space) appears in a view
/// `view_height` pixels high: the diameter of its bounding sphere. Parts
/// the eye is inside of count as huge.
pub fn projected_size(camera: &Camera, view_height: f32, bounds: Aabb) -> f32 {
    if bounds.is_empty() {
        return 0.0;
    }
    let diameter = bounds.diagonal();
    let pixels_per_unit = if camera.is_perspective() {
        let depth = (bounds.center() - camera.eye()).dot(camera.forward());
        if depth <= diameter / 2.0 {
            return f32::INFINITY;
        }
        view_height / (2.0 * depth * (camera.fov_y / 2.0).tan())
    } else {
        view_height / (2.0 * camera.extent)
    };
    diameter * pixels_per_unit
}

#[cfg(test)]
#[path = "../../tests/rust/viewer/test_culling.rs"]
mod tests;
