//! The soft shadow a model casts on the ground in the Presentation mode,
//! the contact shadow of a product photograph (`shadow.wgsl`).
//!
//! It does not depend on the camera: the faces are drawn once from straight
//! above, blurred, and laid on the ground under the model, so the shadow is
//! the same in every view, in every tile of a large capture and for both
//! eyes of the stereo projection. It is drawn again only when the shadowed
//! items or the settings change.

use std::hash::{Hash, Hasher};

use bytemuck::{Pod, Zeroable};
use eframe::egui_wgpu::wgpu;
use glam::camera::rh::{proj::directx, view};
use glam::{Mat4, Vec3};

use super::renderer::{DrawItem, Layer, MeshVertex};
use crate::scene::Aabb;

/// Side of the occlusion textures, in texels.
const SIZE: u32 = 256;
/// Red: the contact shadow; green: the ambient one.
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg8Unorm;
/// How far the shadow spreads past the model (the standard deviation of its
/// blur) as a fraction of the model's width, from crisp (softness 0) to very
/// soft (1): a dark contact shadow where the model meets the ground, and a
/// wide, faint ambient one around it.
const CONTACT: [f32; 2] = [0.015, 0.06];
const AMBIENT: [f32; 2] = [0.06, 0.3];
/// The square of ground under the model is this many times as wide as the
/// model: room for the softest ambient shadow on each side.
const SIDE: f32 = 1.0 + 6.0 * AMBIENT[1];

