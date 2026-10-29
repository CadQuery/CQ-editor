// Shaders of the 3D view.
//
// Colors arrive in linear space; the render target is sRGB, so the hardware
// encodes on write. Edges and points are camera-facing quads expanded in
// screen space, which gives constant pixel widths at any zoom.
//
// A section plane removes everything in front of it (dot(n, p) > d) from
// the objects that are clipped. The cut faces are filled ("capped") with
// the stencil buffer: each clipped object is drawn once more into the
// stencil only, inverting it per surface, so the pixels where the plane
// lies inside the solid end with an odd count; a quad on the plane is then
// drawn where the stencil is set, which also clears it.
//
// In the hidden-line display mode faces are filled with the background, so
// they only hide what is behind them; the hidden edges are drawn again as
// faint dashes where the depth test fails, and section caps are hatched.

struct Camera {
    view_proj: mat4x4<f32>,
    // xyz: eye position; w: 1 for perspective projection, 0 for orthographic.
    eye: vec4<f32>,
    forward: vec4<f32>,
    right: vec4<f32>,
    up: vec4<f32>,
    // Width and height in pixels, then their reciprocals.
    viewport: vec4<f32>,
    background_top: vec4<f32>,
    background_bottom: vec4<f32>,
    // xyz: normal of the section plane; w: its offset d.
    clip_plane: vec4<f32>,
    // x: 1 while a section plane is active.
    section: vec4<f32>,
    // Corners of the quad covering the section plane around the scene.
    cap_corners: array<vec4<f32>, 4>,
    // xy: offset of this pass in the whole image (large captures are drawn
    // in tiles); zw: size of the whole image, in pixels.
    frame: vec4<f32>,
    // xyz: direction of the key light in the camera's frame (right, up,
    // towards the viewer).
    key_light: vec4<f32>,
    // Strength of the key, fill, ambient and rim lights.
    light_levels: vec4<f32>,
    // x: highlight strength; y: shininess; z: exposure factor; w: contrast.
    finish: vec4<f32>,
};

