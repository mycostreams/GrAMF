#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct EdgeVertex {
    pub pos: [f32; 2],
    pub width: f32,
    pub color: f32, // to be mapped by colormap later
}
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct NodeInstance {
    pub position: [f32; 2],
    pub value: f32, // ready for colormap later
    pub _pad: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct QuadVertex {
    pub pos: [f32; 2], // local quad coordinates
}

pub const QUAD_VERTICES: [QuadVertex; 4] = [
    QuadVertex { pos: [-1.0, -1.0] },
    QuadVertex { pos: [1.0, -1.0] },
    QuadVertex { pos: [1.0, 1.0] },
    QuadVertex { pos: [-1.0, 1.0] },
];

pub const QUAD_INDICES: &[u16] = &[0, 1, 2, 0, 2, 3];
