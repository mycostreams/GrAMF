struct Camera {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

struct VSIn {
    @location(0) quad_pos: vec2<f32>,
    @location(1) node_pos: vec2<f32>,
    @location(2) color: vec3<f32>,
};

struct VSOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) color: vec3<f32>,
};

@vertex
fn vs_main(input: VSIn) -> VSOut {
    var out: VSOut;
    let world_pos = input.node_pos + input.quad_pos;
    out.clip_pos = camera.view_proj * vec4<f32>(world_pos, 0.0, 1.0);
    out.color = input.color;
    return out;
}

@fragment
fn fs_main(input: VSOut) -> @location(0) vec4<f32> {
    // Output supplied RGB color directly
    return vec4<f32>(input.color, 1.0);
}
