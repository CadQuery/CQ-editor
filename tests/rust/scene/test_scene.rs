use super::*;

fn node(key: &str, name: &str, children: Vec<SceneNode>) -> SceneNode {
    SceneNode {
        key: key.into(),
        name: name.into(),
        kind: "Workplane".into(),
        inspectable: false,
        parts: vec![Part {
            mesh: Arc::new(Mesh::lines(vec![[0.0; 3], [1.0, 2.0, 3.0]])),
            transform: Mat4::IDENTITY,
            color: None,
            alpha: None,
            face_attributes: Vec::new(),
            material: None,
            render_mesh: None,
        }],
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

fn scene() -> Scene {
    let mut scene = Scene::default();
    scene.append(
        vec![
            node(
                "a",
                "assy",
                vec![node("a1", "plate", vec![]), node("a2", "pin", vec![])],
            ),
            node("b", "box", vec![]),
        ],
        None,
    );
    scene
}

#[test]
fn hiding_a_parent_hides_the_subtree() {
    let mut scene = scene();
    scene.set_visible("a", false);
    let visible: Vec<_> = scene
        .visible_nodes()
        .iter()
        .map(|n| n.key.as_str())
        .collect();
    assert_eq!(visible, ["b"]);
}

#[test]
fn selection_roots_skip_nested_selections() {
    let mut scene = scene();
    scene.select_only("a2");
    scene.toggle_selected("a");
    scene.toggle_selected("b");
    assert_eq!(scene.selected_roots(), ["a", "b"]);
    assert_eq!(scene.selected_top_level(), ["a", "b"]);
    assert_eq!(scene.current().unwrap().key, "b");
}

#[test]
fn properties_survive_a_reload_by_name() {
    let mut scene = scene();
    scene.set_visible("a1", false);
    scene.find_mut("b").unwrap().overrides.color = Some([1.0, 0.0, 0.0]);
    let preserved = scene.snapshot_properties();

    let removed = scene.replace(
        vec![
            node("x", "assy", vec![node("x1", "plate", vec![])]),
            node("y", "box", vec![]),
        ],
        Some(&preserved),
    );
    assert_eq!(removed, ["a", "b"]);
    assert!(!scene.find("x1").unwrap().visible);
    assert_eq!(
        scene.find("y").unwrap().overrides.color,
        Some([1.0, 0.0, 0.0])
    );
}

#[test]
fn colors_resolve_in_priority_order() {
    let mut n = node("n", "n", vec![]);
    let default = [0.5; 3];
    assert_eq!(n.part_color(&n.parts[0], default), (default, 1.0));
    n.color = Some([0.0, 0.0, 1.0]);
    n.alpha = Some(0.5);
    assert_eq!(n.part_color(&n.parts[0], default), ([0.0, 0.0, 1.0], 0.5));
    n.overrides.color = Some([1.0, 0.0, 0.0]);
    assert_eq!(n.part_color(&n.parts[0], default).0, [1.0, 0.0, 0.0]);
}

#[test]
fn a_material_without_a_color_has_its_own() {
    use crate::settings::Finish;
    let mut n = node("n", "n", vec![]);
    let default = [0.5; 3];
    assert_eq!(n.finish(), Finish::Default);

    // The script's material, then the Objects panel's.
    n.finish = Some(Finish::Aluminum);
    assert_eq!(n.finish(), Finish::Aluminum);
    let aluminum = Finish::Aluminum.natural_color().unwrap();
    assert_eq!(n.part_color(&n.parts[0], default).0, aluminum);
    n.overrides.finish = Some(Finish::MattePlastic);
    assert_eq!(n.finish(), Finish::MattePlastic);
    assert_eq!(
        n.part_color(&n.parts[0], default).0,
        default,
        "plastics take any color"
    );

    // A color of the object's own wins over the material's.
    n.overrides.finish = Some(Finish::Wood);
    n.color = Some([0.0, 0.0, 1.0]);
    assert_eq!(n.part_color(&n.parts[0], default).0, [0.0, 0.0, 1.0]);
}

#[test]
fn the_material_of_show_object_reaches_the_node() {
    use crate::settings::Finish;
    let message = |finish: Option<&str>| protocol::SceneNode {
        finish: finish.map(str::to_owned),
        ..worker_node("k", triangle_mesh("material"), Mat4::IDENTITY)
    };
    let mut library = MeshLibrary::default();
    let decode = |library: &mut MeshLibrary, finish| {
        SceneNode::from_message(message(finish), library, None)
            .unwrap()
            .finish
    };
    assert_eq!(
        decode(&mut library, Some("brushed_steel")),
        Some(Finish::BrushedSteel)
    );
    assert_eq!(decode(&mut library, None), None);
    assert_eq!(
        decode(&mut library, Some("unobtainium")),
        None,
        "unknown: plain"
    );
    for finish in Finish::ALL {
        assert_eq!(Finish::from_name(finish.name()), Some(finish));
    }
}

#[test]
fn bounding_boxes_include_all_points_and_ignore_empty_boxes() {
    let box_a = Aabb::from_points([Vec3::new(-2.0, 1.0, 3.0), Vec3::new(4.0, 5.0, 7.0)]);
    assert_eq!(Aabb::EMPTY.union(box_a), box_a);
    assert_eq!(box_a.center(), Vec3::new(1.0, 3.0, 5.0));
    assert_eq!(box_a.size(), Vec3::new(6.0, 4.0, 4.0));
    assert!(Aabb::from_points([]).is_empty());
}

#[test]
fn mesh_bounds_include_edges_and_free_points() {
    let mesh = Mesh::new(
        vec![[0.0, 0.0, 0.0]],
        vec![[0.0, 0.0, 1.0]],
        vec![0, 0, 0],
        vec![[-3.0, 1.0, 2.0]],
        vec![[1.0, 4.0, 5.0]],
    );
    assert_eq!(mesh.bounds.min, Vec3::new(-3.0, 0.0, 0.0));
    assert_eq!(mesh.bounds.max, Vec3::new(1.0, 4.0, 5.0));
    assert_eq!(mesh.triangle_count(), 1);
}

#[test]
fn removing_top_level_nodes_prunes_nested_selection() {
    let mut scene = scene();
    scene.select_only("a1");
    scene.toggle_selected("b");
    assert_eq!(scene.remove_top_level(&["a1".into()]), Vec::<String>::new());
    assert_eq!(scene.remove_top_level(&["a".into()]), ["a"]);
    assert_eq!(scene.selection, ["b"]);
    assert_eq!(scene.top_level_keys(), ["b"]);
}

#[test]
fn clearing_scene_returns_keys_and_removes_selection() {
    let mut scene = scene();
    scene.select_only("a2");
    assert_eq!(scene.clear(), ["a", "b"]);
    assert!(scene.is_empty());
    assert!(scene.selection.is_empty());
    assert!(scene.current().is_none());
}

#[test]
fn visibility_can_be_restored_for_an_entire_subtree() {
    let mut scene = scene();
    scene.set_visible("a", false);
    scene.set_visible("a", true);
    assert_eq!(
        scene
            .visible_nodes()
            .iter()
            .map(|n| n.key.as_str())
            .collect::<Vec<_>>(),
        ["a", "a1", "a2", "b"]
    );
}

#[test]
fn selection_bounds_include_children_and_overlay_affects_visible_bounds() {
    let mut scene = scene();
    scene.select_only("a");
    let selected = scene.selection_bounds();
    assert_eq!(selected.max, Vec3::new(1.0, 2.0, 3.0));
    scene.overlay.meshes.push(Arc::new(Mesh::lines(vec![
        [20.0, 0.0, 0.0],
        [21.0, 0.0, 0.0],
    ])));
    assert_eq!(scene.visible_bounds().max.x, 21.0);
}

#[test]
fn hidden_nodes_are_excluded_from_visible_bounds() {
    let mut scene = scene();
    scene.set_visible("a", false);
    scene.set_visible("b", false);
    assert!(scene.visible_bounds().is_empty());
    scene.select_only("a");
    assert!(!scene.selection_bounds().is_empty());
}

#[test]
fn topology_selection_builds_and_clears_an_exact_highlight() {
    let mut mesh = Mesh::new(
        vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        vec![[0.0, 0.0, 1.0]; 3],
        vec![0, 1, 2],
        vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
        Vec::new(),
    );
    mesh.topology.face_positions = mesh.positions.clone();
    mesh.topology.face_indices = mesh.indices.clone();
    mesh.topology.triangle_faces = vec![4];
    mesh.topology.segment_edges = vec![7];
    mesh.topology.vertices = vec![[0.0, 0.0, 0.0]];

    let mut part_node = node("n", "part", vec![]);
    part_node.parts[0].mesh = Arc::new(mesh);
    let mut scene = Scene::default();
    scene.append(vec![part_node], None);
    scene.select_only("n");

    let selection = TopologySelection {
        node: "n".into(),
        part: 0,
        entity: TopologyEntity::Face(4),
        position: Vec3::new(0.25, 0.25, 0.0),
    };
    assert!(scene.select_topology(selection.clone(), false));
    assert_eq!(scene.topology_selections, vec![selection.clone()]);
    assert_eq!(scene.topology_highlights()[0].mesh.triangle_count(), 1);

    let edge = TopologySelection {
        entity: TopologyEntity::Edge(7),
        ..selection
    };
    assert!(scene.select_topology(edge.clone(), true));
    assert_eq!(scene.topology_selections.len(), 2);
    assert_eq!(scene.current_topology(), Some(&edge));

    // Ctrl-selecting the same topology toggles only that entity.
    assert!(scene.select_topology(edge, true));
    assert_eq!(scene.topology_selections.len(), 1);

    scene.select_only("n");
    assert!(scene.topology_selections.is_empty());
    assert!(scene.topology_highlights().is_empty());
}

#[test]
fn selector_preview_highlights_without_changing_selection() {
    let mut mesh = Mesh::new(
        vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        vec![[0.0, 0.0, 1.0]; 3],
        vec![0, 1, 2],
        Vec::new(),
        Vec::new(),
    );
    mesh.topology.triangle_faces = vec![4];
    let mut part = node("n", "part", vec![]);
    part.parts[0].mesh = Arc::new(mesh);
    let mut scene = Scene::default();
    scene.append(vec![part], None);
    scene.select_only("n");
    scene.set_selector_preview(&[protocol::TopologyReference {
        key: "n".into(),
        part: 0,
        kind: protocol::TopologyKind::Face,
        identifier: 4,
    }]);
    assert_eq!(scene.selector_preview_count(), 1);
    assert_eq!(scene.placed_topology_highlights().len(), 1);
    assert_eq!(scene.selected_roots(), ["n"]);
    assert!(scene.topology_selections.is_empty());
    scene.clear();
    assert_eq!(scene.selector_preview_count(), 0);
}

#[test]
fn ctrl_click_gathers_up_to_the_topology_selection_limit() {
    // One triangle per face id, so each id has a highlight.
    let faces = MAX_TOPOLOGY_SELECTIONS as u32 + 1;
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    for i in 0..faces {
        let x = i as f32;
        let start = positions.len() as u32;
        positions.extend([[x, 0.0, 0.0], [x + 0.5, 0.0, 0.0], [x, 0.5, 0.0]]);
        indices.extend([start, start + 1, start + 2]);
    }
    let normals = vec![[0.0, 0.0, 1.0]; positions.len()];
    let mut mesh = Mesh::new(positions, normals, indices, Vec::new(), Vec::new());
    mesh.topology.triangle_faces = (1..=faces).collect();
    let mut part_node = node("n", "part", vec![]);
    part_node.parts[0].mesh = Arc::new(mesh);
    let mut scene = Scene::default();
    scene.append(vec![part_node], None);
    scene.select_only("n");

    let face = |id| TopologySelection {
        node: "n".into(),
        part: 0,
        entity: TopologyEntity::Face(id),
        position: Vec3::ZERO,
    };
    for id in 1..=faces {
        assert!(scene.select_topology(face(id), id > 1));
    }
    // The oldest made room for the newest.
    assert_eq!(scene.topology_selections.len(), MAX_TOPOLOGY_SELECTIONS);
    assert_eq!(scene.topology_selections[0].entity, TopologyEntity::Face(2));
    assert_eq!(scene.current_topology(), Some(&face(faces)));
    assert_eq!(scene.topology_highlights().len(), MAX_TOPOLOGY_SELECTIONS);
}

#[test]
fn face_highlights_use_display_triangles_without_separate_pick_triangles() {
    let mut mesh = Mesh::new(
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ],
        vec![[0.0, 0.0, 1.0]; 4],
        vec![0, 1, 2, 0, 2, 3],
        Vec::new(),
        Vec::new(),
    );
    mesh.topology.triangle_faces = vec![2, 5];

    let highlight = mesh.topology_mesh(TopologyEntity::Face(5)).unwrap();
    assert_eq!(highlight.triangle_count(), 1);
    assert_eq!(
        highlight.positions,
        [[0.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]]
    );
    assert!(mesh.topology_mesh(TopologyEntity::Face(9)).is_none());
}

fn worker_node(key: &str, mesh: protocol::Mesh, transform: Mat4) -> protocol::SceneNode {
    protocol::SceneNode {
        key: key.into(),
        name: key.into(),
        kind: "Shape".into(),
        inspectable: false,
        line: None,
        color: None,
        alpha: None,
        edge_color: None,
        finish: None,
        parts: vec![protocol::Part {
            mesh,
            transform: transform.to_cols_array(),
            color: None,
            alpha: None,
            face_attributes: Vec::new(),
            material: None,
        }],
        children: Vec::new(),
    }
}

#[test]
fn content_addressed_mesh_references_reuse_cpu_geometry() {
    let mut scene = Scene::default();
    let full = protocol::Mesh {
        id: "same-geometry".into(),
        positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 3],
        indices: vec![0, 1, 2],
        bbox: Some([[0.0, 0.0, 0.0], [1.0, 1.0, 0.0]]),
        ..Default::default()
    };
    let first = scene
        .decode_nodes(None, vec![worker_node("first", full, Mat4::IDENTITY)])
        .unwrap();
    scene.append(first, None);
    assert_eq!(scene.known_geometry(), ["same-geometry"]);

    let reference = protocol::Mesh {
        id: "same-geometry".into(),
        ..Default::default()
    };
    let second = scene
        .decode_nodes(
            None,
            vec![worker_node(
                "second",
                reference,
                Mat4::from_translation(Vec3::X * 10.0),
            )],
        )
        .unwrap();
    assert!(Arc::ptr_eq(
        &scene.nodes[0].parts[0].mesh,
        &second[0].parts[0].mesh
    ));
    assert_eq!(second[0].own_bounds().min.x, 10.0);
    assert_eq!(second[0].own_bounds().max.x, 11.0);
}

