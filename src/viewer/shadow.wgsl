// The soft shadow a model casts on the ground in the Presentation mode.
//
// 1. Occlusion: the faces of the model are drawn from straight above into a
//    two-channel texture. Each texel keeps (by max blending) how close to
//    the ground the lowest face over it is: 1 where it touches the ground,
//    0 at the fade height and above.
// 2. Blur: two passes, across and then along, soften the edges: a little in
//    the red channel, for the contact shadow, much in the green one, for
//    the ambient shadow.
// 3. Floor: a quad on the ground under the model, drawn in the scene pass
//    after the opaque faces, darkens the background by both shadows; the
//    depth test keeps it behind the model.
//
// The passes use distinct bindings, as they share this module.

// --- Occlusion ------------------------------------------------------------

struct ShadowView {
    view_proj: mat4x4<f32>,
    // x: ground height; y: fade height.
    ground: vec4<f32>,
};

// The first member of the scene's object block (`shaders.wgsl`).
struct Object {
    model: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> shadow_view: ShadowView;
@group(1) @binding(0) var<uniform> object: Object;

struct OcclusionOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) height: f32,
};

@vertex
fn vs_occlusion(@location(0) position: vec3<f32>) -> OcclusionOut {
    let world = object.model * vec4<f32>(position, 1.0);
    var out: OcclusionOut;
    out.clip = shadow_view.view_proj * world;
    out.height = world.z - shadow_view.ground.x;
    return out;
}

@fragment
fn fs_occlusion(in: OcclusionOut) -> @location(0) vec4<f32> {
    let occlusion = clamp(1.0 - in.height / max(shadow_view.ground.y, 1e-6), 0.0, 1.0);
    return vec4<f32>(occlusion, occlusion, 0.0, 1.0);
}

// --- Blur -------------------------------------------------------------------

struct Blur {
    // xy: one texel along the blur, in texture coordinates.
    step: vec4<f32>,
    // The standard deviations, in texels: x of the contact shadow, y of the
    // ambient one; z: how many texels are read on each side.
    spread: vec4<f32>,
};

@group(0) @binding(1) var blur_source: texture_2d<f32>;
@group(0) @binding(2) var blur_sampler: sampler;
@group(0) @binding(3) var<uniform> blur: Blur;

struct FullscreenOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_fullscreen(@builtin(vertex_index) index: u32) -> FullscreenOut {
    // One triangle covering the whole texture.
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var out: FullscreenOut;
    out.position = vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(corner.x, 1.0 - corner.y);
    return out;
}

@fragment
fn fs_blur(in: FullscreenOut) -> @location(0) vec4<f32> {
    // Two Gaussians read texel by texel, without the bands of sparse taps.
    let reach = i32(blur.spread.z);
    let deviations = max(blur.spread.xy, vec2<f32>(1e-3));
    var total = vec2<f32>(0.0);
    var weights = vec2<f32>(0.0);
    for (var i = -reach; i <= reach; i++) {
        let offset = f32(i) / deviations;
        let weight = exp(-0.5 * offset * offset);
        let uv = in.uv + blur.step.xy * f32(i);
        total += weight * textureSampleLevel(blur_source, blur_sampler, uv, 0.0).rg;
        weights += weight;
    }
    return vec4<f32>(total / weights, 0.0, 1.0);
}

// --- Floor ------------------------------------------------------------------

// The first member of the scene's camera block (`shaders.wgsl`).
struct Camera {
    view_proj: mat4x4<f32>,
};

struct Floor {
    // xy: the corner of the shadowed square with the smallest x and y; z: its
    // side; w: the height of the floor.
    area: vec4<f32>,
    // x: darkness where the model touches the ground.
    look: vec4<f32>,
};

@group(0) @binding(4) var<uniform> camera: Camera;
@group(0) @binding(5) var floor_occlusion: texture_2d<f32>;
@group(0) @binding(6) var floor_sampler: sampler;
@group(0) @binding(7) var<uniform> floor: Floor;

struct FloorOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_floor(@builtin(vertex_index) index: u32) -> FloorOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
    );
    let corner = corners[index];
    let world = vec3<f32>(floor.area.xy + corner * floor.area.z, floor.area.w);
    var out: FloorOut;
    out.clip = camera.view_proj * vec4<f32>(world, 1.0);
    // The occlusion texture was drawn looking down with +Y up.
    out.uv = vec2<f32>(corner.x, 1.0 - corner.y);
    return out;
}

@fragment
fn fs_floor(in: FloorOut) -> @location(0) vec4<f32> {
    let shadows = textureSample(floor_occlusion, floor_sampler, in.uv).rg;
    // The ambient shadow fades slowly, like the sky the model hides from the
    // ground: a long tail rather than the quick end of a Gaussian, smooth down
    // to the last levels of the texture.
    let ambient = 1.0 - pow(1.0 - shadows.g, 3.0);
    let occlusion = 0.45 * shadows.r + 0.55 * ambient;
    // Faded out towards the border of the square, never cut off.
    let border = smoothstep(vec2<f32>(0.0), vec2<f32>(0.08), in.uv)
        * (vec2<f32>(1.0) - smoothstep(vec2<f32>(0.92), vec2<f32>(1.0), in.uv));
    return vec4<f32>(0.0, 0.0, 0.0, occlusion * floor.look.x * border.x * border.y);
}