struct Object {
    model: mat4x4<f32>,
    face_color: vec4<f32>,
    edge_color: vec4<f32>,
    // rgb: highlight color; a: how strongly faces are tinted with it.
    highlight: vec4<f32>,
    // x: edge width (px); y: point diameter (px); z: 1 when the section
    // plane clips this object; w: 1 to fill faces with the background
    // (hidden-line mode).
    params: vec4<f32>,
    // x: edges whose faces meet at a smaller angle (degrees) are left out.
    edge_filter: vec4<f32>,
    // x: shading model (0 plain, 1 physically based, 2 the same with a
    // wood grain); y: metalness; z: roughness.
    surface: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var<uniform> object: Object;

// Whether the section plane removes this point of the current object.
fn is_cut_away(world: vec3<f32>) -> bool {
    return camera.section.x > 0.5 && object.params.z > 0.5
        && dot(camera.clip_plane.xyz, world) > camera.clip_plane.w;
}

// The background behind a fragment at framebuffer row `y`, alpha included
// (0 for captures with a transparent background).
fn background_at(y: f32) -> vec4<f32> {
    let height = clamp(1.0 - (y + camera.frame.y) / camera.frame.w, 0.0, 1.0);
    return mix(camera.background_bottom, camera.background_top, height);
}

fn fills_with_background() -> bool {
    return object.params.w > 0.5;
}

// Normal facing the viewer, for two-sided lighting.
fn facing_normal(normal: vec3<f32>, world: vec3<f32>) -> vec3<f32> {
    var to_eye = -camera.forward.xyz;
    if (camera.eye.w > 0.5) {
        to_eye = normalize(camera.eye.xyz - world);
    }
    var n = normal;
    let length_n = length(n);
    if (length_n < 1e-6) {
        n = to_eye;
    } else {
        n = n / length_n;
    }
    // Light both sides: open shells and inverted faces look the same as
    // closed solids.
    if (dot(n, to_eye) < 0.0) {
        n = -n;
    }
    return n;
}

// Three-point lighting attached to the camera, as the lighting settings
// say: a key light (from the upper left by default), a dimmer fill from the
// lower right and a little rim light.
fn lit(base: vec3<f32>, n: vec3<f32>, world: vec3<f32>) -> vec3<f32> {
    var to_eye = -camera.forward.xyz;
    if (camera.eye.w > 0.5) {
        to_eye = normalize(camera.eye.xyz - world);
    }
    let back = -camera.forward.xyz;
    let key = normalize(
        camera.key_light.x * camera.right.xyz + camera.key_light.y * camera.up.xyz
            + camera.key_light.z * back,
    );
    let fill = normalize(0.65 * camera.right.xyz - 0.25 * camera.up.xyz + 0.72 * back);
    let levels = camera.light_levels;
    let diffuse = levels.x * max(dot(n, key), 0.0) + levels.y * max(dot(n, fill), 0.0);
    let rim = levels.w * pow(1.0 - max(dot(n, to_eye), 0.0), 3.0);
    let half_vector = normalize(key + to_eye);
    let specular = camera.finish.x * pow(max(dot(n, half_vector), 0.0), camera.finish.y);
    return finished(base * (levels.z + diffuse) + vec3<f32>(specular + rim));
}

// The object's shading model: the plain one, or a material.
fn shaded(base: vec3<f32>, n: vec3<f32>, world: vec3<f32>) -> vec3<f32> {
    if (object.surface.x > 0.5) {
        return lit_material(base, n, world, object.surface.y, object.surface.z);
    }
    return lit(base, n, world);
}

const PI: f32 = 3.14159265;

// GGX distribution of microfacet normals.
fn ggx(n_dot_h: f32, alpha: f32) -> f32 {
    let a2 = alpha * alpha;
    let d = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / (PI * d * d);
}

// Shadowing of microfacets (Smith, with the Schlick approximation).
fn smith(n_dot_v: f32, n_dot_l: f32, roughness: f32) -> f32 {
    let k = (roughness + 1.0) * (roughness + 1.0) / 8.0;
    return n_dot_v / (n_dot_v * (1.0 - k) + k) * n_dot_l / (n_dot_l * (1.0 - k) + k);
}

fn fresnel(f0: vec3<f32>, cosine: f32) -> vec3<f32> {
    return f0 + (vec3<f32>(1.0) - f0) * pow(1.0 - clamp(cosine, 0.0, 1.0), 5.0);
}

// A photo studio around the model, attached to the camera like the lights,
// for what metals and glossy surfaces reflect: bright above, darker below,
// a soft box where the key light is. Rough surfaces see it blurred.
fn studio(direction: vec3<f32>, key: vec3<f32>, roughness: f32) -> vec3<f32> {
    let height = dot(direction, camera.up.xyz);
    let sky = mix(0.16, 1.0, smoothstep(-0.4, 0.7, height));
    let spread = mix(300.0, 4.0, roughness);
    let softbox = pow(max(dot(direction, key), 0.0), spread) * mix(3.0, 0.3, roughness);
    return vec3<f32>(mix(sky + softbox, 0.55, roughness * roughness));
}

// The same lights as the plain shading, for a material of the given
// metalness and roughness: microfacet highlights (scaled by the Highlights
// setting, 1 at its default), a diffuse part that metals lack, and
// reflections of the studio, which the ambient light sets.
fn lit_material(
    base: vec3<f32>,
    n: vec3<f32>,
    world: vec3<f32>,
    metallic: f32,
    roughness: f32,
) -> vec3<f32> {
    var to_eye = -camera.forward.xyz;
    if (camera.eye.w > 0.5) {
        to_eye = normalize(camera.eye.xyz - world);
    }
    let back = -camera.forward.xyz;
    let key = normalize(
        camera.key_light.x * camera.right.xyz + camera.key_light.y * camera.up.xyz
            + camera.key_light.z * back,
    );
    let fill = normalize(0.65 * camera.right.xyz - 0.25 * camera.up.xyz + 0.72 * back);
    let levels = camera.light_levels;
    let alpha = max(roughness * roughness, 0.002);
    let f0 = mix(vec3<f32>(0.04), base, metallic);
    let diffuse = base * (1.0 - metallic);
    let n_dot_v = max(dot(n, to_eye), 1e-3);
    let highlights = camera.finish.x / 0.22;

    var color = vec3<f32>(0.0);
    var lights = array<vec4<f32>, 2>(vec4<f32>(key, levels.x), vec4<f32>(fill, levels.y));
    for (var i = 0; i < 2; i++) {
        let light = lights[i].xyz;
        let n_dot_l = dot(n, light);
        if (n_dot_l > 0.0) {
            let half_vector = normalize(light + to_eye);
            let f = fresnel(f0, dot(half_vector, to_eye));
            let specular = ggx(max(dot(n, half_vector), 0.0), alpha)
                * smith(n_dot_v, n_dot_l, roughness) / (4.0 * n_dot_v * n_dot_l);
            color += lights[i].w * n_dot_l
                * ((vec3<f32>(1.0) - f) * diffuse + PI * specular * f * highlights);
        }
    }

    // Fresnel with roughness for the reflections; a sky above for the
    // diffuse ambient light.
    let grazing = pow(1.0 - n_dot_v, 5.0);
    let f_ambient = f0 + (max(vec3<f32>(1.0 - roughness), f0) - f0) * grazing;
    let sky_light = 0.8 + 0.2 * dot(n, camera.up.xyz);
    let reflected = studio(reflect(-to_eye, n), key, roughness);
    color += levels.z * (diffuse * sky_light * (vec3<f32>(1.0) - f_ambient)
        + reflected * f_ambient * 2.4);
    color += vec3<f32>(levels.w * pow(1.0 - n_dot_v, 3.0) * (1.0 - 0.5 * roughness));
    return finished(color);
}

// Hash of a point in space, in 0..1 (no trigonometry, so it is the same
// on every GPU).
fn hash(point: vec3<f32>) -> f32 {
    var q = fract(point * vec3<f32>(0.1031, 0.1030, 0.0973));
    q += dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

// Smooth value noise in 0..1.
fn noise(point: vec3<f32>) -> f32 {
    let cell = floor(point);
    let t = fract(point);
    let w = t * t * (3.0 - 2.0 * t);
    let x00 = mix(hash(cell), hash(cell + vec3<f32>(1.0, 0.0, 0.0)), w.x);
    let x10 = mix(hash(cell + vec3<f32>(0.0, 1.0, 0.0)), hash(cell + vec3<f32>(1.0, 1.0, 0.0)), w.x);
    let x01 = mix(hash(cell + vec3<f32>(0.0, 0.0, 1.0)), hash(cell + vec3<f32>(1.0, 0.0, 1.0)), w.x);
    let x11 = mix(hash(cell + vec3<f32>(0.0, 1.0, 1.0)), hash(cell + vec3<f32>(1.0, 1.0, 1.0)), w.x);
    return mix(mix(x00, x10, w.y), mix(x01, x11, w.y), w.z);
}

// Wood of the color `light` around the part's Z axis: growth rings about
// 2.5 units apart, wavering, darker latewood, and a fine grain along Z.
fn wood(local: vec3<f32>, light: vec3<f32>) -> vec3<f32> {
    let p = local * 0.4;
    let wobble = noise(p * vec3<f32>(0.5, 0.5, 0.06)) * 0.9;
    let ring = fract(length(p.xy) + wobble);
    let latewood = smoothstep(0.55, 0.85, ring) * (1.0 - smoothstep(0.9, 1.0, ring));
    let grain = noise(local * vec3<f32>(4.0, 4.0, 0.25));
    let dark = light * vec3<f32>(0.62, 0.52, 0.44);
    return mix(light, dark, clamp(latewood * 0.7 + grain * 0.2, 0.0, 1.0));
}

// Exposure, then contrast around middle grey (0.18 in linear light). Both
// leave the color as it is at their defaults (1 and 1).
fn finished(color: vec3<f32>) -> vec3<f32> {
    let exposed = max(color * camera.finish.z, vec3<f32>(0.0));
    return 0.18 * pow(exposed / 0.18, vec3<f32>(camera.finish.w));
}

// --- Background ---------------------------------------------------------

struct BackgroundOut {
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_background(@builtin(vertex_index) index: u32) -> BackgroundOut {
    // One triangle covering the whole viewport.
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var out: BackgroundOut;
    out.position = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
    return out;
}

@fragment
fn fs_background(in: BackgroundOut) -> @location(0) vec4<f32> {
    return background_at(in.position.y);
}

// --- Faces --------------------------------------------------------------

struct MeshIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) face_color: u32,
};

struct MeshOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    // In the part's own frame, for the wood grain.
    @location(2) local: vec3<f32>,
    @location(3) @interpolate(flat) face_color: u32,
};

