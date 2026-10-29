//! 2D overlays of the 3D view: the navigation cube with its roll arrows and
//! the axes indicator.
//!
//! The navigation cube is a chamfered cube in the top-right corner that
//! always shows the camera orientation. Its 6 faces, 12 edges and 8 corners
//! are clickable and turn the camera to look from that direction; the two
//! curved arrows above it roll the view by 45 degrees.

use std::f32::consts::FRAC_PI_4;
use std::sync::OnceLock;

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2, epaint::TextShape};
use glam::{Quat, Vec3};

/// Distance of the cube center from the view's top-right corner (points).
const CORNER_OFFSET: f32 = 85.0;
/// Half the cube's edge length (points).
const CUBE_HALF_SIZE: f32 = 29.0;
/// Half size of the face squares relative to the cube; the rest is chamfer.
const FACE_INSET: f32 = 0.68;
const ARROW_INNER_RADIUS: f32 = 56.0;
const ARROW_OUTER_RADIUS: f32 = 66.0;
/// Angles (degrees from the screen's x axis) where an arrow's tail starts
/// and where its head begins.
const ARROW_TAIL_ANGLE: f32 = 76.0;
const ARROW_HEAD_ANGLE: f32 = 52.0;
const ARROW_TIP_SWEEP: f32 = 13.0;
const ARROW_HEAD_OVERHANG: f32 = 5.0;
const ARC_SEGMENTS: usize = 12;

const AXES_LENGTH: f32 = 34.0;
const AXES_MARGIN: f32 = 52.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum FacetKind {
    Face,
    Edge,
    Corner,
}

struct Facet {
    normal: Vec3,
    vertices: Vec<Vec3>,
    kind: FacetKind,
    /// Text and its reading direction, for the six faces.
    label: Option<(&'static str, Vec3)>,
}

fn facets() -> &'static [Facet] {
    static FACETS: OnceLock<Vec<Facet>> = OnceLock::new();
    FACETS.get_or_init(build_facets)
}

fn build_facets() -> Vec<Facet> {
    let a = FACE_INSET;
    let axis = |i: usize| Vec3::AXES[i];
    let mut facets = Vec::new();

    for (normal, label, reading) in [
        (Vec3::Z, "TOP", Vec3::X),
        (Vec3::NEG_Z, "BOTTOM", Vec3::X),
        (Vec3::NEG_Y, "FRONT", Vec3::X),
        (Vec3::Y, "BACK", Vec3::NEG_X),
        (Vec3::X, "RIGHT", Vec3::Y),
        (Vec3::NEG_X, "LEFT", Vec3::NEG_Y),
    ] {
        let (u, v) = (reading, normal.cross(reading));
        facets.push(Facet {
            normal,
            vertices: vec![
                normal + (-u - v) * a,
                normal + (u - v) * a,
                normal + (u + v) * a,
                normal + (-u + v) * a,
            ],
            kind: FacetKind::Face,
            label: Some((label, reading)),
        });
    }

    for i in 0..3 {
        for j in (i + 1)..3 {
            let k = 3 - i - j;
            for si in [-1.0f32, 1.0] {
                for sj in [-1.0f32, 1.0] {
                    let (ei, ej, ek) = (axis(i) * si, axis(j) * sj, axis(k));
                    facets.push(Facet {
                        normal: (ei + ej).normalize(),
                        vertices: vec![
                            ei + ej * a - ek * a,
                            ei + ej * a + ek * a,
                            ei * a + ej + ek * a,
                            ei * a + ej - ek * a,
                        ],
                        kind: FacetKind::Edge,
                        label: None,
                    });
                }
            }
        }
    }

    for sx in [-1.0f32, 1.0] {
        for sy in [-1.0f32, 1.0] {
            for sz in [-1.0f32, 1.0] {
                let (x, y, z) = (Vec3::X * sx, Vec3::Y * sy, Vec3::Z * sz);
                facets.push(Facet {
                    normal: Vec3::new(sx, sy, sz).normalize(),
                    vertices: vec![x + (y + z) * a, y + (x + z) * a, z + (x + y) * a],
                    kind: FacetKind::Corner,
                    label: None,
                });
            }
        }
    }
    facets
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Hover {
    Facet(usize),
    /// +1: counter-clockwise (left arrow), -1: clockwise (right arrow).
    Roll(f32),
}

/// What a click on the navigation cube asks for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NavAction {
    /// Look from `eye_dir` (pointing from the target to the eye).
    LookFrom {
        eye_dir: Vec3,
        up: Vec3,
    },
    Roll(f32),
}

