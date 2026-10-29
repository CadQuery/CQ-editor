//! GPU renderer of the 3D view.
//!
//! The scene is drawn off screen (4x MSAA when available) into an sRGB
//! texture that egui then shows like an image. egui samples textures as
//! gamma-encoded values, so it is handed a non-sRGB view of the same
//! texture, or, on devices that cannot view a texture in another format
//! (the OpenGL backend), a plain copy encoded by `present.wgsl`.
//!
//! Draw order: background, opaque faces (each followed by its section cap),
//! edges and points, hidden edges (hidden-line mode), transparent faces
//! (back to front, with their caps), then the inspector overlay, which is
//! drawn on top.

use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::{Arc, Weak};
use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use eframe::egui_wgpu::{self, wgpu};
use glam::{Mat4, Vec2, Vec3, Vec4};
use wgpu::util::DeviceExt as _;

use super::camera::{Camera, Eye};
use super::culling::Frustum;
use super::shadow::{GroundShadow, ShadowPass};
use super::stereo;
use crate::scene::Mesh;
use crate::settings::{Lighting, StereoMode};

const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
/// Same texels as `COLOR_FORMAT`, read without sRGB decoding (for egui).
const EGUI_VIEW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// With a stencil, for the caps of section planes.
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24PlusStencil8;
const PREFERRED_SAMPLES: u32 = 4;
/// Captures larger than this, on either side, are drawn in tiles.
const TILE_SIDE: u32 = 2048;
const OBJECT_UNIFORM_SIZE: u64 = std::mem::size_of::<ObjectUniform>() as u64;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
    eye: [f32; 4],
    forward: [f32; 4],
    right: [f32; 4],
    up: [f32; 4],
    viewport: [f32; 4],
    background_top: [f32; 4],
    background_bottom: [f32; 4],
    clip_plane: [f32; 4],
    section: [f32; 4],
    cap_corners: [[f32; 4]; 4],
    frame: [f32; 4],
    key_light: [f32; 4],
    light_levels: [f32; 4],
    finish: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ObjectUniform {
    model: [[f32; 4]; 4],
    face_color: [f32; 4],
    edge_color: [f32; 4],
    highlight: [f32; 4],
    params: [f32; 4],
    edge_filter: [f32; 4],
    surface: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct MeshVertex {
    position: [f32; 3],
    normal: [f32; 3],
    face_color: u32,
}

/// How one mesh is drawn. Colors are linear RGBA.
#[derive(Clone, Copy, Debug)]
pub struct Style {
    pub face_color: [f32; 4],
    pub edge_color: [f32; 4],
    /// RGB tint mixed into faces, with the mix factor in alpha.
    pub highlight: [f32; 4],
    pub edge_width: f32,
    pub point_size: f32,
    pub faces: bool,
    pub edges: bool,
    pub points: bool,
    /// The section plane cuts it (helpers such as the axes are not cut).
    pub clipped: bool,
    /// Faces are filled with the background instead of being lit, so they
    /// only hide what is behind them (hidden-line mode).
    pub background_fill: bool,
    /// Edges hidden behind faces are drawn too, as faint dashes.
    pub hidden_edges: bool,
    /// Edges whose faces meet at a smaller angle (degrees) are left out; 0
    /// draws every edge (see [`crate::scene::Mesh::segment_angles`]).
    pub edge_min_angle: f32,
    /// The outline of curved surfaces is drawn too, with the edges (see
    /// [`crate::scene::Mesh::silhouette_edges`]).
    pub silhouettes: bool,
    /// How faces are shaded: [`crate::settings::Finish::surface`].
    pub surface: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    /// Reference guides drawn behind the model without writing depth. Opaque
    /// scene geometry therefore always covers axes and ground grids.
    Reference,
    Scene,
    /// Drawn after the scene, faces pulled towards the viewer and edges and
    /// points always visible.
    Overlay,
}

pub struct DrawItem {
    pub mesh: Arc<Mesh>,
    pub transform: Mat4,
    pub style: Style,
    pub layer: Layer,
}

/// A section plane as the renderer draws it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SectionPlane {
    /// Unit normal; everything with `normal · p > offset` is removed.
    pub normal: Vec3,
    pub offset: f32,
    /// Fill the cut faces of solids.
    pub caps: bool,
    /// A quad on the plane around the whole scene, for the caps.
    pub cap_corners: [Vec3; 4],
}

/// Everything needed to draw one frame.
#[derive(Clone, Copy)]
pub struct FrameInput<'a> {
    pub camera: &'a Camera,
    /// Physical pixels.
    pub size: [u32; 2],
    /// Linear RGBA.
    pub background_top: [f32; 4],
    pub background_bottom: [f32; 4],
    pub items: &'a [DrawItem],
    pub section: Option<SectionPlane>,
    /// Multiplies edge widths and point sizes, so that a capture larger
    /// than the view looks like the view.
    pub pixel_scale: f32,
    /// Seen from the camera, or from one eye of the stereo pair.
    pub eye: Eye,
    pub lighting: Lighting,
    /// A soft shadow on the ground under the model (Presentation mode).
    pub ground_shadow: Option<GroundShadow>,
}

/// Mode and eye size of the stereo composition (`stereo.wgsl`).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ComposeUniform {
    mode: u32,
    eye_width: u32,
    eye_height: u32,
    padding: u32,
}

/// What composes the two eyes of the stereo projection.
struct Compose {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    uniform: wgpu::Buffer,
}

/// The part of an image one pass draws: `size` pixels at `offset` in an
/// image of `image` pixels. Large captures are drawn tile by tile.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Tile {
    offset: [u32; 2],
    size: [u32; 2],
    image: [u32; 2],
}

impl Tile {
    fn whole(size: [u32; 2]) -> Self {
        Self {
            offset: [0, 0],
            size,
            image: size,
        }
    }

    /// Maps the clip space of the whole image to this tile's: a scale and
    /// a shift after the projection, so it works for both projections and
    /// keeps screen-space sizes in pixels.
    fn projection(&self) -> Mat4 {
        let [width, height] = self.image.map(|v| v as f32);
        let [tile_width, tile_height] = self.size.map(|v| v as f32);
        let [x, y] = self.offset.map(|v| v as f32);
        let (scale_x, scale_y) = (width / tile_width, height / tile_height);
        // The tile's centre in normalized device coordinates (y up).
        let center_x = (x + tile_width / 2.0) / width * 2.0 - 1.0;
        let center_y = 1.0 - (y + tile_height / 2.0) / height * 2.0;
        Mat4::from_cols(
            Vec4::new(scale_x, 0.0, 0.0, 0.0),
            Vec4::new(0.0, scale_y, 0.0, 0.0),
            Vec4::Z,
            Vec4::new(-center_x * scale_x, -center_y * scale_y, 0.0, 1.0),
        )
    }
}

