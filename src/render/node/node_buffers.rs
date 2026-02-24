use crate::graph::topology::GraphTopology;
use crate::render::buffers::{NodeInstance, QUAD_INDICES, QUAD_VERTICES};
use wgpu::util::DeviceExt;

pub struct NodeBuffers {
    pub node_instance_buffer: wgpu::Buffer,
    pub quad_vertex_buffer: wgpu::Buffer,
    pub quad_index_buffer: wgpu::Buffer,
    pub node_count: u32,
}

impl NodeBuffers {
    pub fn update_node_instances(&mut self, queue: &wgpu::Queue, graph: &GraphTopology) {
        let instances: Vec<NodeInstance> = graph
            .graph
            .node_weights()
            .map(|p| NodeInstance {
                position: [p.position.x, p.position.y],
                color: p.color,
                _pad: 0.0,
            })
            .collect();
        queue.write_buffer(
            &self.node_instance_buffer,
            0,
            bytemuck::cast_slice(&instances),
        );
        self.node_count = instances.len() as u32;
    }

    pub fn new(device: &wgpu::Device, graph: &GraphTopology) -> Self {
        let instances: Vec<NodeInstance> = graph
            .graph
            .node_weights()
            .map(|p| NodeInstance {
                position: [p.position.x, p.position.y],
                color: p.color,
                _pad: 0.0,
            })
            .collect();
        let node_instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Node Instance Buffer"),
            contents: bytemuck::cast_slice(&instances),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let quad_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Quad Vertex Buffer"),
            contents: bytemuck::cast_slice(&QUAD_VERTICES),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let quad_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Quad Index Buffer"),
            contents: bytemuck::cast_slice(QUAD_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });
        Self {
            node_instance_buffer,
            quad_vertex_buffer,
            quad_index_buffer,
            node_count: instances.len() as u32,
        }
    }
}