@vertex
fn vs_mesh(in: MeshIn) -> MeshOut {
    var out: MeshOut;
    let world = object.model * vec4<f32>(in.position, 1.0);
    out.clip = camera.view_proj * world;
    out.world = world.xyz;
    let rotation = mat3x3<f32>(object.model[0].xyz, object.model[1].xyz, object.model[2].xyz);
    out.normal = rotation * in.normal;
    out.local = in.position;
    out.face_color = in.face_color;
    return out;
}

@fragment
fn fs_mesh(in: MeshOut) -> @location(0) vec4<f32> {
    if (is_cut_away(in.world)) {
        discard;
    }
    if (fills_with_background()) {
        let background = background_at(in.clip.y);
        let tint = object.highlight.a * 0.5;
        return vec4<f32>(
            mix(background.rgb, object.highlight.rgb, tint),
            mix(background.a, 1.0, tint),
        );
    }
    let n = facing_normal(in.normal, in.world);
    var color = object.face_color.rgb;
    if (in.face_color != 0u) {
        let srgb = vec3<f32>(
            f32(in.face_color & 255u),
            f32((in.face_color >> 8u) & 255u),
            f32((in.face_color >> 16u) & 255u),
        ) / 255.0;
        color = select(srgb / 12.92, pow((srgb + 0.055) / 1.055, vec3<f32>(2.4)), srgb > vec3<f32>(0.04045));
    }
    if (object.surface.x > 1.5) {
        color = wood(in.local, color);
    }
    let base = mix(color, object.highlight.rgb, object.highlight.a);
    return vec4<f32>(shaded(base, n, in.world), object.face_color.a);
}

