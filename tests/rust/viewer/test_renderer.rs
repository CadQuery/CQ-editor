use super::*;

/// WGSL source parsed and validated by wgpu's own front end.
fn validated(source: &str) -> naga::Module {
    let module = naga::front::wgsl::parse_str(source)
        .unwrap_or_else(|error| panic!("{}", error.emit_to_string(source)));
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .unwrap_or_else(|error| panic!("invalid WGSL: {error:?}"));
    module
}

/// The scene shaders.
fn scene_module() -> naga::Module {
    validated(include_str!("../../../src/viewer/shaders.wgsl"))
}

/// The size WGSL lays out for the uniform variable `name`.
fn uniform_size(module: &naga::Module, name: &str) -> u32 {
    let mut layouter = naga::proc::Layouter::default();
    layouter.update(module.to_ctx()).unwrap();
    let (_, variable) = module
        .global_variables
        .iter()
        .find(|(_, variable)| variable.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no uniform {name}"));
    layouter[variable.ty].size
}

#[test]
fn the_scene_shaders_are_valid_wgsl_with_every_entry_point() {
    let module = scene_module();
    let entry_points: Vec<&str> = module
        .entry_points
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    for name in [
        "vs_background",
        "fs_background",
        "vs_mesh",
        "fs_mesh",
        "fs_section_count",
        "vs_cap",
        "fs_cap",
        "vs_edge",
        "fs_edge",
        "fs_hidden_edge",
        "vs_point",
        "fs_point",
    ] {
        assert!(entry_points.contains(&name), "missing entry point {name}");
    }
}

#[test]
fn uniform_blocks_match_their_wgsl_layout() {
    let module = scene_module();
    assert_eq!(
        uniform_size(&module, "camera") as usize,
        std::mem::size_of::<CameraUniform>()
    );
    assert_eq!(
        uniform_size(&module, "object") as usize,
        std::mem::size_of::<ObjectUniform>()
    );
}

#[test]
fn the_stereo_shader_is_valid_wgsl_matching_its_uniform() {
    let module = validated(include_str!("../../../src/viewer/stereo.wgsl"));
    let entry_points: Vec<&str> = module
        .entry_points
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(entry_points, ["vs_compose", "fs_compose"]);
    assert_eq!(
        uniform_size(&module, "compose") as usize,
        std::mem::size_of::<ComposeUniform>()
    );
}

#[test]
fn the_present_shader_is_valid_wgsl() {
    let module = validated(include_str!("../../../src/viewer/present.wgsl"));
    let entry_points: Vec<&str> = module
        .entry_points
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(entry_points, ["vs_present", "fs_present"]);
}

// -- on a GPU ------------------------------------------------------------------

use std::sync::{Mutex, OnceLock, PoisonError};

use crate::viewer::camera::{Eye, Pose, StandardView};
use crate::viewer::section::{Section, SectionAxis};
use crate::viewer::{DisplayMode, PartLook};

const BACKGROUND: [f32; 4] = [0.0, 0.0, 1.0, 1.0];
const BACKGROUND_SRGB: [u8; 4] = [0, 0, 255, 255];
/// Image size; the 2-unit cube spans half of it.
const SIZE: u32 = 128;

/// A renderer on the adapter `WGPU_ADAPTER_NAME` names, or else the best
/// one wgpu finds, software adapters included (`WGPU_BACKEND` limits the
/// backends). Without any adapter there is nothing to draw on and the GPU
/// tests pass without checking anything, unless `CQ_EDITOR_REQUIRE_GPU` is
/// set to something other than `0`, as in CI on Windows, whose software
/// adapter (WARP) is always there.
fn gpu() -> Option<SceneRenderer> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle().with_env());
    let adapter = match block_on(wgpu::util::initialize_adapter_from_env_or_default(
        &instance, None,
    )) {
        Ok(adapter) => adapter,
        Err(error) => {
            let required = std::env::var("CQ_EDITOR_REQUIRE_GPU")
                .is_ok_and(|value| !value.is_empty() && value != "0");
            assert!(!required, "no GPU adapter: {error}");
            eprintln!("no GPU adapter ({error}): skipping the rendering test");
            return None;
        }
    };
    eprintln!("rendering on {:?}", adapter.get_info());
    let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
        .expect("the adapter provides a device");
    Some(SceneRenderer::with_device(device, queue, &adapter))
}

/// Runs a GPU test on the renderer the GPU tests share, one test at a time:
/// creating devices concurrently has crashed the software DX12 adapter.
fn with_gpu(test: impl FnOnce(&mut SceneRenderer)) {
    static GPU: OnceLock<Mutex<Option<SceneRenderer>>> = OnceLock::new();
    let mut gpu = GPU
        .get_or_init(|| Mutex::new(gpu()))
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if let Some(renderer) = gpu.as_mut() {
        test(renderer);
    }
}

/// wgpu's native futures are ready once polled; this is all they need.
fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    loop {
        if let std::task::Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
        std::thread::yield_now();
    }
}

