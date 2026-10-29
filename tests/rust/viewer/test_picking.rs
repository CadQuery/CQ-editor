use super::*;
use crate::viewer::camera::StandardView;

fn square(z: f32, size: f32) -> Mesh {
    let mut mesh = Mesh::new(
        vec![
            [-size, -size, z],
            [size, -size, z],
            [size, size, z],
            [-size, size, z],
        ],
        vec![[0.0, 0.0, 1.0]; 4],
        vec![0, 1, 2, 0, 2, 3],
        vec![
            [-size, -size, z],
            [size, -size, z],
            [size, -size, z],
            [size, size, z],
            [size, size, z],
            [-size, size, z],
            [-size, size, z],
            [-size, -size, z],
        ],
        Vec::new(),
    );
    mesh.topology.face_positions = mesh.positions.clone();
    mesh.topology.face_indices = mesh.indices.clone();
    mesh.topology.triangle_faces = vec![1, 1];
    mesh.topology.segment_edges = vec![1, 2, 3, 4];
    mesh.topology.vertices = mesh.positions.clone();
    mesh
}

fn top_camera() -> Camera {
    Camera {
        rotation: StandardView::Top.rotation(),
        extent: 10.0,
        ..Camera::default()
    }
}

#[test]
fn picks_the_nearest_face() {
    let (low, high) = (square(0.0, 5.0), square(3.0, 1.0));
    let candidates = || {
        [
            Candidate {
                id: "low",
                mesh: &low,
                transform: Mat4::IDENTITY,
            },
            Candidate {
                id: "high",
                mesh: &high,
                transform: Mat4::IDENTITY,
            },
        ]
    };
    let camera = top_camera();
    let viewport = Vec2::new(800.0, 800.0);
    assert_eq!(
        pick(
            &camera,
            1.0,
            Vec2::ZERO,
            viewport,
            SelectionMode::Auto,
            candidates(),
        )
        .map(|hit| (hit.id, hit.entity)),
        Some(("high", Some(TopologyEntity::Face(1))))
    );
    assert_eq!(
        pick(
            &camera,
            1.0,
            Vec2::new(0.3, 0.3),
            viewport,
            SelectionMode::Auto,
            candidates(),
        )
        .map(|hit| (hit.id, hit.entity)),
        Some(("low", Some(TopologyEntity::Face(1))))
    );
    assert_eq!(
        pick(
            &camera,
            1.0,
            Vec2::new(0.9, 0.9),
            viewport,
            SelectionMode::Auto,
            candidates(),
        ),
        None
    );
}

#[test]
fn picks_wires_within_tolerance() {
    let wire = Mesh::lines(vec![[-5.0, 0.0, 0.0], [5.0, 0.0, 0.0]]);
    let mut wire = wire;
    wire.topology.segment_edges = vec![7];
    let camera = top_camera();
    let viewport = Vec2::new(1000.0, 1000.0);
    // 1 world unit = 50 px here; 0.1 units off the line is 5 px.
    let near = Vec2::new(0.0, 0.1 / camera.extent);
    let far = Vec2::new(0.0, 0.5 / camera.extent);
    let candidates = |m| {
        [Candidate {
            id: 1,
            mesh: m,
            transform: Mat4::IDENTITY,
        }]
    };
    assert_eq!(
        pick(
            &camera,
            1.0,
            near,
            viewport,
            SelectionMode::Auto,
            candidates(&wire),
        )
        .map(|hit| (hit.id, hit.entity)),
        Some((1, Some(TopologyEntity::Edge(7))))
    );
    assert_eq!(
        pick(
            &camera,
            1.0,
            far,
            viewport,
            SelectionMode::Auto,
            candidates(&wire),
        ),
        None
    );
}

#[test]
fn prioritizes_vertices_then_edges_then_faces() {
    let mesh = square(0.0, 5.0);
    let camera = top_camera();
    let viewport = Vec2::splat(1000.0);
    let pick_entity = |pointer| {
        pick(
            &camera,
            1.0,
            pointer,
            viewport,
            SelectionMode::Auto,
            [Candidate {
                id: (),
                mesh: &mesh,
                transform: Mat4::IDENTITY,
            }],
        )
        .and_then(|hit| hit.entity)
    };

    assert_eq!(pick_entity(Vec2::ZERO), Some(TopologyEntity::Face(1)));
    assert_eq!(
        pick_entity(Vec2::new(0.0, -0.5)),
        Some(TopologyEntity::Edge(1))
    );
    assert_eq!(
        pick_entity(Vec2::new(0.5, 0.5)),
        Some(TopologyEntity::Vertex(3))
    );
}

