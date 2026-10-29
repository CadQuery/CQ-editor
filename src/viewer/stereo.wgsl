// Composition of the two eyes of the stereo projection into one image.
// `viewer/stereo.rs` composes the same way on the CPU for captures, and the
// tests compare the two.
//
// Both eyes are read texel by texel (no filtering). The textures are sRGB,
// so the anaglyph mixes in linear light, and the sRGB target encodes again.

struct Compose {
    // 1 anaglyph, 2 rows, 3 columns, 4 chess board, 5 side by side,
    // 6 over-under.
    mode: u32,
    // Size of each eye's image, in pixels.
    eye_width: u32,
    eye_height: u32,
    padding: u32,
};

@group(0) @binding(0) var left: texture_2d<f32>;
@group(0) @binding(1) var right: texture_2d<f32>;
@group(0) @binding(2) var<uniform> compose: Compose;

@vertex
fn vs_compose(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    // One triangle covering the whole viewport.
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs_compose(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(floor(position.xy));
    let eye = vec2<i32>(i32(compose.eye_width), i32(compose.eye_height));
    switch compose.mode {
        case 1u: {
            let l = textureLoad(left, p, 0);
            let r = textureLoad(right, p, 0);
            // Optimized red-cyan: the left eye's red is made of its green
            // and blue, so red objects do not flicker between the eyes.
            return vec4<f32>(0.7 * l.g + 0.3 * l.b, r.g, r.b, max(l.a, r.a));
        }
        case 2u: {
            if (p.y % 2 == 0) {
                return textureLoad(left, p, 0);
            }
            return textureLoad(right, p, 0);
        }
        case 3u: {
            if (p.x % 2 == 0) {
                return textureLoad(left, p, 0);
            }
            return textureLoad(right, p, 0);
        }
        case 4u: {
            if ((p.x + p.y) % 2 == 0) {
                return textureLoad(left, p, 0);
            }
            return textureLoad(right, p, 0);
        }
        case 5u: {
            if (p.x < eye.x) {
                return textureLoad(left, p, 0);
            }
            return textureLoad(right, vec2<i32>(p.x - eye.x, p.y), 0);
        }
        default: {
            if (p.y < eye.y) {
                return textureLoad(left, p, 0);
            }
            return textureLoad(right, vec2<i32>(p.x, p.y - eye.y), 0);
        }
    }
}