/// A closed cube with flat faces and its 12 edges, centred on the origin.
fn cube(half: f32) -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();
    for axis in 0..3 {
        for sign in [-1.0f32, 1.0] {
            let mut normal = [0.0; 3];
            normal[axis] = sign;
            let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
            let base = positions.len() as u32;
            for (a, b) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                let mut corner = [0.0; 3];
                corner[axis] = sign * half;
                corner[u] = a * half;
                corner[v] = b * half;
                positions.push(corner);
                normals.push(normal);
            }
            indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
    let mut edges = Vec::new();
    let corner = |bits: u32| [0, 1, 2].map(|i| if bits >> i & 1 == 1 { half } else { -half });
    for bits in 0..8u32 {
        for axis in 0..3 {
            if bits >> axis & 1 == 0 {
                edges.extend([corner(bits), corner(bits | 1 << axis)]);
            }
        }
    }
    assert_eq!(edges.len(), 24);
    Mesh::new(positions, normals, indices, edges, Vec::new())
}

/// A red cube with black edges, as `DisplayMode::Shaded` draws it.
fn red() -> Style {
    Style {
        face_color: [1.0, 0.0, 0.0, 1.0],
        edge_color: [0.0, 0.0, 0.0, 1.0],
        highlight: [0.0; 4],
        edge_width: 1.5,
        point_size: 1.0,
        faces: true,
        edges: false,
        points: false,
        clipped: true,
        background_fill: false,
        hidden_edges: false,
        edge_min_angle: 0.0,
        silhouettes: false,
        surface: [0.0; 4],
    }
}

/// The style a display mode gives the red cube.
fn in_mode(mode: DisplayMode) -> Style {
    let settings = crate::settings::Viewer::default();
    let look = PartLook {
        face_color: [1.0, 0.0, 0.0, 1.0],
        edge_color: [0.0, 0.0, 0.0, 1.0],
        highlight: [0.0; 4],
        has_faces: true,
        emphasized: false,
        edges: None,
        finish: crate::settings::Finish::Default,
    };
    mode.style(look, &settings, &crate::settings::Rendering::default())
}

struct Frame {
    side: u32,
    pixels: Vec<u8>,
}

impl Frame {
    fn at(&self, x: u32, y: u32) -> [u8; 4] {
        let start = ((y * self.side + x) * 4) as usize;
        <[u8; 4]>::try_from(&self.pixels[start..start + 4]).unwrap()
    }
}

/// Renders the 2-unit cube from `view` in a `SIZE` square image whose
/// half-height is 2 units.
fn draw(
    renderer: &mut SceneRenderer,
    view: StandardView,
    style: Style,
    section: Option<Section>,
) -> Frame {
    draw_sized(renderer, view, style, section, SIZE, 1.0, BACKGROUND)
}

/// `draw` in a `side` square image, with widths times `pixel_scale`.
fn draw_sized(
    renderer: &mut SceneRenderer,
    view: StandardView,
    style: Style,
    section: Option<Section>,
    side: u32,
    pixel_scale: f32,
    background: [f32; 4],
) -> Frame {
    draw_lit(
        renderer,
        view,
        style,
        section,
        side,
        pixel_scale,
        background,
        Lighting::default(),
    )
}

/// `draw_sized` with `lighting`.
#[allow(clippy::too_many_arguments)]
fn draw_lit(
    renderer: &mut SceneRenderer,
    view: StandardView,
    style: Style,
    section: Option<Section>,
    side: u32,
    pixel_scale: f32,
    background: [f32; 4],
    lighting: Lighting,
) -> Frame {
    let mesh = Arc::new(cube(1.0));
    let bounds = mesh.bounds;
    let mut camera = Camera {
        scene_radius: 2.0,
        ..Camera::default()
    };
    camera.set_pose(Pose {
        target: Vec3::ZERO,
        rotation: view.rotation(),
        extent: 2.0,
    });
    let items = [DrawItem {
        mesh,
        transform: Mat4::IDENTITY,
        style,
        layer: Layer::Scene,
    }];
    let (size, pixels) = renderer
        .render_pixels(&FrameInput {
            camera: &camera,
            size: [side, side],
            background_top: background,
            background_bottom: background,
            items: &items,
            section: section.and_then(|section| section.renderer_plane(bounds)),
            pixel_scale,
            eye: Eye::Center,
            lighting,
            ground_shadow: None,
        })
        .expect("the frame is read back");
    assert_eq!(size, [side, side]);
    Frame { side, pixels }
}

fn cut(offset: f32, caps: bool) -> Option<Section> {
    Some(Section {
        enabled: true,
        axis: SectionAxis::Z,
        offset,
        flipped: false,
        caps,
    })
}

const CENTER: u32 = SIZE / 2;