#[test]
fn streamed_part_is_visible_before_finish_and_resolves_in_final_scene() {
    let mut scene = Scene::default();
    let message = worker_node("part", triangle_mesh("streamed"), Mat4::IDENTITY);
    scene
        .add_preview_part(7, message.parts.into_iter().next().unwrap())
        .unwrap();
    assert_eq!(scene.preview_parts.len(), 1);
    assert_eq!(scene.visible_bounds().max.x, 1.0);

    let nodes = scene
        .decode_nodes(
            Some(7),
            vec![worker_node(
                "part",
                mesh_reference("streamed"),
                Mat4::IDENTITY,
            )],
        )
        .unwrap();
    assert!(Arc::ptr_eq(
        &scene.preview_parts[0].mesh,
        &nodes[0].parts[0].mesh
    ));
    scene.clear_preview();
    scene.append(nodes, None);
    scene.release_geometry(|_| false);
    assert!(scene.preview_parts.is_empty());
    assert_eq!(scene.nodes[0].parts[0].mesh.indices, [0, 1, 2]);
}

#[test]
fn face_colors_are_instance_local_and_preserve_picking_geometry() {
    let mut scene = Scene::default();
    let mut first = worker_node("first", triangle_mesh("shared"), Mat4::IDENTITY);
    first.parts[0].mesh.topology.triangle_faces = vec![1];
    first.parts[0].face_attributes = vec![protocol::FaceAttribute {
        id: 1,
        name: Some("painted".into()),
        layer: Some("finish".into()),
        color: Some([1.0, 0.0, 0.0]),
        alpha: Some(1.0),
    }];
    let nodes = scene.decode_nodes(None, vec![first]).unwrap();
    scene.append(nodes, None);

    let mut second = worker_node("second", mesh_reference("shared"), Mat4::IDENTITY);
    second.parts[0].face_attributes = vec![protocol::FaceAttribute {
        color: Some([0.0, 1.0, 0.0]),
        ..scene.nodes[0].parts[0].face_attributes[0].clone()
    }];
    let nodes = scene.decode_nodes(None, vec![second]).unwrap();
    assert!(Arc::ptr_eq(
        &scene.nodes[0].parts[0].mesh,
        &nodes[0].parts[0].mesh
    ));
    let first_color = scene.nodes[0].parts[0]
        .render_mesh
        .as_ref()
        .unwrap()
        .vertex_colors[0];
    let second_color = nodes[0].parts[0]
        .render_mesh
        .as_ref()
        .unwrap()
        .vertex_colors[0];
    assert_eq!(first_color, 0xff00_00ff);
    assert_eq!(second_color, 0xff00_ff00);
    assert_eq!(scene.nodes[0].parts[0].mesh.indices, vec![0, 1, 2]);
}