struct GpuMesh {
    source: Weak<Mesh>,
    vertices: Option<wgpu::Buffer>,
    indices: Option<wgpu::Buffer>,
    index_count: u32,
    segments: Option<wgpu::Buffer>,
    segment_count: u32,
    points: Option<wgpu::Buffer>,
    point_count: u32,
    /// Made when an outline is first drawn.
    silhouettes: std::cell::OnceCell<Option<(wgpu::Buffer, u32)>>,
}

struct Target {
    size: [u32; 2],
    multisampled: Option<wgpu::TextureView>,
    resolved: wgpu::Texture,
    resolved_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
    /// What egui shows; only the view's own target has it.
    egui: Option<EguiTexture>,
}

/// The view's image as egui reads it: gamma-encoded 8-bit values.
enum EguiTexture {
    /// A plain view of the sRGB texture: the same texels, not decoded.
    View(wgpu::TextureView),
    /// For devices without `DownlevelFlags::VIEW_FORMATS` (OpenGL): a
    /// plain texture that `present.wgsl` fills after each frame.
    Encoded {
        view: wgpu::TextureView,
        bind_group: wgpu::BindGroup,
    },
}

impl Target {
    fn egui_view(&self) -> &wgpu::TextureView {
        match self.egui.as_ref().expect("the view's target") {
            EguiTexture::View(view) | EguiTexture::Encoded { view, .. } => view,
        }
    }
}

/// Fills an `EguiTexture::Encoded` (`present.wgsl`).
struct Present {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
}

struct Pipelines {
    background: wgpu::RenderPipeline,
    reference_edges: wgpu::RenderPipeline,
    faces: wgpu::RenderPipeline,
    transparent_faces: wgpu::RenderPipeline,
    /// Inverts the stencil per remaining surface of a clipped object.
    section_count: wgpu::RenderPipeline,
    /// Fills the plane where the stencil is set, and clears it.
    caps: wgpu::RenderPipeline,
    transparent_caps: wgpu::RenderPipeline,
    overlay_faces: wgpu::RenderPipeline,
    edges: wgpu::RenderPipeline,
    /// Edges where the depth test fails, dashed (hidden-line mode).
    hidden_edges: wgpu::RenderPipeline,
    silhouettes: wgpu::RenderPipeline,
    overlay_edges: wgpu::RenderPipeline,
    points: wgpu::RenderPipeline,
    overlay_points: wgpu::RenderPipeline,
}

pub struct SceneRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    samples: u32,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    object_layout: wgpu::BindGroupLayout,
    object_buffer: wgpu::Buffer,
    object_bind_group: wgpu::BindGroup,
    object_capacity: usize,
    object_stride: u64,
    pipelines: Pipelines,
    meshes: HashMap<u64, GpuMesh>,
    target: Option<Target>,
    texture_id: Option<egui::TextureId>,
    /// Largest side of one pass of `render_pixels`.
    tile_side: u32,
    stats: std::cell::Cell<RenderStats>,
    compose: Compose,
    /// What the eyes of the stereo projection are drawn into.
    eye_targets: Option<[Target; 2]>,
    present: Present,
    /// The device can view the sRGB target as plain for egui.
    view_formats: bool,
    /// Made when a ground shadow is first asked for.
    shadow: Option<ShadowPass>,
    /// The frame being drawn has a ground shadow ready.
    shadow_shown: bool,
}

/// Items of the last pass drawn, and skipped as out of view.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderStats {
    pub drawn: usize,
    pub culled: usize,
}

impl SceneRenderer {
    pub fn new(render_state: &egui_wgpu::RenderState) -> Self {
        Self::with_device(
            render_state.device.clone(),
            render_state.queue.clone(),
            &render_state.adapter,
        )
    }