#[test]
fn reference_guides_stay_behind_opaque_models_on_the_gpu() {
    with_gpu(|renderer| {
        let solid = Arc::new(cube(1.0));
        let guide = Arc::new(Mesh::lines(vec![[-2.0, 0.0, 2.0], [2.0, 0.0, 2.0]]));
        let guide_style = Style {
            face_color: [0.0; 4],
            edge_color: [0.0, 1.0, 0.0, 1.0],
            highlight: [0.0; 4],
            edge_width: 7.0,
            point_size: 1.0,
            faces: false,
            edges: true,
            points: false,
            clipped: false,
            background_fill: false,
            hidden_edges: false,
            edge_min_angle: 0.0,
            silhouettes: false,
            surface: [0.0; 4],
        };
        let mut camera = Camera {
            scene_radius: 4.0,
            ..Camera::default()
        };
        camera.set_pose(Pose {
            target: Vec3::ZERO,
            rotation: StandardView::Top.rotation(),
            extent: 2.0,
        });
        let render = |renderer: &mut SceneRenderer, guide_layer: Option<Layer>| {
            let mut items = vec![DrawItem {
                mesh: solid.clone(),
                transform: Mat4::IDENTITY,
                style: red(),
                layer: Layer::Scene,
            }];
            if let Some(layer) = guide_layer {
                items.push(DrawItem {
                    mesh: guide.clone(),
                    transform: Mat4::IDENTITY,
                    style: guide_style,
                    layer,
                });
            }
            renderer
                .render_pixels(&FrameInput {
                    camera: &camera,
                    size: [SIZE, SIZE],
                    background_top: BACKGROUND,
                    background_bottom: BACKGROUND,
                    items: &items,
                    section: None,
                    pixel_scale: 1.0,
                    eye: Eye::Center,
                    lighting: Lighting::default(),
                    ground_shadow: None,
                })
                .unwrap()
                .1
        };

        let model = render(renderer, None);
        let reference = render(renderer, Some(Layer::Reference));
        let scene_line = render(renderer, Some(Layer::Scene));
        let center = ((CENTER * SIZE + CENTER) * 4) as usize;
        let pixel = |frame: &[u8]| <[u8; 4]>::try_from(&frame[center..center + 4]).unwrap();

        assert_eq!(pixel(&reference), pixel(&model));
        assert_ne!(pixel(&scene_line), pixel(&model));
        assert!(
            pixel(&scene_line)[1] > pixel(&scene_line)[0],
            "the control line should be green and visible over the model"
        );
    });
}

#[test]
fn a_section_plane_cuts_the_model_and_fills_the_cut_on_the_gpu() {
    with_gpu(|renderer| {
        let top = |renderer: &mut SceneRenderer, style: Style, section: Option<Section>| {
            let frame = draw(renderer, StandardView::Top, style, section);
            (frame.at(CENTER, CENTER), frame.at(2, 2))
        };

        let (whole, outside) = top(renderer, red(), None);
        assert_eq!(outside, BACKGROUND_SRGB);
        assert!(
            whole[0] > 100 && whole[2] < 50,
            "the red top face: {whole:?}"
        );

        // Cut through the middle: the cap, flat and darker than a face, covers
        // the opening, and the stencil leaves nothing outside the cube.
        let (capped, outside) = top(renderer, red(), cut(0.0, true));
        assert_eq!(outside, BACKGROUND_SRGB);
        assert!(capped[0] > 50 && capped[2] < 50, "a red cap: {capped:?}");
        assert!(capped[0] < whole[0], "the cap is darker than the top face");

        // Without caps the inside of the bottom face shows through instead.
        let (open, _) = top(renderer, red(), cut(0.0, false));
        assert_ne!(open, capped);

        // A plane under the cube removes all of it, caps included.
        let (gone, _) = top(renderer, red(), cut(-5.0, true));
        assert_eq!(gone, BACKGROUND_SRGB);

        // Helpers that are not clipped stay whole.
        let helper = Style {
            clipped: false,
            ..red()
        };
        let (kept, _) = top(renderer, helper, cut(-5.0, true));
        assert_eq!(kept, whole);
    });
}

#[test]
fn display_modes_render_faces_edges_and_fills_as_documented() {
    with_gpu(|renderer| {
        use DisplayMode::{HiddenLine, Shaded, Wireframe, XRay};

        // Seen from (1, -1, 1), the near corner (1, -1, 1) and the hidden one
        // (-1, 1, -1) are both at the centre. The right face (+X) is a rhombus
        // to the right, cut along its diagonal by a hidden edge; (94, 62) is
        // inside it, away from every edge. Another hidden edge runs straight up
        // from the centre, behind the top face.
        let face = |frame: &Frame| frame.at(94, 62);
        let behind_top_face = |frame: &Frame| -> Vec<[u8; 4]> {
            (25..52)
                .flat_map(|y| [frame.at(CENTER - 1, y), frame.at(CENTER, y)])
                .collect()
        };
        let iso = |renderer: &mut SceneRenderer, style: Style| {
            draw(renderer, StandardView::Iso, style, None)
        };

        let shaded = iso(renderer, in_mode(Shaded));
        let lit = face(&shaded);
        assert!(lit[0] > 60 && lit[2] < 40, "a lit red face: {lit:?}");
        assert!(
            behind_top_face(&shaded)
                .iter()
                .all(|p| p[0] > 60 && p[2] < 40),
            "the top face hides the edge behind it"
        );

        let wireframe = iso(renderer, in_mode(Wireframe));
        assert_eq!(face(&wireframe), BACKGROUND_SRGB, "no faces");
        assert!(
            behind_top_face(&wireframe).iter().any(|p| p[2] < 200),
            "every edge, the hidden one included"
        );

        let xray = iso(renderer, in_mode(XRay));
        let see_through = face(&xray);
        assert!(
            see_through[0] > 10 && see_through[2] > 60 && see_through != BACKGROUND_SRGB,
            "red faces over the background: {see_through:?}"
        );
        assert!(see_through[0] < lit[0] || see_through[2] > lit[2]);
        // Through the red faces the dark edge takes out the blue background.
        let blue = |frame: &Frame, x: u32| {
            (25..52).map(|y| u32::from(frame.at(x, y)[2])).sum::<u32>() / 27
        };
        assert!(
            blue(&xray, CENTER) + 20 < blue(&xray, CENTER + 4),
            "the hidden edge shows through: {} vs {}",
            blue(&xray, CENTER),
            blue(&xray, CENTER + 4)
        );

        let hidden_line = iso(renderer, in_mode(HiddenLine));
        assert_eq!(
            face(&hidden_line),
            BACKGROUND_SRGB,
            "faces are filled with the background"
        );
        let dashes = behind_top_face(&hidden_line);
        assert!(
            dashes.iter().any(|p| *p != BACKGROUND_SRGB) && dashes.contains(&BACKGROUND_SRGB),
            "the hidden edge is dashed"
        );
        // The visible edge from the centre straight down is solid and dark.
        assert!((80..100).all(|y| hidden_line.at(CENTER, y)[2] < 200));

        let without_hidden = Style {
            hidden_edges: false,
            ..in_mode(HiddenLine)
        };
        let hidden_line = iso(renderer, without_hidden);
        assert!(
            behind_top_face(&hidden_line)
                .iter()
                .all(|p| *p == BACKGROUND_SRGB),
            "hidden edges removed"
        );

        // A section in hidden-line mode is hatched with the edge color.
        let section = draw(
            renderer,
            StandardView::Top,
            in_mode(HiddenLine),
            cut(0.0, true),
        );
        let mut hatch = Vec::new();
        for y in CENTER - 8..CENTER + 8 {
            for x in CENTER - 8..CENTER + 8 {
                hatch.push(section.at(x, y));
            }
        }
        assert!(hatch.contains(&BACKGROUND_SRGB), "between the hatch lines");
        assert!(
            hatch.iter().any(|p| p[2] < 60),
            "hatch lines in the edge color"
        );
    });
}

