use wgpu::{VertexAttribute, VertexBufferLayout, VertexStepMode, vertex_attr_array};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 4],
    pub color: [f32; 4],
}

impl Vertex {
    pub fn vertex_attributes() -> Vec<VertexAttribute> {
        vertex_attr_array![0 => Float32x4, 1 => Float32x4].to_vec()
    }

    pub fn description(attributes: &[VertexAttribute]) -> VertexBufferLayout<'_> {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes,
        }
    }
}

// #[repr(C)]
// #[derive(Default, Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
// pub struct UniformBuffer {
//     pub mvp: nalgebra_glm::Mat4,
// }