#[test]
fn an_unknown_mesh_reference_is_rejected() {
    let mut scene = Scene::default();
    let reference = protocol::Mesh {
        id: "missing".into(),
        ..Default::default()
    };
    let error = scene
        .decode_nodes(None, vec![worker_node("node", reference, Mat4::IDENTITY)])
        .unwrap_err();
    assert!(error.contains("missing"));
}

/// A full one-triangle worker mesh of 84 bytes.
fn triangle_mesh(id: &str) -> protocol::Mesh {
    protocol::Mesh {
        id: id.into(),
        positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 3],
        indices: vec![0, 1, 2],
        bbox: Some([[0.0, 0.0, 0.0], [1.0, 1.0, 0.0]]),
        ..Default::default()
    }
}

fn mesh_reference(id: &str) -> protocol::Mesh {
    protocol::Mesh {
        id: id.into(),
        ..Default::default()
    }
}

/// Decodes a one-part node answering `request`.
fn decode_part(
    scene: &mut Scene,
    request: Option<RequestId>,
    mesh: protocol::Mesh,
) -> Result<Arc<Mesh>, String> {
    let mut nodes = scene.decode_nodes(request, vec![worker_node("n", mesh, Mat4::IDENTITY)])?;
    Ok(nodes.remove(0).parts.remove(0).mesh)
}