#[test]
fn a_front_face_occludes_a_back_edge() {
    let face = square(3.0, 2.0);
    let mut back_edge = Mesh::lines(vec![[-1.0, 0.0, 0.0], [1.0, 0.0, 0.0]]);
    back_edge.topology.segment_edges = vec![9];
    let camera = top_camera();
    let hit = pick(
        &camera,
        1.0,
        Vec2::ZERO,
        Vec2::splat(800.0),
        SelectionMode::Auto,
        [
            Candidate {
                id: "face",
                mesh: &face,
                transform: Mat4::IDENTITY,
            },
            Candidate {
                id: "edge",
                mesh: &back_edge,
                transform: Mat4::IDENTITY,
            },
        ],
    )
    .unwrap();
    assert_eq!(hit.id, "face");
    assert_eq!(hit.entity, Some(TopologyEntity::Face(1)));
}

#[test]
fn picks_a_transformed_mesh_in_world_space() {
    let mesh = square(0.0, 1.0);
    let hit = pick(
        &top_camera(),
        1.0,
        Vec2::ZERO,
        Vec2::splat(800.0),
        SelectionMode::Auto,
        [Candidate {
            id: "moved",
            mesh: &mesh,
            transform: Mat4::from_translation(Vec3::new(0.0, 0.0, 4.0)),
        }],
    )
    .unwrap();
    assert_eq!(hit.id, "moved");
    assert!((hit.position.z - 4.0).abs() < 1e-5);
}

#[test]
fn explicit_selection_modes_only_return_the_requested_topology() {
    let mesh = square(0.0, 5.0);
    let camera = top_camera();
    let viewport = Vec2::splat(1000.0);
    let pick_entity = |pointer, mode| {
        pick(
            &camera,
            1.0,
            pointer,
            viewport,
            mode,
            [Candidate {
                id: (),
                mesh: &mesh,
                transform: Mat4::IDENTITY,
            }],
        )
        .map(|hit| hit.entity)
    };

    let corner = Vec2::new(0.5, 0.5);
    assert_eq!(
        pick_entity(corner, SelectionMode::Vertex),
        Some(Some(TopologyEntity::Vertex(3)))
    );
    assert!(matches!(
        pick_entity(corner, SelectionMode::Edge),
        Some(Some(TopologyEntity::Edge(_)))
    ));
    assert_eq!(
        pick_entity(Vec2::ZERO, SelectionMode::Face),
        Some(Some(TopologyEntity::Face(1)))
    );
    assert_eq!(
        pick_entity(corner, SelectionMode::Object),
        Some(None),
        "object mode must not leak a subshape id"
    );
}

#[test]
fn display_triangles_carry_face_ids_without_separate_pick_triangles() {
    // The worker's normal output: face ids for the display triangles only.
    let mut mesh = square(0.0, 5.0);
    mesh.topology.face_positions.clear();
    mesh.topology.face_indices.clear();
    mesh.topology.triangle_faces = vec![3, 4];
    let pick_at = |pointer| {
        pick(
            &top_camera(),
            1.0,
            pointer,
            Vec2::splat(1000.0),
            SelectionMode::Face,
            [Candidate {
                id: (),
                mesh: &mesh,
                transform: Mat4::IDENTITY,
            }],
        )
        .and_then(|hit| hit.entity)
    };
    // Triangle 0 is (0, 1, 2) below the diagonal, triangle 1 above it.
    assert_eq!(pick_at(Vec2::new(0.2, -0.2)), Some(TopologyEntity::Face(3)));
    assert_eq!(pick_at(Vec2::new(-0.2, 0.2)), Some(TopologyEntity::Face(4)));
}

#[test]
fn triangle_bvh_prunes_a_large_mesh() {
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    for y in 0..64 {
        for x in 0..64 {
            let start = positions.len() as u32;
            positions.extend([
                [x as f32, y as f32, 0.0],
                [x as f32 + 0.8, y as f32, 0.0],
                [x as f32, y as f32 + 0.8, 0.0],
            ]);
            indices.extend([start, start + 1, start + 2]);
        }
    }
    let mesh = Mesh::new(
        positions.clone(),
        vec![[0.0, 0.0, 1.0]; positions.len()],
        indices,
        Vec::new(),
        Vec::new(),
    );
    let (hit, tested) = mesh.nearest_triangle_with_stats(Vec3::new(32.2, 32.2, 10.0), Vec3::NEG_Z);

    assert!(hit.is_some());
    assert!(tested <= 16, "BVH tested {tested} of 4096 triangles");
}

