pub struct Renderer {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,

    node_pipeline: wgpu::RenderPipeline,
    edge_pipeline: wgpu::RenderPipeline,

    node_buffer: wgpu::Buffer,
    edge_vertex_buffer: wgpu::Buffer,

    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,

    node_count: u32,
    edge_vertex_count: u32,
}

impl Renderer {
    pub async fn new(window: &winit::window::Window, graph: &GraphModel) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::default();
        let surface = unsafe { instance.create_surface(window) }.unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await
            .unwrap();

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface.get_capabilities(&adapter).formats[0],
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
        };

        surface.configure(&device, &config);

        // --- Camera ---
        let camera_uniform = CameraUniform {
            view_proj: glam::Mat4::IDENTITY.to_cols_array_2d(),
        };

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
                label: Some("camera layout"),
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
            label: Some("camera bind group"),
        });

        // --- Node Buffer ---
        let node_instances: Vec<NodeInstance> = graph
            .nodes
            .iter()
            .map(|p| NodeInstance {
                position: [p.x, p.y],
            })
            .collect();

        let node_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Node Buffer"),
            contents: bytemuck::cast_slice(&node_instances),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let instances: Vec<NodeInstance> = graph.nodes.iter().map(|p| {
            NodeInstance {
                position: [p.x, p.y],
                value: 0.0,
                _pad: 0.0,
            }
        }).collect();

        let node_instance_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Node Instance Buffer"),
                contents: bytemuck::cast_slice(&instances),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            },
        );

        // --- Edge Vertex Buffer (expanded to line vertices) ---
        let mut edge_vertices = Vec::new();
        for (a, b) in &graph.edges {
            let pa = graph.nodes[*a as usize];
            let pb = graph.nodes[*b as usize];
            edge_vertices.push([pa.x, pa.y]);
            edge_vertices.push([pb.x, pb.y]);
        }

        let quad_vertex_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Quad Vertex Buffer"),
                contents: bytemuck::cast_slice(&quad_vertices),
                usage: wgpu::BufferUsages::VERTEX,
            },
        );

        let quad_index_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Quad Index Buffer"),
                contents: bytemuck::cast_slice(quad_indices),
                usage: wgpu::BufferUsages::INDEX,
            },
        );

        // --- Pipelines created here (omitted for brevity below) ---

        Self {
            surface,
            device,
            queue,
            config,
            node_pipeline,
            edge_pipeline,
            node_buffer,
            edge_vertex_buffer,
            camera_buffer,
            camera_bind_group,
            node_count: node_instances.len() as u32,
            edge_vertex_count: edge_vertices.len() as u32,
        }
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&Default::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            // --- Draw Edges ---
            rpass.set_pipeline(&self.edge_pipeline);
            rpass.set_bind_group(0, &self.camera_bind_group, &[]);
            rpass.set_vertex_buffer(0, self.edge_vertex_buffer.slice(..));
            rpass.draw(0..self.edge_vertex_count, 0..1);

            // --- Draw Nodes ---
            rpass.set_pipeline(&self.node_pipeline);
            rpass.set_bind_group(0, &self.camera_bind_group, &[]);

            rpass.set_vertex_buffer(0, self.quad_vertex_buffer.slice(..));
            rpass.set_vertex_buffer(1, self.node_instance_buffer.slice(..));
            rpass.set_index_buffer(self.quad_index_buffer.slice(..), wgpu::IndexFormat::Uint16);

            rpass.draw_indexed(0..6, 0, 0..self.node_count);
        }

        self.queue.submit(Some(encoder.finish()));
        output.present();

        Ok(())
    }
}