fn without_retention() -> Scene {
    Scene {
        meshes: MeshLibrary::with_budget(0),
        ..Scene::default()
    }
}

#[test]
fn a_debugging_session_resolves_meshes_its_paused_views_dropped() {
    // Without retention only the request's own pins can keep the mesh.
    let mut scene = without_retention();
    let run = 7;
    // Pause 1 shows `a`; the worker now treats it as known to the GUI.
    let first = decode_part(&mut scene, Some(run), triangle_mesh("a")).unwrap();
    let nodes = scene
        .decode_nodes(
            Some(run),
            vec![worker_node("a", mesh_reference("a"), Mat4::IDENTITY)],
        )
        .unwrap();
    scene.replace(nodes, None);
    drop(first);
    // Pause 2, inside a function, shows nothing.
    scene.replace(Vec::new(), None);

    // Pause 3 shows `a` again, as a reference.
    let again = decode_part(&mut scene, Some(run), mesh_reference("a"))
        .expect("a mesh sent earlier in the session stays available");
    assert_eq!(again.positions.len(), 3);
    drop(again);

    // Once the request ends, the worker may no longer rely on it.
    scene.release_geometry(|_| false);
    assert!(scene.known_geometry().is_empty());
    assert!(decode_part(&mut scene, Some(8), mesh_reference("a")).is_err());
}