// -- broad phase and primitive indexes ---------------------------------------

use std::f32::consts::TAU;
use std::hint::black_box;
use std::sync::Arc;
use std::time::{Duration, Instant};

use glam::{EulerRot, Quat};

use crate::scene::{Overrides, Part, SceneNode};
use crate::settings::Projection;

const MODES: [SelectionMode; 5] = [
    SelectionMode::Auto,
    SelectionMode::Object,
    SelectionMode::Face,
    SelectionMode::Edge,
    SelectionMode::Vertex,
];

/// Small deterministic generator, so the tests need no extra dependency.
struct Lcg(u64);

impl Lcg {
    fn range(&mut self, min: f32, max: f32) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let unit = (self.0 >> 40) as f32 / (1u64 << 24) as f32;
        min + (max - min) * unit
    }

    fn point(&mut self, min: f32, max: f32) -> Vec3 {
        Vec3::new(
            self.range(min, max),
            self.range(min, max),
            self.range(min, max),
        )
    }
}

fn part(mesh: &Arc<Mesh>, transform: Mat4) -> Part {
    Part {
        mesh: mesh.clone(),
        transform,
        color: None,
        alpha: None,
        face_attributes: Vec::new(),
        material: None,
        render_mesh: None,
    }
}

fn node(key: &str, parts: Vec<Part>, children: Vec<SceneNode>) -> SceneNode {
    SceneNode {
        key: key.into(),
        name: key.into(),
        kind: "Workplane".into(),
        inspectable: false,
        parts,
        children,
        visible: true,
        color: None,
        alpha: None,
        edge_color: None,
        finish: None,
        overrides: Overrides::default(),
        line: None,
        script: None,
    }
}

fn scene_of(nodes: Vec<SceneNode>) -> Scene {
    let mut scene = Scene::default();
    scene.append(nodes, None);
    scene
}

fn translated(key: &str, mesh: &Arc<Mesh>, offset: Vec3) -> SceneNode {
    node(
        key,
        vec![part(mesh, Mat4::from_translation(offset))],
        Vec::new(),
    )
}

/// Every visible part in scene order, as the viewer passed them before the
/// broad phase existed.
fn all_candidates(scene: &Scene) -> Vec<Candidate<'_, (NodeKey, usize)>> {
    scene
        .visible_nodes()
        .into_iter()
        .flat_map(|node| {
            node.parts
                .iter()
                .enumerate()
                .map(move |(part_index, part)| Candidate {
                    id: (node.key.clone(), part_index),
                    mesh: &part.mesh,
                    transform: part.transform,
                })
        })
        .collect()
}

fn indexed_candidates<'a>(
    index: &SceneIndex,
    scene: &'a Scene,
    camera: &Camera,
    aspect: f32,
    pointer: Vec2,
    viewport: Vec2,
    mode: SelectionMode,
) -> Vec<Candidate<'a, (NodeKey, usize)>> {
    index
        .query(camera, aspect, pointer, viewport, mode)
        .into_iter()
        .filter_map(|found| index.candidate(scene, found))
        .collect()
}

fn pointer_grid(steps: usize) -> impl Iterator<Item = Vec2> {
    let at = move |step: usize| -0.95 + 1.9 * step as f32 / (steps - 1) as f32;
    (0..steps * steps).map(move |i| Vec2::new(at(i % steps), at(i / steps)))
}

/// Orthographic and perspective views from several sides, plus a
/// perspective close-up in which some parts lie behind the eye.
fn test_cameras() -> Vec<Camera> {
    let mut cameras = Vec::new();
    for view in [StandardView::Top, StandardView::Iso, StandardView::Front] {
        for projection in [Projection::Orthographic, Projection::Perspective] {
            cameras.push(Camera {
                rotation: view.rotation(),
                extent: 12.0,
                projection,
                ..Camera::default()
            });
        }
    }
    cameras.push(Camera {
        rotation: StandardView::Iso.rotation(),
        extent: 2.0,
        projection: Projection::Perspective,
        ..Camera::default()
    });
    cameras
}

