use crate::graph::topology::GraphTopology;
use crate::render::buffers::CameraUniform;
use crate::render::edge::edge_buffers::EdgeBuffers;
use crate::render::node::node_buffers::NodeBuffers;
use wgpu::util::DeviceExt;

pub struct GraphRenderer {
    pub node_pipeline: wgpu::RenderPipeline,
    pub edge_pipeline: wgpu::RenderPipeline,

    pub node_buffers: NodeBuffers,
    pub edge_buffers: EdgeBuffers,

    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group: wgpu::BindGroup,
}

impl GraphRenderer {
    pub fn new(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        mut graph: &GraphTopology,
    ) -> Self {
        // --- Camera setup ---
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

        // --- Node setup ---
        let node_buffers = NodeBuffers::new(device, graph);
        let node_pipeline =
            super::node::node_pipeline::node_pipeline(device, config, &camera_bind_group_layout);

        // --- Edge setup ---
        let edge_buffers = EdgeBuffers::new(device, &mut graph);
        let edge_pipeline =
            super::edge::edge_pipeline::edge_pipeline(device, config, camera_bind_group_layout);

        Self {
            node_pipeline,
            edge_pipeline,
            node_buffers,
            edge_buffers,
            camera_buffer,
            camera_bind_group,
        }
    }

    pub fn render(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Graph Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            // --- Draw Edges ---
            rpass.set_pipeline(&self.edge_pipeline);
            rpass.set_bind_group(0, &self.camera_bind_group, &[]);

            rpass.set_vertex_buffer(0, self.edge_buffers.edge_vertex_buffer.slice(..));
            rpass.set_index_buffer(
                self.edge_buffers.edge_index_buffer.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            rpass.draw_indexed(0..(self.edge_buffers.quad_count * 6), 0, 0..1);

            // --- Draw Nodes ---
            rpass.set_pipeline(&self.node_pipeline);
            rpass.set_bind_group(0, &self.camera_bind_group, &[]);

            rpass.set_vertex_buffer(0, self.node_buffers.quad_vertex_buffer.slice(..));
            rpass.set_vertex_buffer(1, self.node_buffers.node_instance_buffer.slice(..));
            rpass.set_index_buffer(
                self.node_buffers.quad_index_buffer.slice(..),
                wgpu::IndexFormat::Uint16,
            );
            rpass.draw_indexed(0..6, 0, 0..self.node_buffers.node_count);
        }
    }
}
