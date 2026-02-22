struct Camera {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

struct VSIn {
    @location(0) quad_pos: vec2<f32>,
    @location(1) instance_pos: vec2<f32>,
    @location(2) value: f32,
};

struct VSOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) local_pos: vec2<f32>,
    @location(1) value: f32,
};

@vertex
fn vs_main(input: VSIn) -> VSOut {
    var out: VSOut;

    let radius = 0.02;

    let world_pos = input.instance_pos + input.quad_pos * radius;

    out.clip_pos = camera.view_proj * vec4<f32>(world_pos, 0.0, 1.0);
    out.local_pos = input.quad_pos;
    out.value = input.value;

    return out;
}

@fragment
fn fs_main(input: VSOut) -> @location(0) vec4<f32> {
    let dist = length(input.local_pos);

    if (dist > 1.0) {
        discard;
    }

    return vec4<f32>(0.2, 0.7, 1.0, 1.0);
}