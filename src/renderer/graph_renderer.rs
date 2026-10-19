use crate::{
    renderer::camera::{Camera, CameraUniform, CameraUniformBinding},
    store::Arcs,
    vertex::Vertex,
};

/// Index value that ends one line strip and starts the next (primitive restart).
const RESTART: u32 = u32::MAX;
const EDGE_COLOR: [f32; 4] = [0.75, 0.9, 0.6, 1.0];

/// GPU buffers for one loaded graph.
struct GraphBuffers {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

/// Draws a graph's edge polylines as line strips through a 2D map camera.
pub struct GraphRenderer {
    pub camera: Camera,
    pub camera_raw: CameraUniform,
    pub uniform: CameraUniformBinding,
    pub pipeline: wgpu::RenderPipeline,
    graph: Option<GraphBuffers>,
}

impl GraphRenderer {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let camera = Camera::default();
        let camera_raw = CameraUniform::from_camera(&camera, 1.0);
        let uniform = CameraUniformBinding::new(device);
        let pipeline = Self::create_pipeline(device, surface_format, &uniform);
        Self {
            camera,
            camera_raw,
            uniform,
            pipeline,
            graph: None,
        }
    }

    /// Uploads every arc as one line strip; replaces any previously loaded graph.
    pub fn set_arcs(&mut self, device: &wgpu::Device, arcs: &Arcs) {
        let vertices: Vec<Vertex> = arcs
            .points
            .iter()
            .map(|&[x, y]| Vertex {
                position: [x, y, 0.0, 1.0],
                color: EDGE_COLOR,
            })
            .collect();
        let mut indices = Vec::with_capacity(arcs.points.len() + arcs.n_edges());
        for edge in arcs.offsets.windows(2) {
            indices.extend(edge[0] as u32..edge[1] as u32);
            indices.push(RESTART);
        }

        let buffer = |label, contents: &[u8], usage| {
            wgpu::util::DeviceExt::create_buffer_init(
                device,
                &wgpu::util::BufferInitDescriptor {
                    label: Some(label),
                    contents,
                    usage,
                },
            )
        };
        self.graph = Some(GraphBuffers {
            vertex_buffer: buffer(
                "Arc Vertex Buffer",
                bytemuck::cast_slice(&vertices),
                wgpu::BufferUsages::VERTEX,
            ),
            index_buffer: buffer(
                "Arc Index Buffer",
                bytemuck::cast_slice(&indices),
                wgpu::BufferUsages::INDEX,
            ),
            index_count: indices.len() as u32,
        });
    }

    pub fn render<'rpass>(&'rpass self, renderpass: &mut wgpu::RenderPass<'rpass>) {
        let Some(graph) = &self.graph else {
            return;
        };
        renderpass.set_pipeline(&self.pipeline);
        renderpass.set_bind_group(0, &self.uniform.bind_group, &[]);
        renderpass.set_vertex_buffer(0, graph.vertex_buffer.slice(..));
        renderpass.set_index_buffer(graph.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        renderpass.draw_indexed(0..graph.index_count, 0, 0..1);
    }

    /// Writes the current camera to its uniform buffer.
    pub fn update(&mut self, queue: &wgpu::Queue, aspect_ratio: f32) {
        self.camera_raw.update_view_proj(&self.camera, aspect_ratio);
        self.uniform.update_buffer(queue, 0, self.camera_raw);
    }

    /// Creates a render pipeline for the scene using the provided device, surface format, and uniform bindings.
    fn create_pipeline(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        uniform: &CameraUniformBinding,
    ) -> wgpu::RenderPipeline {
        // Create the shader module from the WGSL source code
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(include_str!("shader/triangle_shader.wgsl").into()),
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
                topology: wgpu::PrimitiveTopology::LineStrip,
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