#[test]
fn advertised_geometry_survives_clearing_the_view_until_its_request_ends() {
    let mut scene = without_retention();
    let shown = scene
        .decode_nodes(
            None,
            vec![worker_node("a", triangle_mesh("a"), Mat4::IDENTITY)],
        )
        .unwrap();
    scene.append(shown, None);
    let known = scene.known_geometry();
    assert_eq!(known, ["a"]);

    // A debugging run clears the view right after sending its request.
    scene.pin_geometry(3, &known);
    scene.clear();
    assert!(decode_part(&mut scene, Some(3), mesh_reference("a")).is_ok());

    scene.release_geometry(|request| request != 3);
    assert!(scene.known_geometry().is_empty());
}

#[test]
fn recently_used_meshes_are_retained_within_the_byte_budget() {
    // Room for two 84-byte meshes.
    let mut scene = Scene {
        meshes: MeshLibrary::with_budget(200),
        ..Scene::default()
    };
    let first = decode_part(&mut scene, Some(1), triangle_mesh("a")).unwrap();
    let first_id = first.id;
    drop(first);
    for (request, id) in [(2, "b"), (3, "c")] {
        decode_part(&mut scene, Some(request), triangle_mesh(id)).unwrap();
    }
    scene.release_geometry(|_| false);

    // Nothing is shown, yet the two newest meshes remain known.
    assert_eq!(scene.known_geometry(), ["b", "c"]);
    assert!(decode_part(&mut scene, None, mesh_reference("a")).is_err());

    // Using `b` again makes `c` the oldest, so a new mesh evicts `c`.
    decode_part(&mut scene, None, mesh_reference("b")).unwrap();
    decode_part(&mut scene, None, triangle_mesh("d")).unwrap();
    assert_eq!(scene.known_geometry(), ["b", "d"]);

    // A fresh copy of evicted geometry is a new mesh with its own buffers.
    assert_ne!(
        decode_part(&mut scene, None, triangle_mesh("a"))
            .unwrap()
            .id,
        first_id
    );
}

#[test]
fn a_full_mesh_for_held_geometry_keeps_the_existing_mesh_and_gpu_buffers() {
    let mut scene = Scene::default();
    let first = decode_part(&mut scene, None, triangle_mesh("a")).unwrap();
    let resent = decode_part(&mut scene, None, triangle_mesh("a")).unwrap();
    assert!(Arc::ptr_eq(&first, &resent));
    // The renderer keys GPU buffers by this id.
    assert_eq!(first.id, resent.id);
}