    /// A renderer on any device, including one without a window.
    pub fn with_device(device: wgpu::Device, queue: wgpu::Queue, adapter: &wgpu::Adapter) -> Self {
        // Both attachments must support the sample count (software
        // OpenGL, for one, may not).
        let multisampled = [COLOR_FORMAT, DEPTH_FORMAT].into_iter().all(|format| {
            adapter
                .get_texture_format_features(format)
                .flags
                .sample_count_supported(PREFERRED_SAMPLES)
        });
        let samples = if multisampled { PREFERRED_SAMPLES } else { 1 };

        let shader = device.create_shader_module(wgpu::include_wgsl!("shaders.wgsl"));

        let camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let object_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("object"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: NonZeroU64::new(OBJECT_UNIFORM_SIZE),
                },
                count: None,
            }],
        });

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera"),
            layout: &camera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let alignment = u64::from(device.limits().min_uniform_buffer_offset_alignment);
        let object_stride = OBJECT_UNIFORM_SIZE.div_ceil(alignment) * alignment;
        let object_capacity = 64;
        let (object_buffer, object_bind_group) =
            create_object_buffer(&device, &object_layout, object_stride, object_capacity);

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene"),
            bind_group_layouts: &[Some(&camera_layout), Some(&object_layout)],
            immediate_size: 0,
        });
        let pipelines = Pipelines::new(&device, &layout, &shader, samples);
        let compose = Compose::new(&device);
        let present = Present::new(&device);
        let view_formats = adapter
            .get_downlevel_capabilities()
            .flags
            .contains(wgpu::DownlevelFlags::VIEW_FORMATS);

        Self {
            device,
            queue,
            samples,
            camera_buffer,
            camera_bind_group,
            object_layout,
            object_buffer,
            object_bind_group,
            object_capacity,
            object_stride,
            pipelines,
            meshes: HashMap::new(),
            target: None,
            texture_id: None,
            tile_side: TILE_SIDE,
            stats: Default::default(),
            compose,
            eye_targets: None,
            present,
            view_formats,
            shadow: None,
            shadow_shown: false,
        }
    }

    /// Draws the ground shadow's occlusion again when the frame asks for a
    /// shadow and its items or settings changed (see `shadow.rs`).
    fn update_shadow(&mut self, input: &FrameInput<'_>) {
        self.shadow_shown = false;
        let Some(ground) = input.ground_shadow else {
            return;
        };
        let mut shadow = self.shadow.take().unwrap_or_else(|| {
            ShadowPass::new(
                &self.device,
                &self.object_layout,
                &self.camera_buffer,
                COLOR_FORMAT,
                DEPTH_FORMAT,
                self.samples,
            )
        });
        let shown = shadow.update(
            &self.device,
            &self.queue,
            input.items,
            &ground,
            |pass, index| self.draw_occluder(pass, input, index),
        );
        self.shadow = Some(shadow);
        self.shadow_shown = shown;
    }

    /// The faces of an item in the shadow's occlusion pass, whose pipeline
    /// is set.
    fn draw_occluder(&self, pass: &mut wgpu::RenderPass<'_>, input: &FrameInput<'_>, index: usize) {
        let item = &input.items[index];
        let Some(mesh) = self.meshes.get(&item.mesh.id) else {
            return;
        };
        let (Some(vertices), Some(indices)) = (&mesh.vertices, &mesh.indices) else {
            return;
        };
        pass.set_bind_group(1, &self.object_bind_group, &[self.object_offset(index)]);
        pass.set_vertex_buffer(0, vertices.slice(..));
        pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..mesh.index_count, 0, 0..1);
    }

    /// Draws a frame and returns the egui texture showing it.
    pub fn render(
        &mut self,
        egui_renderer: &mut egui_wgpu::Renderer,
        input: &FrameInput<'_>,
    ) -> egui::TextureId {
        let size = [input.size[0].max(1), input.size[1].max(1)];
        let texture_id = self.ensure_target(egui_renderer, size);
        self.upload_meshes(input);
        self.write_objects(input.items, input.pixel_scale);
        self.update_shadow(input);
        let view_projection = self.write_camera(input, Tile::whole(size));
        let target = self.target.as_ref().expect("target created above");
        self.draw(target, input, view_projection);
        self.present(target);
        texture_id
    }

    /// Draws a frame into textures of its own and reads it back as tightly
    /// packed sRGB RGBA rows, so its size does not depend on the view or on
    /// the largest texture of the device: large images are drawn in tiles.
    /// `None` when the read-back fails.
    pub fn render_pixels(&mut self, input: &FrameInput<'_>) -> Option<([u32; 2], Vec<u8>)> {
        let [width, height] = [input.size[0].max(1), input.size[1].max(1)];
        let side = self
            .tile_side
            .min(self.device.limits().max_texture_dimension_2d);
        let tile_size = [width.min(side), height.min(side)];
        let target = create_target(&self.device, tile_size, self.samples);
        self.upload_meshes(input);
        self.write_objects(input.items, input.pixel_scale);
        self.update_shadow(input);
        let row_bytes = width as usize * 4;
        let mut image = vec![0u8; row_bytes * height as usize];
        for y in (0..height).step_by(tile_size[1] as usize) {
            for x in (0..width).step_by(tile_size[0] as usize) {
                // Edge tiles reach past the image; that part is dropped.
                let view_projection = self.write_camera(
                    input,
                    Tile {
                        offset: [x, y],
                        size: tile_size,
                        image: [width, height],
                    },
                );
                self.draw(&target, input, view_projection);
                let (_, pixels) = self.read_target(&target)?;
                let tile_row = tile_size[0] as usize * 4;
                let columns = (width - x).min(tile_size[0]) as usize * 4;
                for row in 0..(height - y).min(tile_size[1]) as usize {
                    let from = row * tile_row;
                    let to = (y as usize + row) * row_bytes + x as usize * 4;
                    image[to..to + columns].copy_from_slice(&pixels[from..from + columns]);
                }
            }
        }
        Some(([width, height], image))
    }

    /// Draws both eyes of the stereo projection, composes them on the GPU
    /// as `mode` says (`StereoMode::shown`) and returns the egui texture.
    pub fn render_stereo(
        &mut self,
        egui_renderer: &mut egui_wgpu::Renderer,
        input: &FrameInput<'_>,
        mode: StereoMode,
    ) -> egui::TextureId {
        let size = [input.size[0].max(1), input.size[1].max(1)];
        let texture_id = self.ensure_target(egui_renderer, size);
        self.draw_eyes(input, mode);
        let target = self.target.as_ref().expect("target created above");
        self.compose_eyes(mode, &target.resolved_view);
        self.present(target);
        texture_id
    }

    /// The stereo image as [`SceneRenderer::render_pixels`] reads a plain
    /// one: each eye drawn by itself (in tiles when large), then composed
    /// by `stereo::compose`.
    pub fn render_stereo_pixels(
        &mut self,
        input: &FrameInput<'_>,
        mode: StereoMode,
    ) -> Option<([u32; 2], Vec<u8>)> {
        let size = [input.size[0].max(1), input.size[1].max(1)];
        let eye_size = stereo::eye_size(mode, size);
        let mut eye = |eye| {
            self.render_pixels(&FrameInput {
                size: eye_size,
                eye,
                ..*input
            })
            .map(|(_, pixels)| pixels)
        };
        let (left, right) = (eye(Eye::Left)?, eye(Eye::Right)?);
        Some((size, stereo::compose(mode, size, &left, &right)))
    }

    /// The live view's GPU composition, read back (for the tests).
    #[cfg(test)]
    fn render_stereo_gpu_pixels(
        &mut self,
        input: &FrameInput<'_>,
        mode: StereoMode,
    ) -> Option<([u32; 2], Vec<u8>)> {
        let size = [input.size[0].max(1), input.size[1].max(1)];
        let output = create_target(&self.device, size, self.samples);
        self.draw_eyes(input, mode);
        self.compose_eyes(mode, &output.resolved_view);
        self.read_target(&output)
    }

    /// Draws the left and the right eye into `eye_targets`.
    fn draw_eyes(&mut self, input: &FrameInput<'_>, mode: StereoMode) {
        let size = [input.size[0].max(1), input.size[1].max(1)];
        let eye_size = stereo::eye_size(mode, size);
        if self
            .eye_targets
            .as_ref()
            .is_none_or(|targets| targets[0].size != eye_size)
        {
            let target = || create_target(&self.device, eye_size, self.samples);
            self.eye_targets = Some([target(), target()]);
        }
        self.upload_meshes(input);
        self.write_objects(input.items, input.pixel_scale);
        self.update_shadow(input);
        let targets = self.eye_targets.as_ref().expect("created above");
        for (eye, target) in [Eye::Left, Eye::Right].into_iter().zip(targets) {
            let input = FrameInput {
                size: eye_size,
                eye,
                ..*input
            };
            let view_projection = self.write_camera(&input, Tile::whole(eye_size));
            self.draw(target, &input, view_projection);
        }
    }

    /// Composes the eyes drawn by `draw_eyes` into `output`.
    fn compose_eyes(&self, mode: StereoMode, output: &wgpu::TextureView) {
        let targets = self.eye_targets.as_ref().expect("eyes drawn first");
        let [eye_width, eye_height] = targets[0].size;
        let uniform = ComposeUniform {
            mode: stereo::shader_mode(mode),
            eye_width,
            eye_height,
            padding: 0,
        };
        self.queue
            .write_buffer(&self.compose.uniform, 0, bytemuck::bytes_of(&uniform));
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("stereo"),
            layout: &self.compose.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&targets[0].resolved_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&targets[1].resolved_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.compose.uniform.as_entire_binding(),
                },
            ],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("stereo"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("stereo"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: output,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.compose.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([encoder.finish()]);
    }

    fn upload_meshes(&mut self, input: &FrameInput<'_>) {
        self.meshes.retain(|_, mesh| mesh.source.strong_count() > 0);
        let uploading = Instant::now();
        let (mut uploaded, mut bytes) = (0, 0);
        for item in input.items {
            if !self.meshes.contains_key(&item.mesh.id) {
                let gpu = upload(&self.device, &item.mesh);
                self.meshes.insert(item.mesh.id, gpu);
                uploaded += 1;
                bytes += item.mesh.byte_size();
            }
        }
        if uploaded > 0 {
            log::debug!(
                "Uploaded {uploaded} mesh(es) to the GPU ({:.2} MB of mesh data) in {:.1} ms",
                bytes as f64 / 1e6,
                uploading.elapsed().as_secs_f64() * 1e3
            );
        }
    }

    /// What the last pass drew, and what it skipped as out of view.
    #[cfg(test)]
    pub fn last_stats(&self) -> RenderStats {
        self.stats.get()
    }

    /// Samples per pixel: 4 with multisampling, else 1.
    #[cfg(test)]
    fn samples(&self) -> u32 {
        self.samples
    }

    fn draw(&self, target: &Target, input: &FrameInput<'_>, view_projection: Mat4) {
        // Items wholly outside the view are skipped, faces, edges and caps,
        // allowing for the widest edge or point drawn across the border.
        let widest = input
            .items
            .iter()
            .map(|item| item.style.edge_width.max(item.style.point_size))
            .fold(0.0, f32::max);
        let margin_px = widest * input.pixel_scale + 2.0;
        let margin = Vec2::from_array(target.size.map(|side| margin_px * 2.0 / side as f32));
        let frustum = Frustum::new(view_projection, margin);
        let in_view: Vec<bool> = input
            .items
            .iter()
            .map(|item| frustum.intersects(item.mesh.bounds.transformed(item.transform)))
            .collect();
        let drawn = in_view.iter().filter(|&&shown| shown).count();
        self.stats.set(RenderStats {
            drawn,
            culled: input.items.len() - drawn,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("scene"),
            });
        {
            let (view, resolve_target) = match &target.multisampled {
                Some(multisampled) => (multisampled, Some(&target.resolved_view)),
                None => (&target.resolved_view, None),
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &target.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    // The section caps count surfaces in the stencil.
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0),
                        store: wgpu::StoreOp::Discard,
                    }),
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_stencil_reference(0);

            pass.set_pipeline(&self.pipelines.background);
            pass.set_bind_group(1, &self.object_bind_group, &[0]);
            pass.draw(0..3, 0..1);

            let is_opaque = |item: &DrawItem| item.style.face_color[3] >= 0.999;
            let order = draw_order(input);
            let in_view = &in_view;
            let scene = |pass_layer: Layer| {
                order
                    .iter()
                    .copied()
                    .filter(move |&(index, layer)| layer == pass_layer && in_view[index])
            };

            // Reference guides are deliberately painted before scene geometry
            // and do not write depth. This keeps axes and grids visible around
            // a model without making them look as if they pass through it.
            for (index, _) in scene(Layer::Reference) {
                self.draw_edges(&mut pass, &self.pipelines.reference_edges, input, index);
            }

            let caps = input.section.is_some_and(|section| section.caps);
            for (index, _) in scene(Layer::Scene).filter(|&(i, _)| is_opaque(&input.items[i])) {
                self.draw_faces(&mut pass, &self.pipelines.faces, input, index);
                if caps {
                    self.draw_cap(&mut pass, &self.pipelines.caps, input, index);
                }
            }
            // The ground shadow, behind the opaque faces drawn so far.
            if self.shadow_shown
                && let Some(shadow) = &self.shadow
            {
                shadow.draw_floor(&mut pass);
                pass.set_bind_group(0, &self.camera_bind_group, &[]);
            }
            for (index, _) in scene(Layer::Scene) {
                self.draw_edges(&mut pass, &self.pipelines.edges, input, index);
                self.draw_silhouettes(&mut pass, input, index);
                self.draw_points(&mut pass, &self.pipelines.points, input, index);
            }
            // After every opaque face, so that they all hide edges.
            for (index, _) in
                scene(Layer::Scene).filter(|&(i, _)| input.items[i].style.hidden_edges)
            {
                self.draw_edges(&mut pass, &self.pipelines.hidden_edges, input, index);
            }
            for (index, _) in scene(Layer::Scene).filter(|&(i, _)| !is_opaque(&input.items[i])) {
                self.draw_faces(&mut pass, &self.pipelines.transparent_faces, input, index);
                if caps {
                    self.draw_cap(&mut pass, &self.pipelines.transparent_caps, input, index);
                }
            }
            for (index, _) in scene(Layer::Overlay) {
                self.draw_faces(&mut pass, &self.pipelines.overlay_faces, input, index);
                self.draw_edges(&mut pass, &self.pipelines.overlay_edges, input, index);
                self.draw_points(&mut pass, &self.pipelines.overlay_points, input, index);
            }
        }
        self.queue.submit([encoder.finish()]);
    }

    /// Reads a target back as tightly packed sRGB RGBA rows.
    fn read_target(&self, target: &Target) -> Option<([u32; 2], Vec<u8>)> {
        self.read_texture(&target.resolved, target.size)
    }

    /// The view's target drawn, as the sRGB texture and as egui's plain
    /// copy encoded by `present.wgsl` (for the tests, on any device).
    #[cfg(test)]
    fn presented_pixels(&mut self, input: &FrameInput<'_>) -> Option<(Vec<u8>, Vec<u8>)> {
        let size = [input.size[0].max(1), input.size[1].max(1)];
        let target = create_view_target(&self.device, &self.present, size, self.samples, false);
        self.upload_meshes(input);
        self.write_objects(input.items, input.pixel_scale);
        self.update_shadow(input);
        let view_projection = self.write_camera(input, Tile::whole(size));
        self.draw(&target, input, view_projection);
        self.present(&target);
        let Some(EguiTexture::Encoded { view, .. }) = &target.egui else {
            unreachable!("created encoded");
        };
        Some((
            self.read_target(&target)?.1,
            self.read_texture(view.texture(), size)?.1,
        ))
    }

    /// Fills egui's plain copy of `target`, when it has one.
    fn present(&self, target: &Target) {
        let Some(EguiTexture::Encoded {
            view, bind_group, ..
        }) = &target.egui
        else {
            return;
        };
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("present"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("present"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.present.pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([encoder.finish()]);
    }

    /// Reads a color texture back as tightly packed RGBA rows.
    fn read_texture(&self, texture: &wgpu::Texture, size: [u32; 2]) -> Option<([u32; 2], Vec<u8>)> {
        let [width, height] = size;
        let row_bytes = width * 4;
        let padded_row = row_bytes.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("screenshot"),
            size: u64::from(padded_row) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("screenshot"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);

        let slice = buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(std::time::Duration::from_secs(5)),
            })
            .ok()?;
        receiver.recv().ok()?.ok()?;

        let mapped = slice.get_mapped_range().ok()?;
        let mut pixels = Vec::with_capacity((row_bytes * height) as usize);
        for row in mapped.chunks(padded_row as usize) {
            pixels.extend_from_slice(&row[..row_bytes as usize]);
        }
        drop(mapped);
        buffer.unmap();
        Some(([width, height], pixels))
    }

    fn ensure_target(
        &mut self,
        egui_renderer: &mut egui_wgpu::Renderer,
        size: [u32; 2],
    ) -> egui::TextureId {
        if self.target.as_ref().is_none_or(|t| t.size != size) {
            let target = create_view_target(
                &self.device,
                &self.present,
                size,
                self.samples,
                self.view_formats,
            );
            match self.texture_id {
                Some(id) => egui_renderer.update_egui_texture_from_wgpu_texture(
                    &self.device,
                    target.egui_view(),
                    wgpu::FilterMode::Linear,
                    id,
                ),
                None => {
                    self.texture_id = Some(egui_renderer.register_native_texture(
                        &self.device,
                        target.egui_view(),
                        wgpu::FilterMode::Linear,
                    ));
                }
            }
            self.target = Some(target);
        }
        self.texture_id.expect("texture registered")
    }

    /// Writes the camera of `tile` and returns its view-projection matrix.
    fn write_camera(&self, input: &FrameInput<'_>, tile: Tile) -> Mat4 {
        let camera = input.camera;
        let [image_width, image_height] = tile.image.map(|v| v as f32);
        let [width, height] = tile.size.map(|v| v as f32);
        let [x, y] = tile.offset.map(|v| v as f32);
        let vec4 = |v: Vec3, w: f32| [v.x, v.y, v.z, w];
        let section = input.section;
        let view_projection =
            tile.projection() * camera.eye_view_projection(image_width / image_height, input.eye);
        let uniform = CameraUniform {
            clip_plane: section.map_or([0.0; 4], |s| vec4(s.normal, s.offset)),
            section: [f32::from(u8::from(section.is_some())), 0.0, 0.0, 0.0],
            cap_corners: section.map_or([[0.0; 4]; 4], |s| s.cap_corners.map(|c| vec4(c, 1.0))),
            view_proj: view_projection.to_cols_array_2d(),
            eye: vec4(
                camera.eye_position(input.eye),
                if camera.is_perspective() { 1.0 } else { 0.0 },
            ),
            forward: vec4(camera.forward(), 0.0),
            right: vec4(camera.right(), 0.0),
            up: vec4(camera.up(), 0.0),
            viewport: [width, height, 1.0 / width, 1.0 / height],
            background_top: input.background_top,
            background_bottom: input.background_bottom,
            frame: [x, y, image_width, image_height],
            ..lighting_uniform(&input.lighting)
        };
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform));
        view_projection
    }

    /// One uniform block per item, at `object_stride` intervals; slot 0 is
    /// reserved for the background pass, so item `i` lives in slot `i + 1`.
    fn write_objects(&mut self, items: &[DrawItem], pixel_scale: f32) {
        let needed = items.len() + 1;
        if needed > self.object_capacity {
            self.object_capacity = needed.next_power_of_two();
            let (buffer, bind_group) = create_object_buffer(
                &self.device,
                &self.object_layout,
                self.object_stride,
                self.object_capacity,
            );
            self.object_buffer = buffer;
            self.object_bind_group = bind_group;
        }
        let stride = self.object_stride as usize;
        let mut data = vec![0u8; needed * stride];
        for (slot, item) in items.iter().enumerate() {
            let style = &item.style;
            let uniform = ObjectUniform {
                model: item.transform.to_cols_array_2d(),
                face_color: style.face_color,
                edge_color: style.edge_color,
                highlight: style.highlight,
                params: [
                    style.edge_width * pixel_scale,
                    style.point_size * pixel_scale,
                    f32::from(u8::from(style.clipped)),
                    f32::from(u8::from(style.background_fill)),
                ],
                edge_filter: [style.edge_min_angle, 0.0, 0.0, 0.0],
                surface: style.surface,
            };
            let offset = (slot + 1) * stride;
            data[offset..offset + OBJECT_UNIFORM_SIZE as usize]
                .copy_from_slice(bytemuck::bytes_of(&uniform));
        }
        self.queue.write_buffer(&self.object_buffer, 0, &data);
    }

    fn object_offset(&self, index: usize) -> u32 {
        ((index + 1) as u64 * self.object_stride) as u32
    }

    fn draw_faces(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        pipeline: &wgpu::RenderPipeline,
        input: &FrameInput<'_>,
        index: usize,
    ) {
        let item = &input.items[index];
        let Some(mesh) = self.meshes.get(&item.mesh.id) else {
            return;
        };
        if !item.style.faces || mesh.index_count == 0 {
            return;
        }
        let (Some(vertices), Some(indices)) = (&mesh.vertices, &mesh.indices) else {
            return;
        };
        pass.set_pipeline(pipeline);
        pass.set_bind_group(1, &self.object_bind_group, &[self.object_offset(index)]);
        pass.set_vertex_buffer(0, vertices.slice(..));
        pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..mesh.index_count, 0, 0..1);
    }

    /// Fills where the section plane cuts a clipped item: counts its
    /// remaining surfaces into the stencil, then draws the plane there.
    fn draw_cap(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        cap: &wgpu::RenderPipeline,
        input: &FrameInput<'_>,
        index: usize,
    ) {
        let item = &input.items[index];
        if !item.style.clipped {
            return;
        }
        let Some(mesh) = self.meshes.get(&item.mesh.id) else {
            return;
        };
        let (Some(vertices), Some(indices)) = (&mesh.vertices, &mesh.indices) else {
            return;
        };
        if !item.style.faces || mesh.index_count == 0 {
            return;
        }
        pass.set_bind_group(1, &self.object_bind_group, &[self.object_offset(index)]);
        pass.set_pipeline(&self.pipelines.section_count);
        pass.set_vertex_buffer(0, vertices.slice(..));
        pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..mesh.index_count, 0, 0..1);
        pass.set_pipeline(cap);
        pass.draw(0..6, 0..1);
    }

    fn draw_edges(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        pipeline: &wgpu::RenderPipeline,
        input: &FrameInput<'_>,
        index: usize,
    ) {
        let item = &input.items[index];
        let Some(mesh) = self.meshes.get(&item.mesh.id) else {
            return;
        };
        let Some(segments) = &mesh.segments else {
            return;
        };
        if !item.style.edges || mesh.segment_count == 0 {
            return;
        }
        pass.set_pipeline(pipeline);
        pass.set_bind_group(1, &self.object_bind_group, &[self.object_offset(index)]);
        pass.set_vertex_buffer(0, segments.slice(..));
        pass.draw(0..6, 0..mesh.segment_count);
    }

    /// The outline of the curved surfaces of an item drawn with one.
    fn draw_silhouettes(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        input: &FrameInput<'_>,
        index: usize,
    ) {
        let item = &input.items[index];
        if !(item.style.edges && item.style.silhouettes && item.style.faces) {
            return;
        }
        let Some(mesh) = self.meshes.get(&item.mesh.id) else {
            return;
        };
        let silhouettes = mesh.silhouettes.get_or_init(|| {
            let edges = item.mesh.silhouette_edges();
            (!edges.is_empty()).then(|| {
                let buffer = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("silhouettes"),
                        contents: bytemuck::cast_slice(edges),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                (buffer, edges.len() as u32)
            })
        });
        let Some((buffer, count)) = silhouettes else {
            return;
        };
        pass.set_pipeline(&self.pipelines.silhouettes);
        pass.set_bind_group(1, &self.object_bind_group, &[self.object_offset(index)]);
        pass.set_vertex_buffer(0, buffer.slice(..));
        pass.draw(0..6, 0..*count);
    }

    fn draw_points(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        pipeline: &wgpu::RenderPipeline,
        input: &FrameInput<'_>,
        index: usize,
    ) {
        let item = &input.items[index];
        let Some(mesh) = self.meshes.get(&item.mesh.id) else {
            return;
        };
        let Some(points) = &mesh.points else { return };
        if !item.style.points || mesh.point_count == 0 {
            return;
        }
        pass.set_pipeline(pipeline);
        pass.set_bind_group(1, &self.object_bind_group, &[self.object_offset(index)]);
        pass.set_vertex_buffer(0, points.slice(..));
        pass.draw(0..6, 0..mesh.point_count);
    }
}

