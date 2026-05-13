use crate::{
    uniform_binding::UniformBinding,
    vertex::{UniformBuffer, Vertex},
};

/// A struct representing the 3D scene, including its model matrix, vertex/index buffers, uniform bindings, and render pipeline.
pub struct Scene {
    pub model: nalgebra_glm::Mat4,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub uniform: UniformBinding,
    pub pipeline: wgpu::RenderPipeline,
}

impl Scene {
    /// Creates a new `Scene` instance, initializing the vertex/index buffers, uniform bindings, and render pipeline.
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let vertex_buffer = wgpu::util::DeviceExt::create_buffer_init(
            device,
            &wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(&crate::constants::VERTICES),
                usage: wgpu::BufferUsages::VERTEX,
            },
        );
        let index_buffer = wgpu::util::DeviceExt::create_buffer_init(
            device,
            &wgpu::util::BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: bytemuck::cast_slice(&crate::constants::INDICES),
                usage: wgpu::BufferUsages::INDEX,
            },
        );
        let uniform = UniformBinding::new(device);
        let pipeline = Self::create_pipeline(device, surface_format, &uniform);
        Self {
            model: nalgebra_glm::Mat4::identity(),
            uniform,
            pipeline,
            vertex_buffer,
            index_buffer,
        }
    }

    /// Renders the scene using the provided render pass, setting the pipeline, bind groups, and vertex/index buffers.
    pub fn render<'rpass>(&'rpass self, renderpass: &mut wgpu::RenderPass<'rpass>) {
        renderpass.set_pipeline(&self.pipeline);
        renderpass.set_bind_group(0, &self.uniform.bind_group, &[]);

        renderpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        renderpass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

        renderpass.draw_indexed(0..(crate::constants::INDICES.len() as _), 0, 0..1);
    }

    /// Updates the scene's model matrix and uniform buffer based on the aspect ratio and delta time.
    pub fn update(&mut self, queue: &wgpu::Queue, aspect_ratio: f32, delta_time: f32) {
        let projection =
            nalgebra_glm::perspective_lh_zo(aspect_ratio, 80_f32.to_radians(), 0.1, 1000.0);
        let view = nalgebra_glm::look_at_lh(
            &nalgebra_glm::vec3(0.0, 0.0, 3.0),
            &nalgebra_glm::vec3(0.0, 0.0, 0.0),
            &nalgebra_glm::Vec3::y(),
        );
        self.model = nalgebra_glm::rotate(
            &self.model,
            30_f32.to_radians() * delta_time,
            &nalgebra_glm::Vec3::y(),
        );
        self.uniform.update_buffer(
            queue,
            0,
            UniformBuffer {
                mvp: projection * view * self.model,
            },
        );
    }

    /// Creates a render pipeline for the scene using the provided device, surface format, and uniform bindings.
    fn create_pipeline(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        uniform: &UniformBinding,
    ) -> wgpu::RenderPipeline {
        // Create the shader module from the WGSL source code
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(
                crate::constants::SHADER_SOURCE,
            )),
        });

        // Create the pipeline layout, specifying the bind group layout for the uniform buffer
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&uniform.bind_group_layout)],
            immediate_size: 0,
        });

        // Create the render pipeline, specifying the vertex and fragment stages, primitive state, depth/stencil state, and multisampling
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vertex_main"),
                buffers: &[Vertex::description(&Vertex::vertex_attributes())],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: Some(wgpu::IndexFormat::Uint32),
                front_face: wgpu::FrontFace::Cw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
                unclipped_depth: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: Some("fragment_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview_mask: None,
            cache: None,
        })
    }
}
