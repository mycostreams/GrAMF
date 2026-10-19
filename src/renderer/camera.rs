#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::from_cols(
    cgmath::Vector4::new(1.0, 0.0, 0.0, 0.0),
    cgmath::Vector4::new(0.0, 1.0, 0.0, 0.0),
    cgmath::Vector4::new(0.0, 0.0, 0.5, 0.0),
    cgmath::Vector4::new(0.0, 0.0, 0.5, 1.0),
);

/// 2D orthographic map camera in µm. World y is the image row, so it grows downward on
/// screen like the plate images. Positions stay 3D so a 3D camera can be added later.
#[derive(Debug, Clone)]
pub struct Camera {
    pub center: [f32; 2],
    /// Half the visible height in µm.
    pub half_height: f32,
}

impl Camera {
    pub fn default() -> Self {
        Self {
            center: [0.0, 0.0],
            half_height: 1.0,
        }
    }

    pub fn ortho_view_matrix(&self, aspect_ratio: f32) -> cgmath::Matrix4<f32> {
        let [cx, cy] = self.center;
        let (hw, hh) = (self.half_height * aspect_ratio, self.half_height);
        // bottom = cy + hh: flips y so rows grow downward
        let proj = cgmath::ortho(cx - hw, cx + hw, cy + hh, cy - hh, -1.0, 1.0);
        OPENGL_TO_WGPU_MATRIX * proj
    }

    /// µm per physical pixel for a viewport `height_px` tall.
    fn scale(&self, height_px: f32) -> f32 {
        2.0 * self.half_height / height_px
    }

    pub fn fit(&mut self, min: [f32; 2], max: [f32; 2], aspect_ratio: f32) {
        self.center = [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0];
        let half = ((max[1] - min[1]) / 2.0).max((max[0] - min[0]) / 2.0 / aspect_ratio);
        self.half_height = half.max(1.0) * 1.05;
    }

    /// Moves the map with a cursor drag of `(dx, dy)` physical pixels.
    pub fn pan(&mut self, dx: f32, dy: f32, height_px: f32) {
        let s = self.scale(height_px);
        self.center[0] -= dx * s;
        self.center[1] -= dy * s;
    }

    /// Zooms by `factor` (<1 zooms in), keeping the world point under `cursor` fixed.
    pub fn zoom(&mut self, factor: f32, cursor: [f32; 2], size_px: [f32; 2]) {
        let offset = [cursor[0] - size_px[0] / 2.0, cursor[1] - size_px[1] / 2.0];
        let s = self.scale(size_px[1]);
        let world = [self.center[0] + offset[0] * s, self.center[1] + offset[1] * s];
        self.half_height *= factor;
        let s = self.scale(size_px[1]);
        self.center = [world[0] - offset[0] * s, world[1] - offset[1] * s];
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    view_proj: [[f32; 4]; 4],
}

impl CameraUniform {
    pub fn new() -> Self {
        use cgmath::SquareMatrix;
        Self {
            view_proj: cgmath::Matrix4::identity().into(),
        }
    }
    pub fn from_camera(camera: &Camera, aspect_ratio: f32) -> Self {
        let view_proj = camera.ortho_view_matrix(aspect_ratio);
        Self {
            view_proj: view_proj.into(),
        }
    }

    pub fn update_view_proj(&mut self, camera: &Camera, aspect_ratio: f32) {
        self.view_proj = camera.ortho_view_matrix(aspect_ratio).into();
    }
}

use wgpu::util::DeviceExt;

/// A helper struct to manage uniform buffers and their associated bind groups.
pub struct CameraUniformBinding {
    pub buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub bind_group_layout: wgpu::BindGroupLayout,
}

impl CameraUniformBinding {
    pub fn new(device: &wgpu::Device) -> Self {
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::cast_slice(&[CameraUniform::new()]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("uniform_bind_group_layout"),
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
            label: Some("uniform_bind_group"),
        });

        Self {
            buffer,
            bind_group,
            bind_group_layout,
        }
    }

    pub fn update_buffer(
        &mut self,
        queue: &wgpu::Queue,
        offset: wgpu::BufferAddress,
        camera_uniform: CameraUniform,
    ) {
        queue.write_buffer(
            &self.buffer,
            offset,
            bytemuck::cast_slice(&[camera_uniform]),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::Camera;

    #[test]
    fn zoom_keeps_point_under_cursor() {
        let size = [800.0, 600.0];
        let cursor = [700.0, 100.0];
        let mut cam = Camera { center: [5000.0, 2000.0], half_height: 300.0 };
        let world = |c: &Camera| {
            let s = 2.0 * c.half_height / size[1];
            [c.center[0] + (cursor[0] - 400.0) * s, c.center[1] + (cursor[1] - 300.0) * s]
        };
        let before = world(&cam);
        cam.zoom(0.5, cursor, size);
        let after = world(&cam);
        assert!((before[0] - after[0]).abs() < 1e-3 && (before[1] - after[1]).abs() < 1e-3);
        assert_eq!(cam.half_height, 150.0);
    }
}
