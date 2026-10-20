// Arc segments as screen-space quads: thickness in pixels, optionally scaled by the edge's
// width at the current timepoint, with a one-pixel antialiased fringe.

struct Uniform {
    mvp: mat4x4<f32>,
    // x: current timepoint index; geometry appearing later is hidden
    time: vec4<f32>,
    // xy: viewport size in pixels; z: base thickness in pixels; w: 1 = scale thickness by width
    view: vec4<f32>,
    // x: 1 = take colour from edge_style; y: reference width (µm) drawn at the base thickness
    style: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniform;
// Per edge: rgb colour (width mode), and width in µm at the current timepoint (<= 0: unknown)
@group(0) @binding(1) var<storage, read> edge_style: array<vec4<f32>>;

struct Segment {
    @location(0) p0: vec2<f32>,
    @location(1) p1: vec2<f32>,
    @location(2) t: vec2<f32>,
    @location(3) colour: vec4<f32>,
    @location(4) edge: u32,
    @location(5) fixed_px: f32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) colour: vec4<f32>,
    // > 0 where the geometry has not appeared yet; interpolates to cut arcs mid-segment
    @location(1) not_yet: f32,
    // signed distance from the centre line, and the half thickness, in pixels
    @location(2) across: f32,
    @location(3) half_px: f32,
};

@vertex
fn vertex_main(@builtin(vertex_index) vi: u32, s: Segment) -> VertexOutput {
    // strip order: (p0,-), (p1,-), (p0,+), (p1,+)
    let along = f32(vi & 1u);
    let side = f32(vi >> 1u) * 2.0 - 1.0;

    let style = edge_style[s.edge];
    var styled_px = u.view.z;
    if u.view.w > 0.5 && style.w > 0.0 && u.style.y > 0.0 {
        styled_px = clamp(u.view.z * style.w / u.style.y, 0.5, 16.0);
    }
    var px = styled_px;
    var colour = s.colour;
    if s.fixed_px > 0.0 {
        px = s.fixed_px;
    } else if s.fixed_px < 0.0 {
        px = styled_px - s.fixed_px;
    } else if u.style.x > 0.5 {
        colour = vec4<f32>(style.rgb, 1.0);
    }

    let c0 = u.mvp * vec4<f32>(s.p0, 0.0, 1.0);
    let c1 = u.mvp * vec4<f32>(s.p1, 0.0, 1.0);
    let d_px = (c1.xy - c0.xy) * u.view.xy * 0.5;
    var dir = vec2<f32>(1.0, 0.0);
    if length(d_px) > 1e-6 {
        dir = normalize(d_px);
    }
    let normal = vec2<f32>(-dir.y, dir.x);
    let half_px = 0.5 * px;
    let reach = half_px + 1.0; // room for the antialiased fringe
    let to_ndc = 2.0 / u.view.xy;
    // extend each end by the half thickness so consecutive segments overlap at joints
    let offset = (normal * side * reach + dir * (along * 2.0 - 1.0) * reach) * to_ndc;

    var out: VertexOutput;
    let base = mix(c0, c1, along);
    out.position = vec4<f32>(base.xy + offset, base.z, base.w);
    out.colour = colour;
    out.not_yet = mix(s.t.x, s.t.y, along) - u.time.x;
    out.across = side * reach;
    out.half_px = half_px;
    return out;
}

@fragment
fn fragment_main(in: VertexOutput) -> @location(0) vec4<f32> {
    if in.not_yet > 0.0 {
        discard;
    }
    let coverage = clamp(in.half_px + 0.5 - abs(in.across), 0.0, 1.0);
    if coverage <= 0.0 {
        discard;
    }
    return vec4<f32>(in.colour.rgb, in.colour.a * coverage);
}
