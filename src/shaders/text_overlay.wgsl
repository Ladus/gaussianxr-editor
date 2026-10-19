struct Params {
    screen: vec2<f32>,
    size: vec2<f32>,
    background_alpha: f32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> params: Params;

@group(0) @binding(1)
var glyphs: texture_2d<f32>;

@group(0) @binding(2)
var glyph_sampler: sampler;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
    );

    let corner = corners[vertex_index];
    let offset_px = (corner - vec2<f32>(0.5, 0.5)) * params.size;
    let ndc = vec2<f32>(offset_px.x, -offset_px.y) * 2.0 / params.screen;

    var out: VertexOutput;
    out.position = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = corner;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let coverage = textureSample(glyphs, glyph_sampler, in.uv).r;
    if coverage >= 0.5 {
        return vec4<f32>(1.0, 1.0, 1.0, 1.0);
    }
    return vec4<f32>(0.0, 0.0, 0.0, params.background_alpha);
}