/// A shadow on the ground, as the Presentation settings ask for it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundShadow {
    /// Darkness where the model touches the ground, 0 to 1.
    pub strength: f32,
    /// How far the edge spreads, 0 (crisp) to 1 (very soft).
    pub softness: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ViewUniform {
    view_proj: [[f32; 4]; 4],
    ground: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BlurUniform {
    step: [f32; 4],
    spread: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct FloorUniform {
    area: [f32; 4],
    look: [f32; 4],
}

/// Where the shadow lies: a square of the ground under the model.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowArea {
    /// The corner with the smallest x and y.
    pub corner: [f32; 2],
    pub side: f32,
    /// Height of the ground: the bottom of the model.
    pub ground: f32,
    /// Faces this far above the ground no longer cast a shadow.
    pub fade: f32,
}

impl ShadowArea {
    /// The square of ground around `bounds` (world space, Z up) that the
    /// shadow of a model with those bounds needs, its edge blur included.
    pub fn around(bounds: Aabb) -> Option<Self> {
        let size = bounds.size();
        if !size.is_finite() || bounds.is_empty() {
            return None;
        }
        let across = size.x.max(size.y).max(size.z * 0.25).max(1e-3);
        let side = across * SIDE;
        let center = bounds.center();
        Some(Self {
            corner: [center.x - side * 0.5, center.y - side * 0.5],
            side,
            ground: bounds.min.z,
            fade: (size.z * 0.6).max(across * 0.15),
        })
    }

    /// Looking straight down at the square, +Y up.
    fn view_projection(&self) -> Mat4 {
        let half = self.side * 0.5;
        let center = Vec3::new(self.corner[0] + half, self.corner[1] + half, 0.0);
        let top = self.ground + self.fade * 4.0 + self.side;
        let view = view::look_to_mat4(Vec3::new(center.x, center.y, top), Vec3::NEG_Z, Vec3::Y);
        // Everything from just under the ground to far above it.
        let depth = top - self.ground + self.side;
        directx::orthographic(-half, half, -half, half, 0.0, depth) * view
    }
}

/// The items whose faces cast the shadow: the model's opaque enough faces.
pub fn casts_shadow(item: &DrawItem) -> bool {
    item.layer == Layer::Scene
        && item.style.faces
        && !item.style.background_fill
        && item.style.face_color[3] >= 0.3
        && !item.mesh.indices.is_empty()
}

/// The world bounds of the items that cast the shadow.
pub fn shadow_bounds(items: &[DrawItem]) -> Aabb {
    items
        .iter()
        .filter(|item| casts_shadow(item))
        .fold(Aabb::EMPTY, |bounds, item| {
            bounds.union(item.mesh.bounds.transformed(item.transform))
        })
}

/// What the shadow was drawn from, to know when to draw it again.
fn shadow_key(items: &[DrawItem], area: &ShadowArea, shadow: &GroundShadow) -> u64 {
    let mut hasher = std::hash::DefaultHasher::new();
    for item in items.iter().filter(|item| casts_shadow(item)) {
        item.mesh.id.hash(&mut hasher);
        for value in item.transform.to_cols_array() {
            value.to_bits().hash(&mut hasher);
        }
    }
    for value in [
        area.corner[0],
        area.corner[1],
        area.side,
        area.ground,
        area.fade,
        shadow.softness,
    ] {
        value.to_bits().hash(&mut hasher);
    }
    hasher.finish()
}

/// The GPU side: pipelines, the two occlusion textures (drawn into, then
/// blurred back and forth) and the floor's bind group.
pub struct ShadowPass {
    occlusion: wgpu::RenderPipeline,
    blur: wgpu::RenderPipeline,
    floor: wgpu::RenderPipeline,
    view_buffer: wgpu::Buffer,
    view_group: wgpu::BindGroup,
    blur_buffers: [wgpu::Buffer; 2],
    blur_groups: [wgpu::BindGroup; 2],
    floor_buffer: wgpu::Buffer,
    floor_group: wgpu::BindGroup,
    textures: [wgpu::TextureView; 2],
    /// What the textures hold now.
    key: Option<u64>,
    area: Option<ShadowArea>,
}

impl ShadowPass {
    /// `object_layout` is the scene's per-item uniform block, whose model
    /// matrix places the faces; `camera_buffer` the scene's camera, which
    /// places the floor; `samples` and `depth_format` those of the scene
    /// pass the floor is drawn in.
    pub fn new(
        device: &wgpu::Device,
        object_layout: &wgpu::BindGroupLayout,
        camera_buffer: &wgpu::Buffer,
        color_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
        samples: u32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("shadow.wgsl"));
        let uniform = |binding, stage| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: stage,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let texture = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let sampler = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        };
        let layout = |label, entries: &[wgpu::BindGroupLayoutEntry]| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(label),
                entries,
            })
        };
        let view_layout = layout(
            "shadow view",
            &[uniform(0, wgpu::ShaderStages::VERTEX_FRAGMENT)],
        );
        let blur_layout = layout(
            "shadow blur",
            &[
                texture(1),
                sampler(2),
                uniform(3, wgpu::ShaderStages::FRAGMENT),
            ],
        );
        let floor_layout = layout(
            "shadow floor",
            &[
                uniform(4, wgpu::ShaderStages::VERTEX),
                texture(5),
                sampler(6),
                uniform(7, wgpu::ShaderStages::VERTEX_FRAGMENT),
            ],
        );

        let pipeline = |label,
                        layouts: &[Option<&wgpu::BindGroupLayout>],
                        vertex: &str,
                        fragment: &str,
                        buffers: &[Option<wgpu::VertexBufferLayout<'_>>],
                        target: wgpu::ColorTargetState,
                        depth: Option<wgpu::DepthStencilState>,
                        samples: u32,
                        cull_mode: Option<wgpu::Face>| {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: layouts,
                immediate_size: 0,
            });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vertex),
                    compilation_options: Default::default(),
                    buffers,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fragment),
                    compilation_options: Default::default(),
                    targets: &[Some(target)],
                }),
                primitive: wgpu::PrimitiveState {
                    cull_mode,
                    ..Default::default()
                },
                depth_stencil: depth,
                multisample: wgpu::MultisampleState {
                    count: samples,
                    ..Default::default()
                },
                multiview_mask: None,
                cache: None,
            })
        };
        let max = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Max,
        };
        // Positions of the scene's interleaved vertices.
        let positions = [Some(wgpu::VertexBufferLayout {
            array_stride: size_of::<MeshVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3],
        })];
        let occlusion = pipeline(
            "shadow occlusion",
            &[Some(&view_layout), Some(object_layout)],
            "vs_occlusion",
            "fs_occlusion",
            &positions,
            wgpu::ColorTargetState {
                format: FORMAT,
                blend: Some(wgpu::BlendState {
                    color: max,
                    alpha: max,
                }),
                write_mask: wgpu::ColorWrites::ALL,
            },
            None,
            1,
            None,
        );
        let blur = pipeline(
            "shadow blur",
            &[Some(&blur_layout)],
            "vs_fullscreen",
            "fs_blur",
            &[],
            wgpu::ColorTargetState {
                format: FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            },
            None,
            1,
            None,
        );
        let floor = pipeline(
            "shadow floor",
            &[Some(&floor_layout)],
            "vs_floor",
            "fs_floor",
            &[],
            wgpu::ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            },
            // Behind the model, which hides it; written nowhere.
            Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            samples,
            // Seen from under the ground, it would cover the model's bottom.
            Some(wgpu::Face::Back),
        );

        let buffer = |label, size: usize| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: size as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let view_buffer = buffer("shadow view", size_of::<ViewUniform>());
        let blur_buffers = [
            buffer("shadow blur across", size_of::<BlurUniform>()),
            buffer("shadow blur along", size_of::<BlurUniform>()),
        ];
        let floor_buffer = buffer("shadow floor", size_of::<FloorUniform>());
        let textures = [0, 1].map(|_| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("shadow occlusion"),
                    size: wgpu::Extent3d {
                        width: SIZE,
                        height: SIZE,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&wgpu::TextureViewDescriptor::default())
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let view_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow view"),
            layout: &view_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: view_buffer.as_entire_binding(),
            }],
        });
        // Across: from the drawn texture into the other; along: back.
        let blur_group = |source: &wgpu::TextureView, buffer: &wgpu::Buffer| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("shadow blur"),
                layout: &blur_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(source),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: buffer.as_entire_binding(),
                    },
                ],
            })
        };
        let blur_groups = [
            blur_group(&textures[0], &blur_buffers[0]),
            blur_group(&textures[1], &blur_buffers[1]),
        ];
        let floor_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow floor"),
            layout: &floor_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&textures[0]),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: floor_buffer.as_entire_binding(),
                },
            ],
        });
        Self {
            occlusion,
            blur,
            floor,
            view_buffer,
            view_group,
            blur_buffers,
            blur_groups,
            floor_buffer,
            floor_group,
            textures,
            key: None,
            area: None,
        }
    }

    /// Draws the occlusion of `items` again when they or the settings
    /// changed, and updates the floor. `draw_faces` draws an item's faces in
    /// a pass with the occlusion pipeline set (the scene's own object
    /// uniforms place them). Returns whether there is a shadow to draw.
    pub fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        items: &[DrawItem],
        shadow: &GroundShadow,
        draw_faces: impl Fn(&mut wgpu::RenderPass<'_>, usize),
    ) -> bool {
        let Some(area) = ShadowArea::around(shadow_bounds(items)) else {
            self.area = None;
            return false;
        };
        let floor = FloorUniform {
            // Just under the bottom faces, so they keep the depth test.
            area: [
                area.corner[0],
                area.corner[1],
                area.side,
                area.ground - area.side * 1e-3,
            ],
            look: [shadow.strength.clamp(0.0, 1.0), 0.0, 0.0, 0.0],
        };
        queue.write_buffer(&self.floor_buffer, 0, bytemuck::bytes_of(&floor));
        self.area = Some(area);
        let key = shadow_key(items, &area, shadow);
        if self.key == Some(key) {
            return true;
        }
        self.key = Some(key);

        queue.write_buffer(
            &self.view_buffer,
            0,
            bytemuck::bytes_of(&ViewUniform {
                view_proj: area.view_projection().to_cols_array_2d(),
                ground: [area.ground, area.fade, 0.0, 0.0],
            }),
        );
        // The square is as many times as wide as the model whatever its
        // size, so the spread in texels depends on the softness alone.
        let softness = shadow.softness.clamp(0.0, 1.0);
        let texels =
            |[crisp, soft]: [f32; 2]| (crisp + (soft - crisp) * softness) / SIDE * SIZE as f32;
        let (contact, ambient) = (texels(CONTACT), texels(AMBIENT));
        let texel = 1.0 / SIZE as f32;
        for (buffer, step) in self.blur_buffers.iter().zip([[texel, 0.0], [0.0, texel]]) {
            queue.write_buffer(
                buffer,
                0,
                bytemuck::bytes_of(&BlurUniform {
                    step: [step[0], step[1], 0.0, 0.0],
                    spread: [contact, ambient, (ambient * 3.0).ceil(), 0.0],
                }),
            );
        }

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("ground shadow"),
        });
        {
            let mut occlusion = texture_pass(&mut encoder, &self.textures[0], "shadow occlusion");
            occlusion.set_pipeline(&self.occlusion);
            occlusion.set_bind_group(0, &self.view_group, &[]);
            for (index, item) in items.iter().enumerate() {
                if casts_shadow(item) {
                    draw_faces(&mut occlusion, index);
                }
            }
        }
        for (source, target, label) in [(0, 1, "shadow blur across"), (1, 0, "shadow blur along")] {
            let mut blur = texture_pass(&mut encoder, &self.textures[target], label);
            blur.set_pipeline(&self.blur);
            blur.set_bind_group(0, &self.blur_groups[source], &[]);
            blur.draw(0..3, 0..1);
        }
        queue.submit([encoder.finish()]);
        true
    }

    /// Draws the floor in the scene pass. It replaces the bind group 0 of
    /// the pass, which the caller sets back.
    pub fn draw_floor(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.area.is_none() {
            return;
        }
        pass.set_pipeline(&self.floor);
        pass.set_bind_group(0, &self.floor_group, &[]);
        pass.draw(0..6, 0..1);
    }
}

/// A pass drawing into `view`, cleared first.
fn texture_pass<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    view: &'a wgpu::TextureView,
    label: &'a str,
) -> wgpu::RenderPass<'a> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
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
    })
}

#[cfg(test)]
#[path = "../../tests/rust/viewer/test_shadow.rs"]
mod tests;
