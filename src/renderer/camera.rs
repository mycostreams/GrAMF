use cgmath::Point3;

#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::from_cols(
    cgmath::Vector4::new(1.0, 0.0, 0.0, 0.0),
    cgmath::Vector4::new(0.0, 1.0, 0.0, 0.0),
    cgmath::Vector4::new(0.0, 0.0, 0.5, 0.0),
    cgmath::Vector4::new(0.0, 0.0, 0.5, 1.0),
);



pub struct Camera {
    pub(crate) position: Point3<f32>,
    pub(crate) target: Point3<f32>,
    pub(crate) up: cgmath::Vector3<f32>,
}   

impl Camera {
    pub fn ortho_view_matrix(&self, aspect_ratio: f32) -> cgmath::Matrix4<f32> {
        let view = cgmath::Matrix4::look_at_rh(self.position, self.target, self.up);
        let proj = cgmath::ortho(-1.0 * aspect_ratio, 1.0 * aspect_ratio    , -1.0, 1.0, -1.0, 100.0);
        OPENGL_TO_WGPU_MATRIX * proj * view
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