#[derive(Default)]
pub struct NavigationCube {
    hover: Option<Hover>,
}

#[derive(Clone)]
pub struct CubeColors {
    pub face: Color32,
    pub chamfer: Color32,
    pub outline: Color32,
    pub text: Color32,
    pub highlight: Color32,
}

impl NavigationCube {
    fn center(view: Rect) -> Pos2 {
        Pos2::new(view.right() - CORNER_OFFSET, view.top() + CORNER_OFFSET)
    }

    /// Whether `pos` is over the cube or its arrows (so the view should not
    /// orbit or select there).
    pub fn contains(view: Rect, pos: Pos2) -> bool {
        Self::center(view).distance(pos) <= ARROW_OUTER_RADIUS + ARROW_HEAD_OVERHANG + 4.0
    }

    pub fn clear_hover(&mut self) {
        self.hover = None;
    }

    /// Updates hover state from the pointer; returns whether it changed.
    pub fn hover(&mut self, view: Rect, rotation: Quat, pointer: Option<Pos2>) -> bool {
        let hover = pointer.and_then(|p| self.hit(view, rotation, p));
        let changed = hover != self.hover;
        self.hover = hover;
        changed
    }

    /// The action for a click at `pos`, given the current camera.
    pub fn click(&self, view: Rect, rotation: Quat, pos: Pos2) -> Option<NavAction> {
        match self.hit(view, rotation, pos)? {
            Hover::Roll(sense) => Some(NavAction::Roll(sense * FRAC_PI_4)),
            Hover::Facet(index) => {
                let eye_dir = facets()[index].normal;
                Some(NavAction::LookFrom {
                    eye_dir,
                    up: up_for(eye_dir, rotation),
                })
            }
        }
    }

    fn hit(&self, view: Rect, rotation: Quat, pos: Pos2) -> Option<Hover> {
        let center = Self::center(view);
        let offset = pos - center;
        let radius = offset.length();
        if (ARROW_INNER_RADIUS - ARROW_HEAD_OVERHANG..=ARROW_OUTER_RADIUS + ARROW_HEAD_OVERHANG)
            .contains(&radius)
        {
            let angle = (-offset.y).atan2(offset.x).to_degrees();
            let (low, high) = (ARROW_HEAD_ANGLE - ARROW_TIP_SWEEP, ARROW_TAIL_ANGLE);
            if (low..=high).contains(&angle) {
                return Some(Hover::Roll(-1.0));
            }
            if (180.0 - high..=180.0 - low).contains(&angle) {
                return Some(Hover::Roll(1.0));
            }
        }

        let inverse = rotation.inverse();
        for (index, facet) in facets().iter().enumerate() {
            if (inverse * facet.normal).z <= 1e-3 {
                continue;
            }
            let polygon: Vec<Pos2> = facet
                .vertices
                .iter()
                .map(|v| project(center, inverse, *v))
                .collect();
            if point_in_convex_polygon(pos, &polygon) {
                return Some(Hover::Facet(index));
            }
        }
        None
    }

    pub fn paint(&self, painter: &Painter, view: Rect, rotation: Quat, colors: &CubeColors) {
        let center = Self::center(view);
        let inverse = rotation.inverse();

        for (index, facet) in facets().iter().enumerate() {
            let facing = (inverse * facet.normal).z;
            if facing <= 1e-3 {
                continue;
            }
            let mut polygon: Vec<Pos2> = facet
                .vertices
                .iter()
                .map(|v| project(center, inverse, *v))
                .collect();
            make_clockwise(&mut polygon);

            let base = if self.hover == Some(Hover::Facet(index)) {
                colors.highlight
            } else if facet.kind == FacetKind::Face {
                colors.face
            } else {
                colors.chamfer
            };
            let fill = shade(base, 0.62 + 0.38 * facing);
            painter.add(Shape::convex_polygon(
                polygon,
                fill,
                Stroke::new(1.0, colors.outline),
            ));

            if let Some((text, reading)) = facet.label
                && facing > 0.3
            {
                let alpha = ((facing - 0.3) / 0.25).clamp(0.0, 1.0);
                let face_center = project(center, inverse, facet.normal);
                let along = project(center, inverse, facet.normal + reading) - face_center;
                paint_rotated_text(
                    painter,
                    face_center,
                    along.y.atan2(along.x),
                    text,
                    // Labels shrink with the face's foreshortening.
                    CUBE_HALF_SIZE * 0.31 * (0.7 + 0.3 * facing),
                    colors.text.gamma_multiply(alpha),
                );
            }
        }

        for sense in [1.0f32, -1.0] {
            let color = if self.hover == Some(Hover::Roll(sense)) {
                colors.highlight
            } else {
                colors.chamfer
            };
            paint_arrow(
                painter,
                &arrow_outline(center, sense),
                color,
                colors.outline,
            );
        }
    }
}