#[test]
fn mesh_byte_size_counts_every_array() {
    let mut mesh = Mesh::from(triangle_mesh("a"));
    assert_eq!(mesh.byte_size(), 3 * 12 + 3 * 12 + 3 * 4);
    mesh.topology.triangle_faces = vec![1];
    mesh.topology.vertices = vec![[0.0; 3]; 2];
    assert_eq!(mesh.byte_size(), 84 + 4 + 2 * 12);
}

// -- exploded view and isolation ---------------------------------------------

/// `node` with its part moved by `offset`.
fn placed(key: &str, name: &str, offset: Vec3, children: Vec<SceneNode>) -> SceneNode {
    let mut node = node(key, name, children);
    node.parts[0].transform = Mat4::from_translation(offset);
    node
}

/// An assembly of two plates left and right of the origin, and a box.
fn assembly(prefix: &str, right: &str) -> Vec<SceneNode> {
    vec![
        SceneNode {
            parts: Vec::new(),
            ..node(
                &format!("{prefix}a"),
                "assy",
                vec![
                    placed(
                        &format!("{prefix}a1"),
                        "left",
                        Vec3::new(-10.0, 0.0, 0.0),
                        vec![],
                    ),
                    placed(
                        &format!("{prefix}a2"),
                        right,
                        Vec3::new(10.0, 0.0, 0.0),
                        vec![],
                    ),
                ],
            )
        },
        placed(
            &format!("{prefix}b"),
            "box",
            Vec3::new(0.0, 0.0, 20.0),
            vec![],
        ),
    ]
}

fn assembly_scene() -> Scene {
    let mut scene = Scene::default();
    scene.append(assembly("", "right"), None);
    scene
}

fn part_center(scene: &Scene, key: &str) -> Vec3 {
    let part = &scene.find(key).unwrap().parts[0];
    part.mesh
        .bounds
        .transformed(scene.part_transform(part))
        .center()
}

fn shown(scene: &Scene) -> Vec<String> {
    scene
        .visible_nodes()
        .iter()
        .map(|node| node.key.clone())
        .collect()
}

#[test]
fn exploding_moves_parts_away_from_the_centre_and_back() {
    let mut scene = assembly_scene();
    let assembled = scene.visible_bounds();
    let center = assembled.center();
    let before = part_center(&scene, "a1");
    let revision = scene.pick_revision();

    scene.set_explode(1.0);
    assert_ne!(scene.pick_revision(), revision, "picking follows the parts");
    let after = part_center(&scene, "a1");
    // Twice as far from the centre, in the same direction.
    assert!((after - center).abs_diff_eq((before - center) * 2.0, 1e-4));
    assert!(scene.visible_bounds().size().x > assembled.size().x * 1.5);
    // The placements, which measurements and exports use, stay put.
    assert_eq!(
        scene.find("a1").unwrap().parts[0].transform,
        Mat4::from_translation(Vec3::new(-10.0, 0.0, 0.0))
    );

    scene.set_explode(100.0);
    assert_eq!(scene.explode(), MAX_EXPLODE);
    scene.set_explode(f32::NAN);
    assert_eq!(scene.explode(), 0.0);
    assert_eq!(part_center(&scene, "a1"), before);
    assert_eq!(scene.visible_bounds(), assembled);
}

#[test]
fn selection_bounds_and_highlights_follow_the_exploded_parts() {
    let mut scene = assembly_scene();
    scene.set_explode(1.0);
    scene.select_only("a2");
    assert!(
        (scene.selection_bounds().center() - part_center(&scene, "a2")).length() < 1e-4,
        "zoom to selection frames the part where it is shown"
    );

    let mut mesh = Mesh::new(
        vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        vec![[0.0, 0.0, 1.0]; 3],
        vec![0, 1, 2],
        Vec::new(),
        Vec::new(),
    );
    mesh.topology.triangle_faces = vec![1];
    scene.find_mut("a2").unwrap().parts[0].mesh = Arc::new(mesh);
    assert!(scene.select_topology(
        TopologySelection {
            node: "a2".into(),
            part: 0,
            entity: TopologyEntity::Face(1),
            position: Vec3::ZERO,
        },
        false,
    ));
    let part = &scene.find("a2").unwrap().parts[0];
    let highlights = scene.placed_topology_highlights();
    assert_eq!(highlights.len(), 1);
    assert_eq!(highlights[0].1, scene.part_transform(part));
    assert_ne!(highlights[0].1, part.transform);
}