/// The lighting fields of the camera uniform, the others zero. Values stay
/// in ranges the shader handles: no negative light and no NaN, whatever the
/// settings file says.
fn lighting_uniform(lighting: &Lighting) -> CameraUniform {
    let bounded = |value: f32, min: f32, max: f32, fallback: f32| {
        if value.is_finite() {
            value.clamp(min, max)
        } else {
            fallback
        }
    };
    let defaults = Lighting::default();
    let direction = Lighting {
        azimuth: bounded(lighting.azimuth, -180.0, 180.0, defaults.azimuth),
        elevation: bounded(lighting.elevation, -90.0, 90.0, defaults.elevation),
        ..defaults
    };
    let [x, y, z] = direction.key_direction();
    CameraUniform {
        key_light: [x, y, z, 0.0],
        light_levels: [
            bounded(lighting.key, 0.0, 4.0, defaults.key),
            bounded(lighting.fill, 0.0, 4.0, defaults.fill),
            bounded(lighting.ambient, 0.0, 4.0, defaults.ambient),
            bounded(lighting.rim, 0.0, 4.0, defaults.rim),
        ],
        finish: [
            bounded(lighting.specular, 0.0, 4.0, defaults.specular),
            bounded(lighting.shininess, 1.0, 1024.0, defaults.shininess),
            2f32.powf(bounded(lighting.exposure, -6.0, 6.0, 0.0)),
            bounded(lighting.contrast, 0.2, 5.0, 1.0),
        ],
        ..CameraUniform::zeroed()
    }
}