/// Plates and wire crosses at random placements, nested one level deep,
/// with a hidden node and a duplicate coinciding with the first plate.
fn mixed_assembly() -> Scene {
    let plate = Arc::new(square(0.0, 0.6));
    let mut cross = Mesh::lines(vec![
        [-0.8, 0.0, 0.0],
        [0.8, 0.0, 0.0],
        [0.0, -0.8, 0.0],
        [0.0, 0.8, 0.0],
    ]);
    cross.topology.segment_edges = vec![1, 2];
    let cross = Arc::new(cross);

    let mut random = Lcg(7);
    let mut parts = Vec::new();
    for index in 0..120 {
        let mesh = if index % 9 == 4 { &cross } else { &plate };
        let transform = Mat4::from_scale_rotation_translation(
            Vec3::splat(random.range(0.5, 2.0)),
            Quat::from_euler(
                EulerRot::XYZ,
                random.range(-1.0, 1.0),
                random.range(-1.0, 1.0),
                random.range(-3.0, 3.0),
            ),
            Vec3::new(
                random.range(-8.0, 8.0),
                random.range(-8.0, 8.0),
                random.range(-4.0, 4.0),
            ),
        );
        parts.push(part(mesh, transform));
    }
    let duplicate = parts[0].clone();
    let mut nodes: Vec<_> = parts
        .into_iter()
        .enumerate()
        .map(|(index, part)| node(&format!("p{index}"), vec![part], Vec::new()))
        .collect();
    let children = nodes.split_off(60);
    nodes.push(node("assembly", Vec::new(), children));
    nodes.push(node("duplicate", vec![duplicate], Vec::new()));
    let mut scene = scene_of(nodes);
    scene.set_visible("p5", false);
    scene
}

#[test]
fn bounds_bvh_query_matches_a_linear_scan() {
    let mut random = Lcg(3);
    let boxes: Vec<(Vec3, Vec3)> = (0..500)
        .map(|_| {
            let min = random.point(-50.0, 50.0);
            (min, min + random.point(0.0, 4.0))
        })
        .collect();
    let bvh = BoundsBvh::build(boxes.iter().copied());

    let mut tested = 0;
    for _ in 0..200 {
        let query_min = random.point(-55.0, 50.0);
        let query_max = query_min + random.point(0.0, 10.0);
        let overlaps =
            |min: Vec3, max: Vec3| min.cmple(query_max).all() && max.cmpge(query_min).all();
        let mut found = Vec::new();
        tested += bvh.query_with_stats(overlaps, &mut found).tested_bounds;
        let expected: Vec<usize> = boxes
            .iter()
            .enumerate()
            .filter(|(_, (min, max))| overlaps(*min, *max))
            .map(|(index, _)| index)
            .collect();
        assert_eq!(found, expected, "BVH results must be complete and ordered");
    }
    assert!(tested < 200 * 500 / 10, "BVH tested {tested} boxes");

    let mut found = vec![42];
    BoundsBvh::build([]).query(|_, _| true, &mut found);
    assert_eq!(found, [42], "an empty BVH adds nothing");
}

#[test]
fn edge_and_vertex_bvhs_keep_every_primitive_within_tolerance() {
    let mut random = Lcg(11);
    let mut segments = Vec::new();
    for _ in 0..600 {
        let start = random.point(-8.0, 8.0);
        let end = start + random.point(-1.5, 1.5);
        segments.extend([start.to_array(), end.to_array()]);
    }
    let points: Vec<[f32; 3]> = (0..400)
        .map(|_| random.point(-8.0, 8.0).to_array())
        .collect();
    let mesh = Mesh::new(Vec::new(), Vec::new(), Vec::new(), segments, points);
    let transform = Mat4::from_scale_rotation_translation(
        Vec3::new(1.2, 0.8, 1.0),
        Quat::from_euler(EulerRot::XYZ, 0.4, -0.3, 0.9),
        Vec3::new(1.0, -2.0, 0.5),
    );
    let (aspect, viewport) = (1.5, Vec2::new(900.0, 600.0));

    let (mut kept, mut total) = (0, 0);
    for camera in test_cameras() {
        let view_projection = camera.view_projection(aspect);
        let screen = |point: &[f32; 3]| {
            project_with(
                view_projection,
                transform.transform_point3(Vec3::from_array(*point)),
            )
            .map(|ndc| ndc_to_px(ndc.truncate(), viewport))
        };
        for pointer in pointer_grid(15) {
            let pointer_px = ndc_to_px(pointer, viewport);
            let near_pointer = |min, max| {
                projected_bounds_hit(view_projection, viewport, pointer_px, transform, min, max)
            };
            let (mut edges, mut vertices) = (Vec::new(), Vec::new());
            mesh.edge_bvh().query(near_pointer, &mut edges);
            mesh.vertex_bvh().query(near_pointer, &mut vertices);

            for (index, segment) in mesh.edges.as_chunks::<2>().0.iter().enumerate() {
                if let (Some(a), Some(b)) = (screen(&segment[0]), screen(&segment[1]))
                    && distance_to_segment(pointer_px, a, b).0 <= PICK_TOLERANCE_PX
                {
                    assert!(edges.contains(&index), "edge {index} pruned at {pointer}");
                }
            }
            for (index, point) in mesh.pick_vertices().iter().enumerate() {
                if screen(point).is_some_and(|p| p.distance(pointer_px) <= PICK_TOLERANCE_PX) {
                    assert!(
                        vertices.contains(&index),
                        "vertex {index} pruned at {pointer}"
                    );
                }
            }
            kept += edges.len() + vertices.len();
            total += mesh.edges.len() / 2 + mesh.pick_vertices().len();
        }
    }
    assert!(kept * 5 < total, "kept {kept} of {total} primitives");
}