// Counts the object's remaining surfaces into the stencil; no color.
@fragment
fn fs_section_count(in: MeshOut) {
    if (is_cut_away(in.world)) {
        discard;
    }
}

// --- Section caps -------------------------------------------------------

struct CapOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
};

@vertex
fn vs_cap(@builtin(vertex_index) index: u32) -> CapOut {
    var corner_of = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u);
    let world = camera.cap_corners[corner_of[index]].xyz;
    var out: CapOut;
    out.clip = camera.view_proj * vec4<f32>(world, 1.0);
    out.world = world;
    return out;
}

@fragment
fn fs_cap(in: CapOut) -> @location(0) vec4<f32> {
    if (fills_with_background()) {
        // Hatched, as a section on a drawing, as wide apart as the edges
        // are wide.
        let spacing = max(object.params.x, 1.0) * 5.0;
        let position = in.clip.xy + camera.frame.xy;
        if (fract((position.x + position.y) / spacing) < 0.2) {
            return vec4<f32>(object.edge_color.rgb, 1.0);
        }
        return background_at(in.clip.y);
    }
    // A little darker than the outside, so the cut reads as a cut.
    let n = facing_normal(camera.clip_plane.xyz, in.world);
    let base = mix(object.face_color.rgb, object.highlight.rgb, object.highlight.a) * 0.78;
    return vec4<f32>(shaded(base, n, in.world), object.face_color.a);
}

// --- Edges --------------------------------------------------------------

struct SegmentIn {
    @location(0) start: vec3<f32>,
    @location(1) end: vec3<f32>,
    // Degrees between the faces that meet at the edge; 180 for an outline.
    @location(2) angle: f32,
};

// Pulls a clip-space point in front of the near plane, along the segment.
fn clip_to_near(point: vec4<f32>, other: vec4<f32>) -> vec4<f32> {
    let min_w = 1e-5;
    if (point.w >= min_w || camera.eye.w < 0.5) {
        return point;
    }
    let t = (min_w - point.w) / (other.w - point.w);
    return mix(point, other, t);
}

struct EdgeOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    // Pixels from the start of the segment, for dashes.
    @location(1) @interpolate(linear) along_px: f32,
};

// A vertex outside the view, for edges that are left out: all six land on
// it, so nothing is drawn.
fn left_out() -> EdgeOut {
    var out: EdgeOut;
    out.clip = vec4<f32>(0.0, 0.0, 2.0, 1.0);
    out.world = vec3<f32>(0.0);
    out.along_px = 0.0;
    return out;
}

@vertex
fn vs_edge(@builtin(vertex_index) index: u32, in: SegmentIn) -> EdgeOut {
    if (in.angle < object.edge_filter.x) {
        return left_out();
    }
    return edge_quad(index, object.model * vec4<f32>(in.start, 1.0), object.model * vec4<f32>(in.end, 1.0));
}

struct SilhouetteIn {
    @location(0) start: vec3<f32>,
    @location(1) end: vec3<f32>,
    // Normals of the two triangles that share the edge.
    @location(2) normal_a: vec3<f32>,
    @location(3) normal_b: vec3<f32>,
};