/// Item indices with their layers: opaque order is irrelevant, transparent
/// faces need back-to-front order.
fn draw_order(input: &FrameInput<'_>) -> Vec<(usize, Layer)> {
    let camera = input.camera;
    let eye = camera.eye();
    let forward = camera.forward();
    let mut order: Vec<(usize, f32)> = input
        .items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let center = item.transform.transform_point3(item.mesh.bounds.center());
            (i, (center - eye).dot(forward))
        })
        .collect();
    order.sort_by(|a, b| b.1.total_cmp(&a.1));
    order
        .into_iter()
        .map(|(i, _)| (i, input.items[i].layer))
        .collect()
}

fn create_object_buffer(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    stride: u64,
    capacity: usize,
) -> (wgpu::Buffer, wgpu::BindGroup) {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("objects"),
        size: stride * capacity as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("objects"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &buffer,
                offset: 0,
                size: NonZeroU64::new(OBJECT_UNIFORM_SIZE),
            }),
        }],
    });
    (buffer, bind_group)
}

/// A target drawn off screen and read back (captures, stereo eyes).
fn create_target(device: &wgpu::Device, size: [u32; 2], samples: u32) -> Target {
    create_target_with(device, size, samples, &[])
}

/// The view's target, which egui shows: through a plain view of the sRGB
/// texture when the device can make one (`view_formats`), else through a
/// plain copy that `SceneRenderer::present` fills.
fn create_view_target(
    device: &wgpu::Device,
    present: &Present,
    size: [u32; 2],
    samples: u32,
    view_formats: bool,
) -> Target {
    if view_formats {
        let mut target = create_target_with(device, size, samples, &[EGUI_VIEW_FORMAT]);
        target.egui = Some(EguiTexture::View(target.resolved.create_view(
            &wgpu::TextureViewDescriptor {
                format: Some(EGUI_VIEW_FORMAT),
                ..Default::default()
            },
        )));
        return target;
    }
    let mut target = create_target(device, size, samples);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("scene for egui"),
        size: wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: EGUI_VIEW_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("present"),
        layout: &present.layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&target.resolved_view),
        }],
    });
    target.egui = Some(EguiTexture::Encoded {
        view: texture.create_view(&wgpu::TextureViewDescriptor::default()),
        bind_group,
    });
    target
}