#[test]
fn indexed_picking_matches_a_linear_scan() {
    let scene = mixed_assembly();
    let mut index = SceneIndex::default();
    index.update(&scene);
    let (aspect, viewport) = (1.5, Vec2::new(900.0, 600.0));

    let mut hits = 0;
    for camera in test_cameras() {
        for mode in MODES {
            for pointer in pointer_grid(11) {
                let linear = pick(
                    &camera,
                    aspect,
                    pointer,
                    viewport,
                    mode,
                    all_candidates(&scene),
                );
                let candidates =
                    indexed_candidates(&index, &scene, &camera, aspect, pointer, viewport, mode);
                let indexed = pick(&camera, aspect, pointer, viewport, mode, candidates);
                assert_eq!(indexed, linear, "{mode:?} at {pointer} with {camera:?}");
                hits += usize::from(linear.is_some());
            }
        }
    }
    assert!(hits > 400, "the sweep must hit geometry, got {hits} hits");
}

#[test]
fn coincident_parts_resolve_to_scene_order() {
    let scene = mixed_assembly();
    let mut index = SceneIndex::default();
    index.update(&scene);
    let camera = top_camera();
    let first = scene.nodes[0].parts[0]
        .transform
        .transform_point3(Vec3::ZERO);
    let pointer = project_with(camera.view_projection(1.0), first)
        .unwrap()
        .truncate();
    let viewport = Vec2::splat(800.0);

    let candidates = indexed_candidates(
        &index,
        &scene,
        &camera,
        1.0,
        pointer,
        viewport,
        SelectionMode::Face,
    );
    let hit = pick(
        &camera,
        1.0,
        pointer,
        viewport,
        SelectionMode::Face,
        candidates,
    );
    assert_eq!(hit.map(|hit| hit.id.0), Some("p0".to_string()));
}

#[test]
fn scene_index_returns_candidates_in_scene_order() {
    // Later parts sit lower, so a spatial sort alone would reverse them.
    let plate = Arc::new(square(0.0, 1.0));
    let scene = scene_of(
        (0..32)
            .map(|i| translated(&format!("p{i}"), &plate, Vec3::Z * (i as f32 * -0.25)))
            .collect(),
    );
    let mut index = SceneIndex::default();
    index.update(&scene);
    let found = index.query(
        &top_camera(),
        1.0,
        Vec2::ZERO,
        Vec2::splat(800.0),
        SelectionMode::Face,
    );
    assert_eq!(found, (0..32).collect::<Vec<_>>());
}

