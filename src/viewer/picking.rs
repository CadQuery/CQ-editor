//! Finding the object under the mouse pointer.
//!
//! Faces are hit-tested exactly with a ray. Topological edges and vertices
//! are matched in screen space within a few pixels, since a ray would
//! practically never touch them. A front face occludes entities behind it.

use glam::{Mat4, Vec2, Vec3};

use super::SelectionMode;
use super::camera::{Camera, project_with};
use super::section::ClipPlane;
use crate::scene::{Aabb, Mesh, NodeKey, Scene, TopologyEntity};
use crate::spatial::{BoundsBvh, QueryStats};

/// How close to an edge or point the pointer must be, in pixels.
const PICK_TOLERANCE_PX: f32 = 6.0;
/// NDC depth allowance for an edge or vertex lying on the hit face.
const OCCLUSION_EPSILON: f32 = 1e-3;

#[derive(Debug)]
struct ScenePart {
    path: Vec<usize>,
    part: usize,
    key: NodeKey,
    world_bounds: Aabb,
}

/// World-space broad phase for all visible scene parts. The viewer retains
/// this index across pointer moves and rebuilds it only when the scene's
/// geometry, placement or visibility revision changes.
#[derive(Debug, Default)]
pub(crate) struct SceneIndex {
    revision: Option<u64>,
    parts: Vec<ScenePart>,
    bvh: BoundsBvh,
    rebuilds: u64,
}

impl SceneIndex {
    pub(crate) fn update(&mut self, scene: &Scene) -> bool {
        let revision = scene.pick_revision();
        if self.revision == Some(revision) {
            return false;
        }

        self.parts.clear();
        for (path, node) in scene.shown_nodes() {
            for (part, geometry) in node.parts.iter().enumerate() {
                if !geometry.mesh.bounds.is_empty() {
                    self.parts.push(ScenePart {
                        path: path.clone(),
                        part,
                        key: node.key.clone(),
                        world_bounds: geometry
                            .mesh
                            .bounds
                            .transformed(scene.part_transform(geometry)),
                    });
                }
            }
        }
        self.bvh = BoundsBvh::build(
            self.parts
                .iter()
                .map(|entry| (entry.world_bounds.min, entry.world_bounds.max)),
        );
        self.revision = Some(revision);
        self.rebuilds += 1;
        true
    }

    pub(crate) fn query(
        &self,
        camera: &Camera,
        aspect: f32,
        pointer_ndc: Vec2,
        viewport_px: Vec2,
        mode: SelectionMode,
    ) -> Vec<usize> {
        self.query_with_stats(camera, aspect, pointer_ndc, viewport_px, mode)
            .0
    }

    fn query_with_stats(
        &self,
        camera: &Camera,
        aspect: f32,
        pointer_ndc: Vec2,
        viewport_px: Vec2,
        mode: SelectionMode,
    ) -> (Vec<usize>, QueryStats) {
        let (origin, direction) = camera.ray(pointer_ndc, aspect);
        let view_projection = camera.view_projection(aspect);
        let pointer_px = ndc_to_px(pointer_ndc, viewport_px);
        let screen_pick = mode.allows_edges() || mode.allows_vertices();
        let mut candidates = Vec::new();
        let stats = self.bvh.query_with_stats(
            |min, max| {
                ray_hits_box(origin, direction, min, max)
                    || (screen_pick
                        && projected_bounds_hit(
                            view_projection,
                            viewport_px,
                            pointer_px,
                            Mat4::IDENTITY,
                            min,
                            max,
                        ))
            },
            &mut candidates,
        );
        (candidates, stats)
    }

    pub(crate) fn candidate<'a>(
        &self,
        scene: &'a Scene,
        index: usize,
    ) -> Option<Candidate<'a, (NodeKey, usize)>> {
        let entry = self.parts.get(index)?;
        // Guards against a tree changed without a new pick revision: a
        // missed pick is better than reporting another node's part.
        let node = node_at_path(&scene.nodes, &entry.path).filter(|node| node.key == entry.key)?;
        let part = node.parts.get(entry.part)?;
        Some(Candidate {
            id: (entry.key.clone(), entry.part),
            mesh: &part.mesh,
            transform: scene.part_transform(part),
        })
    }

    #[cfg(test)]
    fn rebuild_count(&self) -> u64 {
        self.rebuilds
    }
}

fn node_at_path<'a>(
    mut nodes: &'a [crate::scene::SceneNode],
    path: &[usize],
) -> Option<&'a crate::scene::SceneNode> {
    let mut result = None;
    for &index in path {
        let node = nodes.get(index)?;
        result = Some(node);
        nodes = &node.children;
    }
    result
}

/// Something to test against, tagged with a caller-defined id.
pub struct Candidate<'a, T> {
    pub id: T,
    pub mesh: &'a Mesh,
    pub transform: Mat4,
}