fn create_target_with(
    device: &wgpu::Device,
    size: [u32; 2],
    samples: u32,
    view_formats: &[wgpu::TextureFormat],
) -> Target {
    let extent = wgpu::Extent3d {
        width: size[0],
        height: size[1],
        depth_or_array_layers: 1,
    };
    let texture = |label, format, samples, usage, view_formats: &[wgpu::TextureFormat]| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: extent,
            mip_level_count: 1,
            sample_count: samples,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats,
        })
    };

    let resolved = texture(
        "scene color",
        COLOR_FORMAT,
        1,
        wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats,
    );
    let resolved_view = resolved.create_view(&wgpu::TextureViewDescriptor::default());
    let multisampled = (samples > 1).then(|| {
        texture(
            "scene msaa",
            COLOR_FORMAT,
            samples,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
            &[],
        )
        .create_view(&wgpu::TextureViewDescriptor::default())
    });
    let depth_view = texture(
        "scene depth",
        DEPTH_FORMAT,
        samples,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
        &[],
    )
    .create_view(&wgpu::TextureViewDescriptor::default());

    Target {
        size,
        multisampled,
        resolved,
        resolved_view,
        depth_view,
        egui: None,
    }
}

fn upload(device: &wgpu::Device, mesh: &Arc<Mesh>) -> GpuMesh {
    let buffer = |label, contents: &[u8], usage| {
        (!contents.is_empty()).then(|| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents,
                usage,
            })
        })
    };
    let interleaved: Vec<MeshVertex> = mesh
        .positions
        .iter()
        .zip(&mesh.normals)
        .enumerate()
        .map(|(index, (p, n))| MeshVertex {
            position: *p,
            normal: *n,
            face_color: mesh.vertex_colors.get(index).copied().unwrap_or(0),
        })
        .collect();
    let has_triangles = !interleaved.is_empty() && !mesh.indices.is_empty();
    // Each segment: its two ends and the angle its edge's faces meet at.
    let segments: Vec<[f32; 7]> = mesh
        .edges
        .as_chunks::<2>()
        .0
        .iter()
        .enumerate()
        .map(|(index, [a, b])| {
            [
                a[0],
                a[1],
                a[2],
                b[0],
                b[1],
                b[2],
                mesh.segment_angle(index),
            ]
        })
        .collect();
    GpuMesh {
        source: Arc::downgrade(mesh),
        vertices: has_triangles
            .then(|| {
                buffer(
                    "vertices",
                    bytemuck::cast_slice(&interleaved),
                    wgpu::BufferUsages::VERTEX,
                )
            })
            .flatten(),
        indices: has_triangles
            .then(|| {
                buffer(
                    "indices",
                    bytemuck::cast_slice(&mesh.indices),
                    wgpu::BufferUsages::INDEX,
                )
            })
            .flatten(),
        index_count: if has_triangles {
            mesh.indices.len() as u32
        } else {
            0
        },
        segments: buffer(
            "segments",
            bytemuck::cast_slice(&segments),
            wgpu::BufferUsages::VERTEX,
        ),
        segment_count: segments.len() as u32,
        points: buffer(
            "points",
            bytemuck::cast_slice(&mesh.points),
            wgpu::BufferUsages::VERTEX,
        ),
        point_count: mesh.points.len() as u32,
        silhouettes: std::cell::OnceCell::new(),
    }
}

