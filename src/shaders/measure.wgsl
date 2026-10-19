struct Params {
    view_proj: mat4x4<f32>,
    point_a: vec4<f32>,
    point_b: vec4<f32>,
    screen: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) @interpolate(flat) kind: u32,
};

@group(0) @binding(0)
var<uniform> params: Params;

const NEAR_W: f32 = 0.001;
const LINE_HALF_WIDTH_PX: f32 = 1.5;
const DOT_RADIUS_PX: f32 = 7.0;

fn hidden(kind: u32) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    out.local = vec2<f32>(0.0, 0.0);
    out.kind = kind;
    return out;
}

// Instance 0 is the line between the points, 1 and 2 are the point markers.
@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance: u32,
) -> VertexOutput {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(1.0, 1.0),
    );

    let corner = corners[vertex_index];
    let half_screen = params.screen.xy * 0.5;

    var out: VertexOutput;
    out.local = corner;
    out.kind = instance;

    if instance == 0u {
        if params.point_a.w < 0.5 || params.point_b.w < 0.5 {
            return hidden(instance);
        }

        var ca = params.view_proj * vec4<f32>(params.point_a.xyz, 1.0);
        var cb = params.view_proj * vec4<f32>(params.point_b.xyz, 1.0);
        if ca.w < NEAR_W && cb.w < NEAR_W {
            return hidden(instance);
        }
        // Trim the end that is behind the camera so the line still reaches the screen edge.
        if ca.w < NEAR_W {
            ca = mix(ca, cb, (NEAR_W - ca.w) / (cb.w - ca.w));
        }
        if cb.w < NEAR_W {
            cb = mix(cb, ca, (NEAR_W - cb.w) / (ca.w - cb.w));
        }

        let na = ca.xy / ca.w;
        let nb = cb.xy / cb.w;
        let delta = (nb - na) * half_screen;
        let dir = delta / max(length(delta), 0.0001);
        let normal = vec2<f32>(-dir.y, dir.x);

        let base = select(na, nb, corner.x > 0.0);
        let offset = normal * corner.y * LINE_HALF_WIDTH_PX / half_screen;
        out.position = vec4<f32>(base + offset, 0.0, 1.0);
        return out;
    }

    let point = select(params.point_a, params.point_b, instance == 2u);
    if point.w < 0.5 {
        return hidden(instance);
    }

    let clip = params.view_proj * vec4<f32>(point.xyz, 1.0);
    if clip.w < NEAR_W {
        return hidden(instance);
    }

    let ndc = clip.xy / clip.w + corner * DOT_RADIUS_PX / half_screen;
    out.position = vec4<f32>(ndc, 0.0, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    if in.kind == 0u {
        return vec4<f32>(1.0, 0.85, 0.1, 1.0);
    }

    let d = length(in.local);
    if d > 1.0 {
        discard;
    }
    if d > 0.7 {
        return vec4<f32>(1.0, 1.0, 1.0, 1.0);
    }
    if in.kind == 1u {
        return vec4<f32>(0.2, 0.9, 0.3, 1.0);
    }
    return vec4<f32>(1.0, 0.3, 0.3, 1.0);
}