#[test]
fn scene_index_rebuilds_only_when_pickable_geometry_changes() {
    let plate = Arc::new(square(0.0, 1.0));
    let camera = top_camera();
    let viewport = Vec2::splat(1000.0);
    // 1 world unit is 50 px with the top camera.
    let at = |x: f32| Vec2::new(x / camera.extent, 0.0);
    let keys = |index: &SceneIndex, scene: &Scene, x: f32| -> Vec<NodeKey> {
        index
            .query(&camera, 1.0, at(x), viewport, SelectionMode::Auto)
            .into_iter()
            .filter_map(|found| index.candidate(scene, found))
            .map(|candidate| candidate.id.0)
            .collect()
    };

    let mut scene = scene_of(vec![
        translated("a", &plate, Vec3::X * -3.0),
        translated("b", &plate, Vec3::X * 3.0),
    ]);
    let mut index = SceneIndex::default();
    assert!(index.update(&scene));
    assert!(!index.update(&scene), "an unchanged scene keeps its index");
    scene.select_only("a");
    scene.set_visible("a", true);
    assert!(
        !index.update(&scene),
        "selection and no-op visibility changes keep the index"
    );
    assert_eq!(index.rebuild_count(), 1);
    assert_eq!(keys(&index, &scene, -3.0), ["a"]);

    scene.set_visible("a", false);
    assert!(index.update(&scene));
    assert!(
        keys(&index, &scene, -3.0).is_empty(),
        "hidden parts are skipped"
    );

    scene.append(vec![translated("c", &plate, Vec3::ZERO)], None);
    assert!(index.update(&scene));
    assert_eq!(keys(&index, &scene, 0.0), ["c"]);

    scene.remove_top_level(&["c".to_string()]);
    assert!(index.update(&scene));
    assert!(keys(&index, &scene, 0.0).is_empty());

    scene.clear();
    assert!(index.update(&scene));
    assert!(keys(&index, &scene, 3.0).is_empty());
    assert_eq!(index.rebuild_count(), 5);

    // Two scenes built the same way never share a revision.
    let left = scene_of(vec![translated("left", &plate, Vec3::X * -3.0)]);
    let right = scene_of(vec![translated("right", &plate, Vec3::X * 3.0)]);
    assert!(index.update(&left));
    assert!(index.update(&right), "another scene must rebuild the index");
    assert_eq!(keys(&index, &right, 3.0), ["right"]);
}

#[test]
fn scene_index_never_returns_a_part_under_another_key() {
    let plate = Arc::new(square(0.0, 1.0));
    let mut scene = scene_of(vec![
        translated("a", &plate, Vec3::X * -3.0),
        translated("b", &plate, Vec3::X * 3.0),
    ]);
    let mut index = SceneIndex::default();
    index.update(&scene);
    // Bypasses the pick revision on purpose, as a future caller might.
    scene.nodes.swap(0, 1);
    assert!(index.candidate(&scene, 0).is_none());
    assert!(index.candidate(&scene, 1).is_none());
}

/// `side` x `side` instances of one shared mesh, spaced 3 units apart.
fn instanced_grid(mesh: &Arc<Mesh>, side: usize) -> Scene {
    scene_of(
        (0..side * side)
            .map(|i| {
                let offset = Vec3::new((i % side) as f32 * 3.0, (i / side) as f32 * 3.0, 0.0);
                let transform =
                    Mat4::from_rotation_translation(Quat::from_rotation_z(i as f32 * 0.37), offset);
                node(&format!("p{i}"), vec![part(mesh, transform)], Vec::new())
            })
            .collect(),
    )
}

/// An isometric view fitting the whole grid.
fn grid_camera(side: usize) -> Camera {
    let center = Vec3::new(1.5, 1.5, 0.0) * (side as f32 - 1.0);
    Camera {
        target: center,
        rotation: StandardView::Iso.rotation(),
        extent: side as f32 * 2.2,
        scene_center: center,
        scene_radius: side as f32 * 2.2,
        ..Camera::default()
    }
}

#[test]
fn scene_index_work_grows_slowly_with_the_part_count() {
    let plate = Arc::new(square(0.0, 1.0));
    let mut work = Vec::new();
    for side in [16, 64] {
        let scene = instanced_grid(&plate, side);
        let mut index = SceneIndex::default();
        index.update(&scene);
        let camera = grid_camera(side);
        let mut total = 0;
        for pointer in pointer_grid(9) {
            let (_, stats) = index.query_with_stats(
                &camera,
                1.0,
                pointer,
                Vec2::splat(1000.0),
                SelectionMode::Auto,
            );
            total += stats.visited_nodes + stats.tested_bounds;
        }
        work.push(total / 81);
    }
    let (small, large) = (work[0], work[1]);
    // 16x more parts (256 -> 4096) must cost far less than 16x the work.
    assert!(large < small * 3, "{small} -> {large} visited per query");
    assert!(large < 4096 / 20, "{large} visited per query of 4096 parts");
}