impl Present {
    fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("present.wgsl"));
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("present"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("present"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("present"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_present"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_present"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: EGUI_VIEW_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        Self { pipeline, layout }
    }
}

impl Compose {
    fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("stereo.wgsl"));
        let texture = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("stereo"),
            entries: &[
                texture(0),
                texture(1),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("stereo"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("stereo"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_compose"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_compose"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: COLOR_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("stereo"),
            size: std::mem::size_of::<ComposeUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            layout,
            uniform,
        }
    }
}

impl Pipelines {
    fn new(
        device: &wgpu::Device,
        layout: &wgpu::PipelineLayout,
        shader: &wgpu::ShaderModule,
        samples: u32,
    ) -> Self {
        const MESH_ATTRIBUTES: [wgpu::VertexAttribute; 3] =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Uint32];
        const SEGMENT_ATTRIBUTES: [wgpu::VertexAttribute; 3] =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32];
        const SILHOUETTE_ATTRIBUTES: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
            0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Float32x3
        ];
        const POINT_ATTRIBUTES: [wgpu::VertexAttribute; 1] =
            wgpu::vertex_attr_array![0 => Float32x3];

        let mesh_buffers = [Some(wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<MeshVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &MESH_ATTRIBUTES,
        })];
        let segment_buffers = [Some(wgpu::VertexBufferLayout {
            array_stride: 28,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &SEGMENT_ATTRIBUTES,
        })];
        let silhouette_buffers = [Some(wgpu::VertexBufferLayout {
            array_stride: 48,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &SILHOUETTE_ATTRIBUTES,
        })];
        let point_buffers = [Some(wgpu::VertexBufferLayout {
            array_stride: 12,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &POINT_ATTRIBUTES,
        })];

        let builder = PipelineBuilder {
            device,
            layout,
            shader,
            samples,
        };
        // Faces are pushed slightly back so that edges lying on them win the
        // depth test; overlay faces are pulled forward instead.
        let push_back = wgpu::DepthBiasState {
            constant: 2,
            slope_scale: 1.5,
            clamp: 0.0,
        };
        let pull_forward = wgpu::DepthBiasState {
            constant: -4,
            slope_scale: -2.0,
            clamp: 0.0,
        };
        let no_bias = wgpu::DepthBiasState::default();
        use wgpu::CompareFunction::{Always, Greater, Less, LessEqual, NotEqual};
        use wgpu::StencilOperation::{Invert, Keep, Zero};

        // Section caps: parity of the remaining surfaces, then a quad where
        // it is odd that also resets the stencil, even where it is hidden.
        let face = |compare, depth_fail_op, pass_op| wgpu::StencilFaceState {
            compare,
            fail_op: Keep,
            depth_fail_op,
            pass_op,
        };
        let stencil = |face: wgpu::StencilFaceState| wgpu::StencilState {
            front: face,
            back: face,
            read_mask: 0xff,
            write_mask: 0xff,
        };
        let count = stencil(face(Always, Invert, Invert));
        let fill = stencil(face(NotEqual, Zero, Zero));

        Self {
            section_count: builder.build_with(Spec {
                stencil: count,
                write_mask: wgpu::ColorWrites::empty(),
                ..Spec::new(
                    "section count",
                    "vs_mesh",
                    "fs_section_count",
                    &mesh_buffers,
                    None,
                    Always,
                    false,
                    no_bias,
                )
            }),
            caps: builder.build_with(Spec {
                stencil: fill.clone(),
                ..Spec::new("caps", "vs_cap", "fs_cap", &[], None, Less, true, push_back)
            }),
            transparent_caps: builder.build_with(Spec {
                stencil: fill.clone(),
                ..Spec::new(
                    "transparent caps",
                    "vs_cap",
                    "fs_cap",
                    &[],
                    Some(wgpu::BlendState::ALPHA_BLENDING),
                    Less,
                    false,
                    push_back,
                )
            }),
            background: builder.build(
                "background",
                "vs_background",
                "fs_background",
                &[],
                None,
                Always,
                false,
                no_bias,
            ),
            reference_edges: builder.build(
                "reference edges",
                "vs_edge",
                "fs_edge",
                &segment_buffers,
                Some(wgpu::BlendState::ALPHA_BLENDING),
                Always,
                false,
                no_bias,
            ),
            faces: builder.build(
                "faces",
                "vs_mesh",
                "fs_mesh",
                &mesh_buffers,
                None,
                Less,
                true,
                push_back,
            ),
            transparent_faces: builder.build(
                "transparent faces",
                "vs_mesh",
                "fs_mesh",
                &mesh_buffers,
                Some(wgpu::BlendState::ALPHA_BLENDING),
                Less,
                false,
                push_back,
            ),
            overlay_faces: builder.build(
                "overlay faces",
                "vs_mesh",
                "fs_mesh",
                &mesh_buffers,
                Some(wgpu::BlendState::ALPHA_BLENDING),
                LessEqual,
                false,
                pull_forward,
            ),
            edges: builder.build(
                "edges",
                "vs_edge",
                "fs_edge",
                &segment_buffers,
                Some(wgpu::BlendState::ALPHA_BLENDING),
                LessEqual,
                true,
                no_bias,
            ),
            silhouettes: builder.build(
                "silhouettes",
                "vs_silhouette",
                "fs_edge",
                &silhouette_buffers,
                Some(wgpu::BlendState::ALPHA_BLENDING),
                LessEqual,
                true,
                no_bias,
            ),
            hidden_edges: builder.build(
                "hidden edges",
                "vs_edge",
                "fs_hidden_edge",
                &segment_buffers,
                Some(wgpu::BlendState::ALPHA_BLENDING),
                Greater,
                false,
                no_bias,
            ),
            overlay_edges: builder.build(
                "overlay edges",
                "vs_edge",
                "fs_edge",
                &segment_buffers,
                Some(wgpu::BlendState::ALPHA_BLENDING),
                Always,
                false,
                no_bias,
            ),
            points: builder.build(
                "points",
                "vs_point",
                "fs_point",
                &point_buffers,
                None,
                LessEqual,
                true,
                no_bias,
            ),
            overlay_points: builder.build(
                "overlay points",
                "vs_point",
                "fs_point",
                &point_buffers,
                None,
                Always,
                false,
                no_bias,
            ),
        }
    }
}

