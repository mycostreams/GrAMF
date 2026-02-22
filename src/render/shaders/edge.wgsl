struct Camera {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

struct VSIn {
    @location(0) pos: vec2<f32>,
};

struct VSOut {
    @builtin(position) clip_pos: vec4<f32>,
};

@vertex
fn vs_main(input: VSIn) -> VSOut {
    var out: VSOut;
    out.clip_pos = camera.view_proj * vec4<f32>(input.pos, 0.0, 1.0);
    return out;
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(0.5, 0.5, 0.5, 1.0);
}