#[test]
fn the_lights_follow_the_lighting_settings() {
    with_gpu(|renderer| {
        // Seen from the iso view: the top face (+Z) at (64, 40), the right
        // face (+X) at (94, 62) and the left one (-Y) at (34, 62).
        let iso = |renderer: &mut SceneRenderer, lighting: Lighting| {
            let frame = draw_lit(
                renderer,
                StandardView::Iso,
                red(),
                None,
                SIZE,
                1.0,
                BACKGROUND,
                lighting,
            );
            [frame.at(64, 40), frame.at(94, 62), frame.at(34, 62)].map(|p| u32::from(p[0]))
        };
        let defaults = Lighting::default();
        let lit = iso(renderer, defaults);

        // Nothing but ambient light: every face the same, darker red.
        let flat = iso(
            renderer,
            Lighting {
                key: 0.0,
                fill: 0.0,
                rim: 0.0,
                specular: 0.0,
                ..defaults
            },
        );
        assert!(flat[0].abs_diff(flat[1]) <= 1 && flat[1].abs_diff(flat[2]) <= 1);
        assert!(
            flat.iter().zip(&lit).all(|(f, l)| f < l),
            "{flat:?} {lit:?}"
        );

        // One stop more light, or more contrast, changes what is lit.
        let exposed = iso(
            renderer,
            Lighting {
                exposure: 1.0,
                ..defaults
            },
        );
        assert!(exposed.iter().zip(&lit).all(|(e, l)| e > l || *l == 255));
        let contrasted = iso(
            renderer,
            Lighting {
                contrast: 2.0,
                ..defaults
            },
        );
        assert_ne!(contrasted, lit);

        // The key light moved to the right lights the right face more than
        // the left one, and the other way round.
        let from = |renderer: &mut SceneRenderer, azimuth: f32| {
            iso(
                renderer,
                Lighting {
                    azimuth,
                    elevation: 0.0,
                    fill: 0.0,
                    rim: 0.0,
                    specular: 0.0,
                    ..defaults
                },
            )
        };
        let (left, right) = (from(renderer, -60.0), from(renderer, 60.0));
        assert!(
            right[1] > right[2] && left[2] > left[1],
            "{left:?} {right:?}"
        );
    });
}

/// An open tube of radius 1 along Z, from -1 to 1, with smooth normals.
fn tube(sides: u32) -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    for side in 0..sides {
        let angle = side as f32 / sides as f32 * std::f32::consts::TAU;
        let (y, x) = angle.sin_cos();
        for z in [-1.0, 1.0] {
            positions.push([x, y, z]);
            normals.push([x, y, 0.0]);
        }
    }
    let mut indices = Vec::new();
    for side in 0..sides {
        let (a, b) = (side * 2, (side + 1) % sides * 2);
        indices.extend([a, b, b + 1, a, b + 1, a + 1]);
    }
    Mesh::new(positions, normals, indices, Vec::new(), Vec::new())
}

/// Renders `mesh` from `view` with `style` in a `SIZE` image, 32 pixels per
/// unit.
fn render_mesh(
    renderer: &mut SceneRenderer,
    mesh: Mesh,
    view: StandardView,
    style: Style,
) -> Frame {
    render_placed(renderer, mesh, Mat4::IDENTITY, view, style)
}