struct PipelineBuilder<'a> {
    device: &'a wgpu::Device,
    layout: &'a wgpu::PipelineLayout,
    shader: &'a wgpu::ShaderModule,
    samples: u32,
}

/// One pipeline's state.
struct Spec<'a> {
    label: &'a str,
    vertex: &'a str,
    fragment: &'a str,
    buffers: &'a [Option<wgpu::VertexBufferLayout<'a>>],
    blend: Option<wgpu::BlendState>,
    depth_compare: wgpu::CompareFunction,
    depth_write: bool,
    bias: wgpu::DepthBiasState,
    stencil: wgpu::StencilState,
    write_mask: wgpu::ColorWrites,
}

impl<'a> Spec<'a> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        label: &'a str,
        vertex: &'a str,
        fragment: &'a str,
        buffers: &'a [Option<wgpu::VertexBufferLayout<'a>>],
        blend: Option<wgpu::BlendState>,
        depth_compare: wgpu::CompareFunction,
        depth_write: bool,
        bias: wgpu::DepthBiasState,
    ) -> Self {
        Self {
            label,
            vertex,
            fragment,
            buffers,
            blend,
            depth_compare,
            depth_write,
            bias,
            stencil: wgpu::StencilState::default(),
            write_mask: wgpu::ColorWrites::ALL,
        }
    }
}

impl PipelineBuilder<'_> {
    #[allow(clippy::too_many_arguments)]
    fn build(
        &self,
        label: &str,
        vertex_entry: &str,
        fragment_entry: &str,
        buffers: &[Option<wgpu::VertexBufferLayout<'_>>],
        blend: Option<wgpu::BlendState>,
        depth_compare: wgpu::CompareFunction,
        depth_write: bool,
        bias: wgpu::DepthBiasState,
    ) -> wgpu::RenderPipeline {
        self.build_with(Spec::new(
            label,
            vertex_entry,
            fragment_entry,
            buffers,
            blend,
            depth_compare,
            depth_write,
            bias,
        ))
    }

    fn build_with(&self, spec: Spec<'_>) -> wgpu::RenderPipeline {
        let Spec {
            label,
            vertex: vertex_entry,
            fragment: fragment_entry,
            buffers,
            blend,
            depth_compare,
            depth_write,
            bias,
            stencil,
            write_mask,
        } = spec;
        self.device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(self.layout),
                vertex: wgpu::VertexState {
                    module: self.shader,
                    entry_point: Some(vertex_entry),
                    compilation_options: Default::default(),
                    buffers,
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(depth_write),
                    depth_compare: Some(depth_compare),
                    stencil,
                    bias,
                }),
                multisample: wgpu::MultisampleState {
                    count: self.samples,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                fragment: Some(wgpu::FragmentState {
                    module: self.shader,
                    entry_point: Some(fragment_entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: COLOR_FORMAT,
                        blend: blend.or(Some(wgpu::BlendState::REPLACE)),
                        write_mask,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
    }
}

#[cfg(test)]
#[path = "../../tests/rust/viewer/test_renderer.rs"]
mod tests;
