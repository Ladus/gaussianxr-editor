struct Params {
    view_proj: mat4x4<f32>,
    screen: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> params: Params;

const NEAR_W: f32 = 0.001;
const DOT_RADIUS_PX: f32 = 2.5;

// One screen-aligned square per selected splat center.
@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @location(0) position: vec3<f32>,
) -> VertexOutput {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(1.0, 1.0),
    );

    var out: VertexOutput;
    let clip = params.view_proj * vec4<f32>(position, 1.0);
    if clip.w < NEAR_W {
        out.position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
        return out;
    }

    let half_screen = params.screen.xy * 0.5;
    let ndc = clip.xy / clip.w + corners[vertex_index] * DOT_RADIUS_PX / half_screen;
    out.position = vec4<f32>(ndc, 0.0, 1.0);
    return out;
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.05, 0.05, 1.0);
}
