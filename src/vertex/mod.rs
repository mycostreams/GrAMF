pub mod data;

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

pub struct EdgeInstance {
    pub start_position: cgmath::Vector3<f32>,
    pub end_position: cgmath::Vector3<f32>,
    pub color: cgmath::Vector4<f32>,
    pub width: f32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct EdgeInstanceRaw {
    pub start_position: [f32; 3],
    pub end_position: [f32; 3],
    pub color: [f32; 4],
    _padding: [f32; 3], // Padding to ensure 16-byte alignment for the width field
    pub width: f32,
}

impl EdgeInstance {
    fn to_raw(&self) -> EdgeInstanceRaw {
        EdgeInstanceRaw {
            start_position: self.start_position.into(),
            end_position: self.end_position.into(),
            color: self.color.into(),
            _padding: [0.0; 3],
            width: self.width,
        }
    }
}

impl EdgeInstanceRaw {
    pub fn vertex_attributes() -> Vec<VertexAttribute> {
        vertex_attr_array![
            2 => Float32x3, // start_position
            3 => Float32x3, // end_position
            4 => Float32x4, // color
            5 => Float32,   // width
        ]
        .to_vec()
    }

    pub fn description(attributes: &[VertexAttribute]) -> VertexBufferLayout<'_> {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<EdgeInstanceRaw>() as wgpu::BufferAddress,
            step_mode: VertexStepMode::Instance,
            attributes,
        }
    }
}

// #[repr(C)]
// #[derive(Default, Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
// pub struct UniformBuffer {
//     pub mvp: nalgebra_glm::Mat4,
// }