#[test]
fn isolating_shows_only_the_isolated_subtrees() {
    let mut scene = assembly_scene();
    assert_eq!(shown(&scene), ["a", "a1", "a2", "b"]);

    scene.isolate(&["a2".to_owned()]);
    assert!(scene.is_isolating());
    assert_eq!(scene.isolated_count(), 1);
    assert_eq!(shown(&scene), ["a2"]);
    let members = scene.isolation_members().unwrap();
    assert!(members.contains("a2") && !members.contains("a") && !members.contains("b"));
    assert!(
        (scene.visible_bounds().center() - part_center(&scene, "a2")).length() < 1e-4,
        "fit frames what is isolated"
    );

    // A whole assembly, with a hidden part that stays hidden.
    scene.set_visible("a1", false);
    scene.isolate(&["a".to_owned()]);
    assert_eq!(shown(&scene), ["a", "a2"]);
    // Nothing known to isolate: no change.
    scene.isolate(&["missing".to_owned()]);
    assert_eq!(shown(&scene), ["a", "a2"]);

    scene.end_isolation();
    assert!(!scene.is_isolating() && scene.isolation_members().is_none());
    assert_eq!(shown(&scene), ["a", "a2", "b"], "visibility flags are kept");

    scene.isolate(&["b".to_owned()]);
    scene.show_all();
    assert!(!scene.is_isolating());
    assert_eq!(shown(&scene), ["a", "a1", "a2", "b"]);
}

#[test]
fn isolation_survives_a_new_run_by_name_and_ends_without_a_match() {
    let mut scene = assembly_scene();
    scene.isolate(&["a2".to_owned()]);

    // The next run has new keys for the same names.
    scene.replace(assembly("2", "right"), None);
    assert_eq!(shown(&scene), ["2a2"]);

    // Renamed: isolating nothing would show nothing, so the isolation ends.
    scene.replace(assembly("3", "renamed"), None);
    assert!(!scene.is_isolating());
    assert_eq!(shown(&scene), ["3a", "3a1", "3a2", "3b"]);
}

// -- simplified meshes ---------------------------------------------------------

/// A sphere of `rings` × `segments` quads, each corner with its own normal.
fn sphere(radius: f32, rings: u32, segments: u32) -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    for ring in 0..=rings {
        let polar = std::f32::consts::PI * ring as f32 / rings as f32;
        for segment in 0..=segments {
            let azimuth = std::f32::consts::TAU * segment as f32 / segments as f32;
            let normal = Vec3::new(
                polar.sin() * azimuth.cos(),
                polar.sin() * azimuth.sin(),
                polar.cos(),
            );
            positions.push((normal * radius).to_array());
            normals.push(normal.to_array());
        }
    }
    let mut indices = Vec::new();
    let row = segments + 1;
    for ring in 0..rings {
        for segment in 0..segments {
            let a = ring * row + segment;
            let (b, c, d) = (a + 1, a + row, a + row + 1);
            indices.extend([a, c, b, b, c, d]);
        }
    }
    let edges = (0..segments)
        .flat_map(|segment| {
            let at = |s: u32| {
                let azimuth = std::f32::consts::TAU * s as f32 / segments as f32;
                [radius * azimuth.cos(), radius * azimuth.sin(), 0.0]
            };
            [at(segment), at(segment + 1)]
        })
        .collect();
    Mesh::new(positions, normals, indices, edges, vec![[0.0, 0.0, radius]])
}

#[test]
fn fine_meshes_get_a_coarser_copy_for_drawing_small() {
    let fine = sphere(10.0, 120, 240);
    let simplified = fine.simplified().expect("a fine mesh is simplified");
    assert!(
        std::ptr::eq(simplified, fine.simplified().unwrap()),
        "made once"
    );
    assert!(simplified.triangle_count() * 4 < fine.triangle_count());
    assert!(simplified.triangle_count() > 100, "still a sphere");

    // Within a cell of the original everywhere, with valid indices and
    // unit or zero normals.
    let cell = 20.0 / 40.0;
    assert!(
        (simplified.bounds.min - fine.bounds.min)
            .abs()
            .max_element()
            <= cell
    );
    assert!(
        (simplified.bounds.max - fine.bounds.max)
            .abs()
            .max_element()
            <= cell
    );
    for position in &simplified.positions {
        let radius = Vec3::from_array(*position).length();
        assert!((radius - 10.0).abs() <= cell, "{radius}");
    }
    assert!(
        simplified
            .indices
            .iter()
            .all(|&index| (index as usize) < simplified.positions.len())
    );
    for normal in &simplified.normals {
        let length = Vec3::from_array(*normal).length();
        assert!(length < 1e-6 || (length - 1.0).abs() < 1e-4);
    }
    assert!(!simplified.edges.is_empty() && simplified.edges.len() <= fine.edges.len());
    assert_eq!(simplified.points, fine.points);
    // Only for drawing: no picking data.
    assert!(simplified.topology.triangle_faces.is_empty());

    // Small meshes are left alone.
    assert!(sphere(10.0, 10, 20).simplified().is_none());
}