/// Up vector when jumping to `eye_dir`: world Z, except when looking along
/// Z, where the current screen orientation is kept, snapped to an axis.
fn up_for(eye_dir: Vec3, rotation: Quat) -> Vec3 {
    if eye_dir.z.abs() < 0.99 {
        return Vec3::Z;
    }
    let (up, forward) = (rotation * Vec3::Y, rotation * Vec3::NEG_Z);
    let mut candidate = Vec3::new(up.x, up.y, 0.0);
    if candidate.length_squared() < 0.05 {
        // Coming from a side view: tilting towards the top keeps the former
        // forward direction at the top of the screen, and the opposite for
        // the bottom.
        let flat_forward = Vec3::new(forward.x, forward.y, 0.0);
        candidate = if eye_dir.z > 0.0 {
            flat_forward
        } else {
            -flat_forward
        };
    }
    let standard = if eye_dir.z > 0.0 {
        Vec3::Y
    } else {
        Vec3::NEG_Y
    };
    if candidate.length_squared() < 1e-6 {
        return standard;
    }
    // From a corner view both axes are equally close; the standard
    // orientation is the least surprising choice.
    if (candidate.x.abs() - candidate.y.abs()).abs() < 1e-3 * candidate.length() {
        return standard;
    }
    if candidate.x.abs() > candidate.y.abs() {
        Vec3::X * candidate.x.signum()
    } else {
        Vec3::Y * candidate.y.signum()
    }
}

fn project(center: Pos2, inverse_rotation: Quat, point: Vec3) -> Pos2 {
    let v = inverse_rotation * point;
    Pos2::new(
        center.x + v.x * CUBE_HALF_SIZE,
        center.y - v.y * CUBE_HALF_SIZE,
    )
}

fn shade(color: Color32, factor: f32) -> Color32 {
    let f = |c: u8| (f32::from(c) * factor).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_unmultiplied(f(color.r()), f(color.g()), f(color.b()), color.a())
}

/// egui anti-aliases convex polygons correctly only in clockwise order.
fn make_clockwise(points: &mut [Pos2]) {
    let area: f32 = points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .map(|(a, b)| a.x * b.y - b.x * a.y)
        .sum();
    // Screen y points down, so a positive shoelace sum is clockwise.
    if area < 0.0 {
        points.reverse();
    }
}

fn point_in_convex_polygon(p: Pos2, polygon: &[Pos2]) -> bool {
    let mut sign = 0.0f32;
    for (a, b) in polygon.iter().zip(polygon.iter().cycle().skip(1)) {
        let cross = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
        if cross.abs() < 1e-6 {
            continue;
        }
        if sign == 0.0 {
            sign = cross.signum();
        } else if cross.signum() != sign {
            return false;
        }
    }
    sign != 0.0
}

