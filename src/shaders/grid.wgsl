struct Params {
    screen: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> params: Params;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(3.0, 1.0),
        vec2<f32>(-1.0, 1.0),
    );
    return vec4<f32>(positions[vertex_index], 0.0, 1.0);
}

// Rule-of-thirds grid: white line with a dark halo so it shows on any background.
@fragment
fn fs_main(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let third = params.screen / 3.0;
    let dx = min(abs(frag.x - third.x), abs(frag.x - 2.0 * third.x));
    let dy = min(abs(frag.y - third.y), abs(frag.y - 2.0 * third.y));
    let d = min(dx, dy);

    if d >= 2.0 {
        discard;
    }
    if d < 1.0 {
        return vec4<f32>(1.0, 1.0, 1.0, 0.8);
    }
    return vec4<f32>(0.0, 0.0, 0.0, 0.4);
}