// A mesh edge drawn where one of its triangles faces the camera and the
// other faces away: the outline of a curved surface.
@vertex
fn vs_silhouette(@builtin(vertex_index) index: u32, in: SilhouetteIn) -> EdgeOut {
    let start_world = object.model * vec4<f32>(in.start, 1.0);
    let end_world = object.model * vec4<f32>(in.end, 1.0);
    var to_eye = -camera.forward.xyz;
    if (camera.eye.w > 0.5) {
        to_eye = camera.eye.xyz - 0.5 * (start_world.xyz + end_world.xyz);
    }
    let rotation = mat3x3<f32>(object.model[0].xyz, object.model[1].xyz, object.model[2].xyz);
    let facing_a = dot(rotation * in.normal_a, to_eye);
    let facing_b = dot(rotation * in.normal_b, to_eye);
    if (facing_a * facing_b > 0.0) {
        return left_out();
    }
    return edge_quad(index, start_world, end_world);
}

// The two triangles of a segment from `start_world` to `end_world`,
// expanded to the edge width in screen space.
fn edge_quad(index: u32, start_world: vec4<f32>, end_world: vec4<f32>) -> EdgeOut {
    let start_raw = camera.view_proj * start_world;
    let end_raw = camera.view_proj * end_world;
    let start = clip_to_near(start_raw, end_raw);
    let end = clip_to_near(end_raw, start_raw);

    let half_size = camera.viewport.xy * 0.5;
    let start_px = start.xy / start.w * half_size;
    let end_px = end.xy / end.w * half_size;
    var along = end_px - start_px;
    let length_px = length(along);
    if (length_px > 1e-4) {
        along = along / length_px;
    } else {
        along = vec2<f32>(1.0, 0.0);
    }
    let across = vec2<f32>(-along.y, along.x);

    // Two triangles: (start,-), (end,-), (end,+), (start,-), (end,+), (start,+).
    var at_end = array<f32, 6>(0.0, 1.0, 1.0, 0.0, 1.0, 0.0);
    var side = array<f32, 6>(-1.0, -1.0, 1.0, -1.0, 1.0, 1.0);
    let t = at_end[index];
    var clip = mix(start, end, t);
    let half_width = object.params.x * 0.5;
    // Extending past the endpoints by half a width gives square caps that
    // close the gaps between consecutive segments.
    let offset_px = across * side[index] * half_width + along * (t * 2.0 - 1.0) * half_width;
    clip = vec4<f32>(clip.xy + offset_px * camera.viewport.zw * 2.0 * clip.w, clip.zw);
    var out: EdgeOut;
    out.clip = clip;
    out.world = mix(start_world.xyz, end_world.xyz, t);
    out.along_px = t * length_px + (t * 2.0 - 1.0) * half_width;
    return out;
}

@fragment
fn fs_edge(in: EdgeOut) -> @location(0) vec4<f32> {
    if (is_cut_away(in.world)) {
        discard;
    }
    return object.edge_color;
}

// Edges behind faces, in hidden-line mode: faint dashes.
@fragment
fn fs_hidden_edge(in: EdgeOut) -> @location(0) vec4<f32> {
    let period = max(object.params.x, 1.0) * 6.5;
    if (is_cut_away(in.world) || fract(in.along_px / period) > 0.55) {
        discard;
    }
    return vec4<f32>(object.edge_color.rgb, object.edge_color.a * 0.45);
}

// --- Points -------------------------------------------------------------

struct PointOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) world: vec3<f32>,
};

@vertex
fn vs_point(@builtin(vertex_index) index: u32, @location(0) position: vec3<f32>) -> PointOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    let corner = corners[index];
    let world = object.model * vec4<f32>(position, 1.0);
    var clip = camera.view_proj * world;
    let radius_px = object.params.y * 0.5;
    clip = vec4<f32>(clip.xy + corner * radius_px * camera.viewport.zw * 2.0 * clip.w, clip.zw);
    var out: PointOut;
    out.clip = clip;
    out.local = corner;
    out.world = world.xyz;
    return out;
}

@fragment
fn fs_point(in: PointOut) -> @location(0) vec4<f32> {
    if (dot(in.local, in.local) > 1.0 || is_cut_away(in.world)) {
        discard;
    }
    return object.edge_color;
}