/// Outline of a curved arrow; `sense` +1 is the left one.
fn arrow_outline(center: Pos2, sense: f32) -> Vec<Pos2> {
    let polar = |radius: f32, angle: f32| {
        let angle = if sense > 0.0 { 180.0 - angle } else { angle }.to_radians();
        Pos2::new(
            center.x + radius * angle.cos(),
            center.y - radius * angle.sin(),
        )
    };
    let mid = (ARROW_INNER_RADIUS + ARROW_OUTER_RADIUS) / 2.0;
    let mut points = Vec::new();
    for i in 0..=ARC_SEGMENTS {
        let t = i as f32 / ARC_SEGMENTS as f32;
        points.push(polar(
            ARROW_OUTER_RADIUS,
            ARROW_TAIL_ANGLE + (ARROW_HEAD_ANGLE - ARROW_TAIL_ANGLE) * t,
        ));
    }
    points.push(polar(
        ARROW_OUTER_RADIUS + ARROW_HEAD_OVERHANG,
        ARROW_HEAD_ANGLE,
    ));
    points.push(polar(mid, ARROW_HEAD_ANGLE - ARROW_TIP_SWEEP));
    points.push(polar(
        ARROW_INNER_RADIUS - ARROW_HEAD_OVERHANG,
        ARROW_HEAD_ANGLE,
    ));
    for i in 0..=ARC_SEGMENTS {
        let t = i as f32 / ARC_SEGMENTS as f32;
        points.push(polar(
            ARROW_INNER_RADIUS,
            ARROW_HEAD_ANGLE + (ARROW_TAIL_ANGLE - ARROW_HEAD_ANGLE) * t,
        ));
    }
    points
}

/// Fills the (concave) arrow from convex pieces: one quad per arc segment
/// between the outer and inner arcs, plus the head triangle.
fn paint_arrow(painter: &Painter, outline: &[Pos2], fill: Color32, stroke: Color32) {
    // Layout of `arrow_outline`: outer arc, three head points, inner arc
    // running backwards.
    let n = ARC_SEGMENTS + 1;
    let outer = &outline[..n];
    let head = &outline[n..n + 3];
    let inner: Vec<Pos2> = outline[n + 3..].iter().rev().copied().collect();
    for i in 0..ARC_SEGMENTS {
        let mut quad = vec![outer[i], outer[i + 1], inner[i + 1], inner[i]];
        make_clockwise(&mut quad);
        painter.add(Shape::convex_polygon(quad, fill, Stroke::NONE));
    }
    let mut tip = head.to_vec();
    make_clockwise(&mut tip);
    painter.add(Shape::convex_polygon(tip, fill, Stroke::NONE));
    painter.add(Shape::closed_line(
        outline.to_vec(),
        Stroke::new(1.0, stroke),
    ));
}

fn paint_rotated_text(
    painter: &Painter,
    center: Pos2,
    angle: f32,
    text: &str,
    size: f32,
    color: Color32,
) {
    let galley = painter.layout_no_wrap(text.to_owned(), egui::FontId::proportional(size), color);
    let half = galley.size() / 2.0;
    let (sin, cos) = angle.sin_cos();
    // The text pivots around its top-left corner.
    let offset = Vec2::new(half.x * cos - half.y * sin, half.x * sin + half.y * cos);
    painter.add(TextShape::new(center - offset, galley, color).with_angle(angle));
}

/// Paints the X/Y/Z indicator in the bottom-right corner.
pub fn paint_axes_indicator(
    painter: &Painter,
    view: Rect,
    rotation: Quat,
    colors: [Color32; 3],
    text: Color32,
) {
    let origin = Pos2::new(view.right() - AXES_MARGIN, view.bottom() - AXES_MARGIN);
    let inverse = rotation.inverse();
    let mut axes: Vec<(f32, Vec3, &str, Color32)> =
        [("X", Vec3::X), ("Y", Vec3::Y), ("Z", Vec3::Z)]
            .into_iter()
            .zip(colors)
            .map(|((name, axis), color)| {
                let v = inverse * axis;
                (v.z, v, name, color)
            })
            .collect();
    // Farthest first, so nearer axes are drawn over them.
    axes.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, v, name, color) in axes {
        let tip = origin + Vec2::new(v.x, -v.y) * AXES_LENGTH;
        painter.line_segment([origin, tip], Stroke::new(2.0, color));
        painter.circle_filled(tip, 2.5, color);
        let label_pos = origin + Vec2::new(v.x, -v.y) * (AXES_LENGTH + 10.0);
        painter.text(
            label_pos,
            egui::Align2::CENTER_CENTER,
            name,
            egui::FontId::proportional(11.0),
            text,
        );
    }
}

#[cfg(test)]
#[path = "../../tests/rust/viewer/test_overlays.rs"]
mod tests;