#[test]
fn the_coarser_copy_keeps_the_angle_of_each_edge() {
    let mut fine = sphere(10.0, 120, 240);
    // Every other segment smooth.
    fine.segment_angles = (0..fine.edges.len() / 2)
        .map(|i| if i % 2 == 0 { 0.0 } else { 90.0 })
        .collect();
    let simplified = fine.simplified().unwrap();
    assert_eq!(simplified.segment_angles.len() * 2, simplified.edges.len());
    assert!(simplified.segment_angles.contains(&0.0));
    assert!(simplified.segment_angles.contains(&90.0));
}

// -- edge angles and silhouettes ---------------------------------------------

#[test]
fn segments_without_an_angle_count_as_outlines() {
    let mut mesh = Mesh::lines(vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0; 3], [0.0, 1.0, 0.0]]);
    assert_eq!(mesh.segment_angle(1), OUTLINE_ANGLE);
    mesh.segment_angles = vec![12.0, 90.0];
    assert_eq!((mesh.segment_angle(0), mesh.segment_angle(1)), (12.0, 90.0));
}

#[test]
fn worker_angles_are_kept_only_when_there_is_one_per_segment() {
    let lines = |angles: Vec<f32>| {
        Mesh::from(protocol::Mesh {
            edges: vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0; 3], [0.0, 1.0, 0.0]],
            segment_angles: angles,
            ..Default::default()
        })
    };
    assert_eq!(lines(vec![0.0, 90.0]).segment_angles, [0.0, 90.0]);
    assert!(lines(vec![0.0]).segment_angles.is_empty(), "damaged: none");
}

/// Four corners of the square [0, 1]² at height `z`, normals up.
fn square(z: f32) -> (Vec<[f32; 3]>, Vec<[f32; 3]>) {
    let positions = vec![[0.0, 0.0, z], [1.0, 0.0, z], [1.0, 1.0, z], [0.0, 1.0, z]];
    (positions, vec![[0.0, 0.0, 1.0]; 4])
}

#[test]
fn a_flat_face_has_no_silhouette() {
    let (positions, normals) = square(0.0);
    let mesh = Mesh::new(
        positions,
        normals,
        vec![0, 1, 2, 0, 2, 3],
        Vec::new(),
        Vec::new(),
    );
    assert!(
        mesh.silhouette_edges().is_empty(),
        "the diagonal lies in the plane"
    );
}

#[test]
fn silhouette_edges_join_faces_meshed_apart_at_a_crease() {
    // A roof: two faces with vertices of their own, meeting along x = 0.5
    // at a right angle; corners at the same place are one corner.
    let positions = vec![
        [0.0, 0.0, 0.0],
        [0.5, 0.0, 0.5],
        [0.5, 1.0, 0.5],
        [0.0, 1.0, 0.0],
        [0.5, 0.0, 0.5],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.5, 1.0, 0.5],
    ];
    let left = Vec3::new(-1.0, 0.0, 1.0).normalize().to_array();
    let right = Vec3::new(1.0, 0.0, 1.0).normalize().to_array();
    let normals = vec![left, left, left, left, right, right, right, right];
    let indices = vec![0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7];
    let mesh = Mesh::new(positions, normals, indices, Vec::new(), Vec::new());

    let edges = mesh.silhouette_edges();
    assert_eq!(edges.len(), 1, "only the ridge: {edges:?}");
    let ridge = edges[0];
    let ends = [&ridge[0..3], &ridge[3..6]];
    assert!(ends.iter().all(|end| end[0] == 0.5 && end[2] == 0.5));
    let normals = [
        Vec3::from_slice(&ridge[6..9]),
        Vec3::from_slice(&ridge[9..12]),
    ];
    assert!((normals[0].dot(normals[1])).abs() < 1e-5, "a right angle");
    assert!(std::ptr::eq(edges, mesh.silhouette_edges()), "found once");
}

#[test]
fn a_smooth_surface_offers_its_mesh_edges_as_silhouettes() {
    let sphere = sphere(1.0, 12, 24);
    let edges = sphere.silhouette_edges();
    // Every edge between two quads' triangles across the curved surface,
    // not the diagonals inside a flat quad.
    assert!(edges.len() > 12 * 24);
    for edge in edges {
        let (a, b) = (
            Vec3::from_slice(&edge[6..9]),
            Vec3::from_slice(&edge[9..12]),
        );
        assert!(a.dot(b) < 0.999_99);
    }
}