/// `render_mesh` with the mesh placed by `transform`.
fn render_placed(
    renderer: &mut SceneRenderer,
    mesh: Mesh,
    transform: Mat4,
    view: StandardView,
    style: Style,
) -> Frame {
    let mut camera = Camera {
        scene_radius: 2.0,
        ..Camera::default()
    };
    camera.set_pose(Pose {
        target: Vec3::ZERO,
        rotation: view.rotation(),
        extent: 2.0,
    });
    let items = [DrawItem {
        mesh: Arc::new(mesh),
        transform,
        style,
        layer: Layer::Scene,
    }];
    let (_, pixels) = renderer
        .render_pixels(&FrameInput {
            camera: &camera,
            size: [SIZE, SIZE],
            background_top: BACKGROUND,
            background_bottom: BACKGROUND,
            items: &items,
            section: None,
            pixel_scale: 1.0,
            eye: Eye::Center,
            lighting: Lighting::default(),
            ground_shadow: None,
        })
        .expect("the frame is read back");
    Frame { side: SIZE, pixels }
}

#[test]
fn edges_whose_faces_meet_smoothly_are_left_out_on_the_gpu() {
    with_gpu(|renderer| {
        // Two edges seen from the top: y = 1 between faces meeting smoothly
        // (0°), y = -1 at a right angle (90°).
        let lines = || {
            let mut mesh = Mesh::lines(vec![
                [-1.5, 1.0, 0.0],
                [1.5, 1.0, 0.0],
                [-1.5, -1.0, 0.0],
                [1.5, -1.0, 0.0],
            ]);
            mesh.segment_angles = vec![0.0, 90.0];
            mesh
        };
        let style = |edge_min_angle| Style {
            faces: false,
            edges: true,
            edge_width: 3.0,
            edge_min_angle,
            ..red()
        };
        // Rows 32 (y = 1) and 96 (y = -1) of the top view.
        let drawn = |frame: &Frame, row: u32| frame.at(CENTER, row) != BACKGROUND_SRGB;

        let every = render_mesh(renderer, lines(), StandardView::Top, style(0.0));
        assert!(drawn(&every, 32) && drawn(&every, 96), "every edge");
        let sharp = render_mesh(renderer, lines(), StandardView::Top, style(30.0));
        assert!(!drawn(&sharp, 32), "the smooth edge is left out");
        assert!(drawn(&sharp, 96), "the sharp edge stays");
    });
}

#[test]
fn curved_surfaces_get_an_outline_on_the_gpu() {
    with_gpu(|renderer| {
        // From the front the tube spans x = 32…96 and its outline is the
        // two vertical sides, where its surface turns away.
        let dark_columns = |frame: &Frame| -> Vec<u32> {
            (16..112)
                .filter(|&x| {
                    let p = frame.at(x, CENTER);
                    p[0] < 40 && p[2] < 40
                })
                .collect()
        };
        let style = |silhouettes| Style {
            edges: true,
            silhouettes,
            edge_width: 2.0,
            ..red()
        };
        let plain = render_mesh(renderer, tube(48), StandardView::Front, style(false));
        assert!(dark_columns(&plain).is_empty(), "no outline without it");

        let outlined = render_mesh(renderer, tube(48), StandardView::Front, style(true));
        let dark = dark_columns(&outlined);
        assert!(
            dark.iter().any(|&x| (29..=35).contains(&x)),
            "the left side: {dark:?}"
        );
        assert!(
            dark.iter().any(|&x| (93..=99).contains(&x)),
            "the right side: {dark:?}"
        );
        assert!(
            dark.iter().all(|&x| x <= 35 || x >= 93),
            "nothing across the surface: {dark:?}"
        );
        // Outlines follow edges: no edges, no outline.
        let hidden = render_mesh(
            renderer,
            tube(48),
            StandardView::Front,
            Style {
                edges: false,
                ..style(true)
            },
        );
        assert!(dark_columns(&hidden).is_empty());
    });
}