/// Exact result of a view pick. `entity` is absent only for legacy meshes
/// without worker-supplied topology metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit<T> {
    pub id: T,
    pub entity: Option<TopologyEntity>,
    pub position: Vec3,
}

struct RankedHit<T> {
    hit: Hit<T>,
    depth: f32,
    screen_distance: f32,
    priority: u8,
    surface: bool,
}

/// Picks the candidate closest to the viewer under `pointer_ndc`
/// (normalized device coordinates; `viewport_px` converts to pixels).
#[cfg(test)]
pub fn pick<'a, T: Clone>(
    camera: &Camera,
    aspect: f32,
    pointer_ndc: Vec2,
    viewport_px: Vec2,
    mode: SelectionMode,
    candidates: impl IntoIterator<Item = Candidate<'a, T>>,
) -> Option<Hit<T>> {
    pick_clipped(
        camera,
        aspect,
        pointer_ndc,
        viewport_px,
        mode,
        candidates,
        None,
    )
}

/// [`pick`] when a section plane removes part of the model: what it
/// removes can neither be picked nor hide what lies behind it.
pub fn pick_clipped<'a, T: Clone>(
    camera: &Camera,
    aspect: f32,
    pointer_ndc: Vec2,
    viewport_px: Vec2,
    mode: SelectionMode,
    candidates: impl IntoIterator<Item = Candidate<'a, T>>,
    clip: Option<ClipPlane>,
) -> Option<Hit<T>> {
    let (origin, dir) = camera.ray(pointer_ndc, aspect);
    // In world ray parameters, which local rays share (affine transforms).
    let kept = match clip {
        Some(plane) => match plane.kept_range(origin, dir) {
            Some((start, end)) => (start.max(0.0), end),
            None => (f32::INFINITY, f32::NEG_INFINITY),
        },
        None => (0.0, f32::INFINITY),
    };
    let removed = |point: Vec3| clip.is_some_and(|plane| plane.removes(point));
    // Computed once: projecting thousands of points per pick dominates the
    // cost when every projection rebuilds the camera matrices.
    let view_projection = camera.view_projection(aspect);
    let pointer_px = ndc_to_px(pointer_ndc, viewport_px);

    let mut hits = Vec::new();
    let mut front_surface: Option<f32> = None;

    for candidate in candidates {
        let mesh = candidate.mesh;
        if mesh.bounds.is_empty() {
            continue;
        }
        let inverse = candidate.transform.inverse();
        let local_origin = inverse.transform_point3(origin);
        let local_dir = inverse.transform_vector3(dir);

        if ray_hits_box(local_origin, local_dir, mesh.bounds.min, mesh.bounds.max) {
            let topology_faces = !mesh.topology.triangle_faces.is_empty();
            if let Some((distance, triangle)) =
                mesh.nearest_triangle_within(local_origin, local_dir, kept)
            {
                let local_position = local_origin + local_dir * distance;
                let position = candidate.transform.transform_point3(local_position);
                if let Some(depth) = projected_depth(view_projection, position) {
                    front_surface = Some(front_surface.map_or(depth, |front| front.min(depth)));
                    let entity = topology_faces
                        .then(|| mesh.topology.triangle_faces.get(triangle).copied())
                        .flatten()
                        .map(TopologyEntity::Face);
                    if mode.allows_faces() && mode.accepts(entity) {
                        hits.push(RankedHit {
                            hit: Hit {
                                id: candidate.id.clone(),
                                entity: mode.entity(entity),
                                position,
                            },
                            depth,
                            screen_distance: 0.0,
                            priority: 2,
                            surface: true,
                        });
                    }
                }
            }
        }

        let screen = |p: &[f32; 3]| {
            let world = candidate.transform.transform_point3(Vec3::from_array(*p));
            project_with(view_projection, world)
                .map(|ndc| (ndc_to_px(ndc.truncate(), viewport_px), world, ndc.z))
        };
        let mut primitive_candidates = Vec::new();
        if mode.allows_edges() {
            mesh.edge_bvh().query(
                |min, max| {
                    projected_bounds_hit(
                        view_projection,
                        viewport_px,
                        pointer_px,
                        candidate.transform,
                        min,
                        max,
                    )
                },
                &mut primitive_candidates,
            );
        }
        for segment_index in primitive_candidates.drain(..) {
            let Some(segment) = mesh.edges.as_chunks::<2>().0.get(segment_index) else {
                continue;
            };
            let (Some((a, wa, _)), Some((b, wb, _))) = (screen(&segment[0]), screen(&segment[1]))
            else {
                continue;
            };
            let (distance, t) = distance_to_segment(pointer_px, a, b);
            if distance <= PICK_TOLERANCE_PX && !removed(wa.lerp(wb, t)) {
                let position = wa.lerp(wb, t);
                if let Some(depth) = projected_depth(view_projection, position) {
                    let entity = mesh
                        .topology
                        .segment_edges
                        .get(segment_index)
                        .copied()
                        .map(TopologyEntity::Edge);
                    if mode.accepts(entity) {
                        hits.push(RankedHit {
                            hit: Hit {
                                id: candidate.id.clone(),
                                entity: mode.entity(entity),
                                position,
                            },
                            depth,
                            screen_distance: distance,
                            priority: 1,
                            surface: false,
                        });
                    }
                }
            }
        }
        let topology_vertices = !mesh.topology.vertices.is_empty();
        let vertices = mesh.pick_vertices();
        if mode.allows_vertices() {
            mesh.vertex_bvh().query(
                |min, max| {
                    projected_bounds_hit(
                        view_projection,
                        viewport_px,
                        pointer_px,
                        candidate.transform,
                        min,
                        max,
                    )
                },
                &mut primitive_candidates,
            );
        }
        for index in primitive_candidates.drain(..) {
            let Some(point) = vertices.get(index) else {
                continue;
            };
            if let Some((screen_pos, position, depth)) = screen(point)
                && let distance = screen_pos.distance(pointer_px)
                && distance <= PICK_TOLERANCE_PX
                && !removed(position)
            {
                let entity = topology_vertices.then_some(TopologyEntity::Vertex(index as u32 + 1));
                if mode.accepts(entity) {
                    hits.push(RankedHit {
                        hit: Hit {
                            id: candidate.id.clone(),
                            entity: mode.entity(entity),
                            position,
                        },
                        depth,
                        screen_distance: distance,
                        priority: 0,
                        surface: false,
                    });
                }
            }
        }
    }

    hits.into_iter()
        .filter(|hit| {
            hit.surface || front_surface.is_none_or(|depth| hit.depth <= depth + OCCLUSION_EPSILON)
        })
        .min_by(|a, b| {
            a.priority
                .cmp(&b.priority)
                .then_with(|| a.depth.total_cmp(&b.depth))
                .then_with(|| a.screen_distance.total_cmp(&b.screen_distance))
        })
        .map(|ranked| ranked.hit)
}

