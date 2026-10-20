use wgpu::{VertexAttribute, VertexBufferLayout, VertexStepMode, vertex_attr_array};

/// One straight piece of an arc, drawn as a screen-space quad (4 vertices, triangle strip)
/// by `segments.wgsl`, so lines can be thicker than one pixel.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SegmentInstance {
    /// End points in µm.
    pub p0: [f32; 2],
    pub p1: [f32; 2],
    /// Timepoint index at which each end appears.
    pub t: [f32; 2],
    pub colour: [f32; 4],
    /// Row in the per-edge style table (current width, and colour in width mode).
    pub edge: u32,
    /// 0: thickness from the line style; > 0: this many pixels; < 0: the styled thickness
    /// plus this many pixels (a halo around the edge, for highlights). Non-zero also keeps
    /// `colour` instead of the per-edge colour.
    pub fixed_px: f32,
}

impl SegmentInstance {
    pub fn layout() -> VertexBufferLayout<'static> {
        const ATTRIBUTES: [VertexAttribute; 6] =
            vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x2, 3 => Float32x4, 4 => Uint32, 5 => Float32];
        VertexBufferLayout {
            array_stride: std::mem::size_of::<SegmentInstance>() as wgpu::BufferAddress,
            step_mode: VertexStepMode::Instance,
            attributes: &ATTRIBUTES,
        }
    }
}