/// A meshed cylinder shaped like worker output: side and caps are faces
/// 1-3, both circles and the seam are edges 1-3, plus two vertices.
fn cylinder(segments: u32) -> Mesh {
    let ring_point = |i: u32, z: f32| {
        let angle = i as f32 / segments as f32 * TAU;
        [angle.cos(), angle.sin(), z]
    };
    let positions: Vec<[f32; 3]> = (0..segments)
        .map(|i| ring_point(i, 0.0))
        .chain((0..segments).map(|i| ring_point(i, 2.0)))
        .collect();
    let n = segments;
    let (mut indices, mut triangle_faces) = (Vec::new(), Vec::new());
    for i in 0..n {
        let j = (i + 1) % n;
        indices.extend([i, j, n + j, i, n + j, n + i]);
        triangle_faces.extend([1, 1]);
    }
    for i in 1..n - 1 {
        indices.extend([0, i + 1, i, n, n + i, n + i + 1]);
        triangle_faces.extend([2, 3]);
    }
    let (mut edges, mut segment_edges) = (Vec::new(), Vec::new());
    for (edge, z) in [(1, 0.0), (2, 2.0)] {
        for i in 0..n {
            edges.extend([ring_point(i, z), ring_point(i + 1, z)]);
            segment_edges.push(edge);
        }
    }
    edges.extend([[1.0, 0.0, 0.0], [1.0, 0.0, 2.0]]);
    segment_edges.push(3);

    let normals = vec![[0.0, 0.0, 1.0]; positions.len()];
    let mut mesh = Mesh::new(positions, normals, indices, edges, Vec::new());
    mesh.topology.face_positions = mesh.positions.clone();
    mesh.topology.face_indices = mesh.indices.clone();
    mesh.topology.triangle_faces = triangle_faces;
    mesh.topology.segment_edges = segment_edges;
    mesh.topology.vertices = vec![[1.0, 0.0, 0.0], [1.0, 0.0, 2.0]];
    mesh
}

/// A pointer sweeping across the view, as during a mouse move.
fn sweep() -> Vec<Vec2> {
    (0..240)
        .map(|i| {
            let t = i as f32 / 239.0;
            Vec2::new(-0.6 + 1.2 * t, 0.3 * (t * 15.0).sin())
        })
        .collect()
}

fn latency_summary(mut samples: Vec<Duration>) -> String {
    samples.sort_unstable();
    let micros = |d: Duration| d.as_secs_f64() * 1e6;
    format!(
        "{:>10.1} {:>10.1} {:>10.1}",
        micros(samples[samples.len() / 2]),
        micros(samples[samples.len() * 95 / 100]),
        micros(samples[samples.len() - 1]),
    )
}

/// Hover latency on growing assemblies, linear scan against the indexed
/// broad phase. Run with
/// `cargo test --release pick_latency -- --ignored --nocapture`.
#[test]
#[ignore = "benchmark; run explicitly in release mode"]
fn pick_latency_on_growing_assemblies() {
    println!(
        "environment: cq-editor={} os={} arch={} profile={} logical_cpus={}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        std::thread::available_parallelism().map_or(1, usize::from),
    );
    let mesh = Arc::new(cylinder(48));
    let (aspect, viewport) = (1.6, Vec2::new(1600.0, 1000.0));
    let path = sweep();
    println!(
        "{:>7} | {:>32} | {:>32} | {:>10}",
        "parts", "linear p50 / p95 / max (us)", "indexed p50 / p95 / max (us)", "index (ms)"
    );
    for side in [8, 16, 32, 64, 128] {
        let scene = instanced_grid(&mesh, side);
        let camera = grid_camera(side);
        // Build the shared mesh's lazy BVHs outside the measurements.
        pick(
            &camera,
            aspect,
            Vec2::ZERO,
            viewport,
            SelectionMode::Auto,
            all_candidates(&scene),
        );

        let linear: Vec<_> = path
            .iter()
            .map(|&pointer| {
                let start = Instant::now();
                black_box(pick(
                    &camera,
                    aspect,
                    pointer,
                    viewport,
                    SelectionMode::Auto,
                    all_candidates(&scene),
                ));
                start.elapsed()
            })
            .collect();

        let mut index = SceneIndex::default();
        let start = Instant::now();
        index.update(&scene);
        let build = start.elapsed();
        let indexed: Vec<_> = path
            .iter()
            .map(|&pointer| {
                let start = Instant::now();
                let candidates = indexed_candidates(
                    &index,
                    &scene,
                    &camera,
                    aspect,
                    pointer,
                    viewport,
                    SelectionMode::Auto,
                );
                black_box(pick(
                    &camera,
                    aspect,
                    pointer,
                    viewport,
                    SelectionMode::Auto,
                    candidates,
                ));
                start.elapsed()
            })
            .collect();

        println!(
            "{:>7} | {} | {} | {:>10.2}",
            side * side,
            latency_summary(linear),
            latency_summary(indexed),
            build.as_secs_f64() * 1e3,
        );
    }
}