fn ndc_to_px(ndc: Vec2, viewport_px: Vec2) -> Vec2 {
    ndc * viewport_px * 0.5
}

/// Conservatively checks whether a transformed 3D AABB can overlap the
/// pointer's screen-space tolerance. If any corner is behind the eye, keep
/// the node: pruning an eye-crossing box would risk a false negative.
fn projected_bounds_hit(
    view_projection: Mat4,
    viewport_px: Vec2,
    pointer_px: Vec2,
    transform: Mat4,
    min: Vec3,
    max: Vec3,
) -> bool {
    let mut screen_min = Vec2::splat(f32::INFINITY);
    let mut screen_max = Vec2::splat(f32::NEG_INFINITY);
    for corner in 0..8 {
        let local = Vec3::new(
            if corner & 1 == 0 { min.x } else { max.x },
            if corner & 2 == 0 { min.y } else { max.y },
            if corner & 4 == 0 { min.z } else { max.z },
        );
        let world = transform.transform_point3(local);
        let Some(ndc) = project_with(view_projection, world) else {
            return true;
        };
        let screen = ndc_to_px(ndc.truncate(), viewport_px);
        screen_min = screen_min.min(screen);
        screen_max = screen_max.max(screen);
    }
    pointer_px.x >= screen_min.x - PICK_TOLERANCE_PX
        && pointer_px.x <= screen_max.x + PICK_TOLERANCE_PX
        && pointer_px.y >= screen_min.y - PICK_TOLERANCE_PX
        && pointer_px.y <= screen_max.y + PICK_TOLERANCE_PX
}

fn projected_depth(view_projection: Mat4, point: Vec3) -> Option<f32> {
    project_with(view_projection, point)
        .map(|projected| projected.z)
        .filter(|depth| (0.0..=1.0).contains(depth))
}

/// Slab test.
fn ray_hits_box(origin: Vec3, dir: Vec3, min: Vec3, max: Vec3) -> bool {
    let inv = dir.recip();
    let t1 = (min - origin) * inv;
    let t2 = (max - origin) * inv;
    let near = t1.min(t2).max_element();
    let far = t1.max(t2).min_element();
    // NaN (ray parallel to a slab, starting on its border) counts as a hit.
    !(near > far || far < 0.0)
}

/// Distance from `p` to segment `ab`, and the parameter of the closest point.
fn distance_to_segment(p: Vec2, a: Vec2, b: Vec2) -> (f32, f32) {
    let ab = b - a;
    let length2 = ab.length_squared();
    let t = if length2 > 0.0 {
        ((p - a).dot(ab) / length2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p.distance(a + ab * t), t)
}

#[cfg(test)]
#[path = "../../tests/rust/viewer/test_picking.rs"]
mod tests;
