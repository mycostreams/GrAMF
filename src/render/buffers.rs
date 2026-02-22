#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct NodeInstance {
    pub position: [f32; 2],
    pub value: f32, // ready for colormap later
    pub _pad: f32,
}

struct EdgeInstance {
    src: u32,
    trg: u32,
    width: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct QuadVertex {
    pub pos: [f32; 2], // local quad coordinates
}

const quad_vertices: [QuadVertex; 4] = [
    QuadVertex { pos: [-1.0, -1.0] },
    QuadVertex { pos: [ 1.0, -1.0] },
    QuadVertex { pos: [ 1.0,  1.0] },
    QuadVertex { pos: [-1.0,  1.0] },
];

const quad_indices: &[u16] = &[0, 1, 2, 0, 2, 3];