/// A ball of `radius` with smooth normals.
fn ball(radius: f32) -> Mesh {
    let (rings, segments) = (32u32, 64u32);
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    for ring in 0..=rings {
        let polar = std::f32::consts::PI * ring as f32 / rings as f32;
        for segment in 0..=segments {
            let azimuth = std::f32::consts::TAU * segment as f32 / segments as f32;
            let normal = [
                polar.sin() * azimuth.cos(),
                polar.sin() * azimuth.sin(),
                polar.cos(),
            ];
            positions.push(normal.map(|c| c * radius));
            normals.push(normal);
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
    Mesh::new(positions, normals, indices, Vec::new(), Vec::new())
}

#[test]
fn materials_change_how_faces_are_shaded_on_the_gpu() {
    use crate::settings::Finish;
    with_gpu(|renderer| {
        // A grey ball of radius 1.2 (38 px), or a 40-unit one drawn as
        // small, whose wood grain is in its own units.
        let grey = [0.3, 0.3, 0.3, 1.0];
        let shade = |renderer: &mut SceneRenderer, finish: Finish, big: bool| {
            let style = Style {
                face_color: grey,
                surface: finish.surface(),
                ..red()
            };
            let (mesh, scale) = if big {
                (ball(40.0), 0.03)
            } else {
                (ball(1.2), 1.0)
            };
            let frame = render_placed(
                renderer,
                mesh,
                Mat4::from_scale(Vec3::splat(scale)),
                StandardView::Front,
                style,
            );
            // The square inside the ball, row by row.
            (CENTER - 26..CENTER + 26)
                .flat_map(|y| (CENTER - 26..CENTER + 26).map(move |x| (x, y)))
                .map(|(x, y)| frame.at(x, y))
                .collect::<Vec<_>>()
        };
        let brightest = |pixels: &[[u8; 4]]| pixels.iter().map(|p| p[1]).max().unwrap();
        let unevenness = |row: &[[u8; 4]]| {
            row.windows(2)
                .map(|pair| u32::from(pair[0][1].abs_diff(pair[1][1])))
                .sum::<u32>()
        };

        let matte = shade(renderer, Finish::MattePlastic, false);
        let glossy = shade(renderer, Finish::GlossyPlastic, false);
        assert!(
            brightest(&glossy) > brightest(&matte) + 40,
            "a glossy highlight: {} vs {}",
            brightest(&glossy),
            brightest(&matte)
        );
        // A metal reflects the studio around it in its own color.
        let steel = shade(renderer, Finish::BrushedSteel, false);
        assert_ne!(steel, matte);
        // Default stays the plain shading.
        let plain = shade(renderer, Finish::Default, false);
        let red_style = in_mode(DisplayMode::Shaded);
        assert_eq!(red_style.surface, [0.0; 4]);
        assert_ne!(plain, matte);
        // Wood has a grain where matte plastic of its color is even.
        let wood = shade(renderer, Finish::Wood, true);
        let matte_big = shade(renderer, Finish::MattePlastic, true);
        assert!(
            unevenness(&wood) > 2 * unevenness(&matte_big) + 20,
            "a grain: {} vs {}",
            unevenness(&wood),
            unevenness(&matte_big)
        );
    });
}

#[test]
fn annotated_face_color_overrides_the_part_color_on_the_gpu() {
    with_gpu(|renderer| {
        let style = Style {
            face_color: [0.3, 0.3, 0.3, 1.0],
            ..red()
        };
        let plain = render_placed(
            renderer,
            cube(1.0),
            Mat4::IDENTITY,
            StandardView::Iso,
            style,
        );
        let mut painted = cube(1.0);
        painted.vertex_colors = vec![0xff00_00ff; painted.positions.len()];
        let colored = render_placed(renderer, painted, Mat4::IDENTITY, StandardView::Iso, style);
        // A single pixel at the center can land on a dark triangle edge on
        // software renderers. Check the visible face instead.
        let colored_face_pixels = plain
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .zip(colored.pixels.as_chunks::<4>().0)
            .filter(|(plain, colored)| {
                plain != colored && colored[0] > colored[1].saturating_add(50)
            })
            .count();
        let changed_pixels = plain
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .zip(colored.pixels.as_chunks::<4>().0)
            .filter(|(plain, colored)| plain != colored)
            .count();
        assert!(
            colored_face_pixels > 100,
            "{colored_face_pixels} colored face pixels; {changed_pixels} changed pixels"
        );
    });
}

#[test]
fn a_ground_shadow_darkens_the_floor_around_the_model_on_the_gpu() {
    use crate::viewer::shadow::GroundShadow;
    with_gpu(|renderer| {
        let grey = [0.7, 0.7, 0.7, 1.0];
        let render = |renderer: &mut SceneRenderer,
                      shadow: Option<GroundShadow>,
                      background: [f32; 4],
                      side: u32| {
            let mut camera = Camera {
                scene_radius: 3.0,
                ..Camera::default()
            };
            camera.set_pose(Pose {
                target: Vec3::ZERO,
                rotation: StandardView::Iso.rotation(),
                extent: 2.5,
            });
            let items = [DrawItem {
                mesh: Arc::new(cube(0.8)),
                transform: Mat4::IDENTITY,
                style: Style {
                    face_color: grey,
                    ..red()
                },
                layer: Layer::Scene,
            }];
            renderer
                .render_pixels(&FrameInput {
                    camera: &camera,
                    size: [side, side],
                    background_top: background,
                    background_bottom: background,
                    items: &items,
                    section: None,
                    pixel_scale: 1.0,
                    eye: Eye::Center,
                    lighting: Lighting::default(),
                    ground_shadow: shadow,
                })
                .unwrap()
                .1
        };
        let shadow = Some(GroundShadow {
            strength: 0.8,
            softness: 0.5,
        });
        let light = [0.9, 0.9, 0.9, 1.0];
        let plain = render(renderer, None, light, SIZE);
        let shaded = render(renderer, shadow, light, SIZE);
        let background = plain[0..4].to_vec();
        let pixels = |frame: &[u8]| frame.chunks(4).map(<[u8]>::to_vec).collect::<Vec<_>>();
        let (plain, shaded) = (pixels(&plain), pixels(&shaded));
        // Outside the cube, the floor darkens under and around it.
        let darkened = plain
            .iter()
            .zip(&shaded)
            .filter(|(p, s)| **p == background && s[0] < background[0] - 20)
            .count();
        assert!(darkened > 300, "{darkened} darkened floor pixels");
        // The cube is drawn as before, in front of its shadow: only the
        // smoothed pixels of its outline take some of the darker floor.
        let side = SIZE as usize;
        let inside = |index: usize| {
            let (x, y) = (index % side, index / side);
            (x.saturating_sub(2)..=(x + 2).min(side - 1)).all(|nx| {
                (y.saturating_sub(2)..=(y + 2).min(side - 1))
                    .all(|ny| plain[ny * side + nx] != background)
            })
        };
        let cube_changed = (0..plain.len())
            .filter(|&index| inside(index) && plain[index] != shaded[index])
            .count();
        assert_eq!(cube_changed, 0, "cube pixels changed");
        // The corners of the image stay background.
        assert_eq!(shaded[0], background);

        // On a transparent background the shadow has an alpha of its own,
        // to lay the picture on anything.
        let clear = render(renderer, shadow, [0.0; 4], SIZE);
        let translucent = pixels(&clear)
            .iter()
            .filter(|p| p[3] > 20 && p[3] < 255)
            .count();
        assert!(translucent > 300, "{translucent} shadow pixels");

        // A capture drawn in tiles has the same shadow as one pass.
        renderer.tile_side = SIZE / 2;
        let tiled = render(renderer, shadow, light, SIZE);
        renderer.tile_side = TILE_SIDE;
        let whole = render(renderer, shadow, light, SIZE);
        // Tiled projections may round one channel by one sRGB level on a
        // different GPU backend.
        assert_eq!(tiled.len(), whole.len());
        let changed_channels = tiled
            .iter()
            .zip(&whole)
            .filter(|(a, b)| a.abs_diff(**b) > 1)
            .count();
        assert_eq!(
            changed_channels, 0,
            "shadow tiles differ by more than one level"
        );
    });
}

#[test]
fn large_captures_are_drawn_in_tiles_that_match_one_pass() {
    with_gpu(|renderer| {
        let style = in_mode(DisplayMode::Shaded);
        let whole = draw(renderer, StandardView::Iso, style, None);
        // Tiles of 48 pixels: three by three, the last ones reaching past
        // the image.
        renderer.tile_side = 48;
        let tiled = draw(renderer, StandardView::Iso, style, None);
        let hatched = draw(
            renderer,
            StandardView::Top,
            in_mode(DisplayMode::HiddenLine),
            cut(0.0, true),
        );
        renderer.tile_side = TILE_SIDE;
        let hatched_whole = draw(
            renderer,
            StandardView::Top,
            in_mode(DisplayMode::HiddenLine),
            cut(0.0, true),
        );

        for (a, b) in [(&whole, &tiled), (&hatched_whole, &hatched)] {
            let differences = a
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .zip(b.pixels.as_chunks::<4>().0)
                .filter(|(a, b)| a.iter().zip(b.iter()).any(|(a, b)| a.abs_diff(*b) > 2))
                .count();
            assert!(
                differences <= (SIZE * SIZE / 500) as usize,
                "{differences} pixels differ between the tiled and the whole image"
            );
        }
    });
}

#[test]
fn captures_leave_out_a_transparent_background_and_scale_edges() {
    with_gpu(|renderer| {
        let style = in_mode(DisplayMode::Shaded);
        let transparent = draw_sized(
            renderer,
            StandardView::Iso,
            style,
            None,
            SIZE,
            1.0,
            [0.0; 4],
        );
        assert_eq!(transparent.at(2, 2), [0, 0, 0, 0], "no background");
        assert_eq!(transparent.at(94, 62)[3], 255, "an opaque face");
        let silhouette = transparent
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| (1..255).contains(&pixel[3]))
            .count();
        // Without multisampling a pixel is covered or not.
        if renderer.samples() > 1 {
            assert!(silhouette > 0, "antialiased edges are partly transparent");
        }

        // Twice the size with twice the width: the visible edge straight
        // down from the centre is twice as wide. Its width is measured as
        // its coverage over the faces on either side, in linear space, so
        // it does not depend on how the edge falls on the pixels.
        let edge_width = |frame: &Frame, y: u32| -> f32 {
            let linear = |c: u8| (f32::from(c) / 255.0).powf(2.2);
            let (center, reach) = (frame.side / 2, frame.side / 16);
            let face = |x: u32| linear(frame.at(x, y)[0]);
            let (left, right) = (face(center - reach), face(center + reach));
            (center - reach..center + reach)
                .map(|x| 1.0 - face(x) / if x < center { left } else { right })
                .sum()
        };
        let small = draw(renderer, StandardView::Iso, style, None);
        let large = draw_sized(
            renderer,
            StandardView::Iso,
            style,
            None,
            SIZE * 2,
            2.0,
            BACKGROUND,
        );
        let (thin, thick) = (edge_width(&small, 90), edge_width(&large, 180));
        // Whole pixels without multisampling: 2 for 1.5, 3 or 4 for 3.
        let (expected, tolerance) = if renderer.samples() > 1 {
            ((1.5, 0.4), 0.25)
        } else {
            ((2.0, 0.6), 0.6)
        };
        assert!(
            (thin - expected.0).abs() < expected.1,
            "a 1.5 pixel edge: {thin}"
        );
        assert!(
            (thick / thin - 2.0).abs() < tolerance,
            "{thin} then {thick}"
        );
    });
}

#[test]
fn parts_out_of_view_are_not_drawn() {
    with_gpu(|renderer| {
        let mesh = Arc::new(cube(1.0));
        let mut camera = Camera {
            scene_radius: 2.0,
            ..Camera::default()
        };
        camera.set_pose(Pose {
            target: Vec3::ZERO,
            rotation: StandardView::Iso.rotation(),
            extent: 2.0,
        });
        let at = |offset: Vec3| DrawItem {
            mesh: mesh.clone(),
            transform: Mat4::from_translation(offset),
            style: in_mode(DisplayMode::Shaded),
            layer: Layer::Scene,
        };
        let mut render = |items: &[DrawItem]| {
            let pixels = renderer
                .render_pixels(&FrameInput {
                    camera: &camera,
                    size: [SIZE, SIZE],
                    background_top: BACKGROUND,
                    background_bottom: BACKGROUND,
                    items,
                    section: None,
                    pixel_scale: 1.0,
                    eye: Eye::Center,
                    lighting: Lighting::default(),
                    ground_shadow: None,
                })
                .unwrap()
                .1;
            (pixels, renderer.last_stats())
        };
        let (alone, stats) = render(&[at(Vec3::ZERO)]);
        assert_eq!(
            stats,
            RenderStats {
                drawn: 1,
                culled: 0
            }
        );
        let far = [
            at(Vec3::ZERO),
            at(Vec3::new(100.0, 0.0, 0.0)),
            at(Vec3::new(0.0, -50.0, 30.0)),
        ];
        let (with_far, stats) = render(&far);
        assert_eq!(
            stats,
            RenderStats {
                drawn: 1,
                culled: 2
            }
        );
        assert!(alone == with_far, "skipping them changes nothing on screen");
    });
}

#[test]
fn stereo_modes_compose_on_the_gpu_as_the_reference_does() {
    with_gpu(|renderer| {
        let mesh = Arc::new(cube(1.0));
        let mut camera = Camera {
            projection: crate::settings::Projection::Stereo,
            scene_radius: 2.0,
            ..Camera::default()
        };
        camera.set_pose(Pose {
            target: Vec3::ZERO,
            rotation: StandardView::Iso.rotation(),
            extent: 2.0,
        });
        let items = [DrawItem {
            mesh,
            transform: Mat4::IDENTITY,
            style: in_mode(DisplayMode::Shaded),
            layer: Layer::Scene,
        }];
        // An odd width, so that the halves of side by side differ.
        let input = FrameInput {
            camera: &camera,
            size: [SIZE + 1, SIZE],
            background_top: BACKGROUND,
            background_bottom: BACKGROUND,
            items: &items,
            section: None,
            pixel_scale: 1.0,
            eye: Eye::Center,
            lighting: Lighting::default(),
            ground_shadow: None,
        };

        // The eyes see the cube from places 5% of the distance apart.
        let eye = |renderer: &mut SceneRenderer, eye| {
            renderer
                .render_pixels(&FrameInput { eye, ..input })
                .unwrap()
                .1
        };
        let (left, right) = (eye(renderer, Eye::Left), eye(renderer, Eye::Right));
        let differing = left
            .as_chunks::<4>()
            .0
            .iter()
            .zip(right.as_chunks::<4>().0)
            .filter(|(a, b)| a != b)
            .count();
        assert!(differing > 50, "{differing} pixels differ between the eyes");

        for mode in crate::settings::StereoMode::ALL {
            let (size, gpu) = renderer.render_stereo_gpu_pixels(&input, mode).unwrap();
            let (reference_size, reference) = renderer.render_stereo_pixels(&input, mode).unwrap();
            assert_eq!(size, reference_size);
            let worst = gpu
                .iter()
                .zip(&reference)
                .map(|(a, b)| a.abs_diff(*b))
                .max()
                .unwrap();
            // Choosing texels is exact; mixing (the anaglyph) rounds.
            let tolerance = if mode.shown() == crate::settings::StereoMode::Anaglyph {
                1
            } else {
                0
            };
            assert!(worst <= tolerance, "{mode:?}: off by {worst}");
        }
    });
}

#[test]
fn egui_gets_the_same_texels_where_textures_cannot_be_viewed_as_plain() {
    // OpenGL cannot view the sRGB target as plain 8-bit values, so egui is
    // given a copy encoded by hand; it must hold exactly the same bytes.
    with_gpu(|renderer| {
        let mesh = Arc::new(cube(1.0));
        let mut camera = Camera {
            scene_radius: 2.0,
            ..Camera::default()
        };
        camera.set_pose(Pose {
            target: Vec3::ZERO,
            rotation: StandardView::Iso.rotation(),
            extent: 2.0,
        });
        let items = [DrawItem {
            mesh,
            transform: Mat4::IDENTITY,
            style: Style {
                face_color: [1.0, 0.0, 0.0, 0.6],
                ..in_mode(DisplayMode::Shaded)
            },
            layer: Layer::Scene,
        }];
        let (srgb, encoded) = renderer
            .presented_pixels(&FrameInput {
                camera: &camera,
                size: [SIZE, SIZE],
                // A gradient, so many levels go through the encoding.
                background_top: [1.0, 0.5, 0.0, 1.0],
                background_bottom: [0.0, 0.1, 0.9, 0.5],
                items: &items,
                section: None,
                pixel_scale: 1.0,
                eye: Eye::Center,
                lighting: Lighting::default(),
                ground_shadow: None,
            })
            .unwrap();
        assert_eq!(srgb.len(), encoded.len());
        let worst = srgb
            .iter()
            .zip(&encoded)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap();
        assert_eq!(worst, 0, "the encoded copy differs by up to {worst}");
    });
}
