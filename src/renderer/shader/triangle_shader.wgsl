//Projection matrix
struct Uniform {
    mvp: mat4x4<f32>,
};

// Bind the uniform buffer to group 0, binding 0
@group(0) @binding(0)
var<uniform> ubo: Uniform;

// Vertex input, defined by position and color
struct VertexInput {
    @location(0) position: vec4<f32>,
    @location(1) color: vec4<f32>,
};

//Vertex output, defined by position and color
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

// Vertex shader: Transforms vertex positions using the MVP matrix
@vertex
fn vertex_main(vert: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.color = vert.color;
    out.position = ubo.mvp * vert.position;
    return out;
};

// Fragment shader: Outputs the interpolated color from the vertex shader
@fragment
fn fragment_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color);
}