#[test]
fn a_section_plane_hides_what_it_removes_from_picking() {
    use crate::viewer::section::ClipPlane;
    let (low, high) = (square(0.0, 5.0), square(3.0, 5.0));
    let candidates = || {
        [
            Candidate {
                id: "low",
                mesh: &low,
                transform: Mat4::IDENTITY,
            },
            Candidate {
                id: "high",
                mesh: &high,
                transform: Mat4::IDENTITY,
            },
        ]
    };
    let camera = top_camera();
    let viewport = Vec2::splat(800.0);
    let pick_with = |clip, mode| {
        pick_clipped(
            &camera,
            1.0,
            Vec2::new(0.1, 0.1),
            viewport,
            mode,
            candidates(),
            clip,
        )
        .map(|hit| hit.id)
    };

    assert_eq!(pick_with(None, SelectionMode::Face), Some("high"));
    // Everything above z = 1 is removed: the low square is picked.
    let top_off = Some(ClipPlane {
        normal: Vec3::Z,
        offset: 1.0,
    });
    assert_eq!(pick_with(top_off, SelectionMode::Face), Some("low"));
    // Everything below z = 1 removed instead.
    let bottom_off = Some(ClipPlane {
        normal: Vec3::NEG_Z,
        offset: -1.0,
    });
    assert_eq!(pick_with(bottom_off, SelectionMode::Face), Some("high"));
    // Removing everything leaves nothing to pick, edges included.
    let all_off = Some(ClipPlane {
        normal: Vec3::Z,
        offset: -10.0,
    });
    assert_eq!(pick_with(all_off, SelectionMode::Auto), None);
}

#[test]
fn removed_edges_and_vertices_cannot_be_picked() {
    use crate::viewer::section::ClipPlane;
    let mesh = square(3.0, 5.0);
    let camera = top_camera();
    let viewport = Vec2::splat(1000.0);
    let corner = Vec2::new(0.5, 0.5);
    let pick_at = |clip| {
        pick_clipped(
            &camera,
            1.0,
            corner,
            viewport,
            SelectionMode::Vertex,
            [Candidate {
                id: (),
                mesh: &mesh,
                transform: Mat4::IDENTITY,
            }],
            clip,
        )
        .and_then(|hit| hit.entity)
    };
    assert_eq!(pick_at(None), Some(TopologyEntity::Vertex(3)));
    let removed = Some(ClipPlane {
        normal: Vec3::Z,
        offset: 1.0,
    });
    assert_eq!(pick_at(removed), None);
}

#[test]
fn exploded_and_isolated_parts_are_picked_where_they_are_shown() {
    let plate = Arc::new(square(0.0, 1.0));
    let mut scene = scene_of(vec![
        translated("left", &plate, Vec3::new(-3.0, 0.0, 0.0)),
        translated("right", &plate, Vec3::new(3.0, 0.0, 0.0)),
    ]);
    let camera = Camera {
        rotation: StandardView::Top.rotation(),
        extent: 20.0,
        ..Camera::default()
    };
    let viewport = Vec2::splat(800.0);
    let mut index = SceneIndex::default();
    let mut pick_at = |scene: &Scene, x: f32| {
        index.update(scene);
        // 20 units from the centre to the edge: x / 20 in NDC.
        let pointer = Vec2::new(x / 20.0, 0.0);
        let candidates = indexed_candidates(
            &index,
            scene,
            &camera,
            1.0,
            pointer,
            viewport,
            SelectionMode::Object,
        );
        pick(
            &camera,
            1.0,
            pointer,
            viewport,
            SelectionMode::Object,
            candidates,
        )
        .map(|hit| hit.id.0)
    };
    assert_eq!(pick_at(&scene, 3.5).as_deref(), Some("right"));
    assert_eq!(pick_at(&scene, 9.5), None);

    // Three times as far from the centre: the right plate is around x = 9.
    scene.set_explode(2.0);
    assert_eq!(pick_at(&scene, 3.5), None);
    assert_eq!(pick_at(&scene, 9.5).as_deref(), Some("right"));

    scene.isolate(&["left".to_owned()]);
    assert_eq!(pick_at(&scene, 9.5), None, "isolated out");
    // The explosion centres on what is shown: alone, the plate is at home.
    assert_eq!(pick_at(&scene, -9.5), None);
    assert_eq!(pick_at(&scene, -3.5).as_deref(), Some("left"));
}
