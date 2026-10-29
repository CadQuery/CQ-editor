// Copies the view's sRGB texture into a plain 8-bit one for egui, which
// expects gamma-encoded colors. Only for devices that cannot view an sRGB
// texture as plain (the OpenGL backend): elsewhere egui reads the sRGB
// texture through a plain view and this pass is not needed.
//
// Reading the sRGB texture decodes to linear light, so the shader encodes
// again by hand; the plain target stores that unchanged.

@group(0) @binding(0) var scene: texture_2d<f32>;

@vertex
fn vs_present(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    // One triangle covering the whole viewport.
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

fn encode(linear: vec3<f32>) -> vec3<f32> {
    let low = linear * 12.92;
    let high = 1.055 * pow(linear, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(high, low, linear <= vec3<f32>(0.0031308));
}

@fragment
fn fs_present(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let color = textureLoad(scene, vec2<i32>(floor(position.xy)), 0);
    return vec4<f32>(encode(color.rgb), color.a);
}
