use crate::graph::topology::GraphTopology;
use crate::render::buffers::EdgeVertex;
use glam::Vec2;
use petgraph::visit::EdgeRef;
use wgpu::util::DeviceExt;

pub struct EdgeBuffers {
    pub edge_vertex_buffer: wgpu::Buffer,
    pub edge_index_buffer: wgpu::Buffer,
    pub quad_count: u32,
}

impl EdgeBuffers {
    pub fn new(device: &wgpu::Device, graph: &mut &GraphTopology) -> Self {
        let mut edge_vertices = Vec::new();
        let mut edge_indices = Vec::new();
        let mut quad_count = 0;
        for edge in graph.graph.edge_references() {
            let pa = graph.graph.node_weight(edge.source()).unwrap().position;
            let pb = graph.graph.node_weight(edge.target()).unwrap().position;
            let width = edge.weight().width;
            let color: [f32; 3] = edge.weight().color;
            let dir = (pb - pa).normalize();
            let perp = Vec2::new(-dir.y, dir.x);
            let halfw = width * 0.5;
            // Four quad vertices
            let v0 = pa + perp * halfw;
            let v1 = pa - perp * halfw;
            let v2 = pb - perp * halfw;
            let v3 = pb + perp * halfw;
            let base = quad_count * 4;
            edge_vertices.push(EdgeVertex {
                pos: [v0.x, v0.y],
                width,
                color,
            });
            edge_vertices.push(EdgeVertex {
                pos: [v1.x, v1.y],
                width,
                color,
            });
            edge_vertices.push(EdgeVertex {
                pos: [v2.x, v2.y],
                width,
                color,
            });
            edge_vertices.push(EdgeVertex {
                pos: [v3.x, v3.y],
                width,
                color,
            });
            // Two triangles per quad
            edge_indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            quad_count += 1;
        }
        let edge_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Edge Quad Vertex Buffer"),
            contents: bytemuck::cast_slice(&edge_vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let edge_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Edge Quad Index Buffer"),
            contents: bytemuck::cast_slice(&edge_indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        Self {
            edge_vertex_buffer,
            edge_index_buffer,
            quad_count,
        }
    }

    /// Update edge vertex buffer with new color/width data from the graph
    pub fn update_edge_vertices(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, graph: &GraphTopology) {
        use glam::Vec2;
        let mut edge_vertices = Vec::new();
        let mut quad_count = 0;
        for edge in graph.graph.edge_references() {
            let pa = graph.graph.node_weight(edge.source()).unwrap().position;
            let pb = graph.graph.node_weight(edge.target()).unwrap().position;
            let width = edge.weight().width;
            let color: [f32; 3] = edge.weight().color;
            let dir = (pb - pa).normalize();
            let perp = Vec2::new(-dir.y, dir.x);
            let halfw = width * 0.5;
            // Four quad vertices
            let v0 = pa + perp * halfw;
            let v1 = pa - perp * halfw;
            let v2 = pb - perp * halfw;
            let v3 = pb + perp * halfw;
            edge_vertices.push(EdgeVertex {
                pos: [v0.x, v0.y],
                width,
                color,
            });
            edge_vertices.push(EdgeVertex {
                pos: [v1.x, v1.y],
                width,
                color,
            });
            edge_vertices.push(EdgeVertex {
                pos: [v2.x, v2.y],
                width,
                color,
            });
            edge_vertices.push(EdgeVertex {
                pos: [v3.x, v3.y],
                width,
                color,
            });
            quad_count += 1;
        }
        if quad_count != self.quad_count {
            // Recreate buffer with new size
            self.edge_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Edge Quad Vertex Buffer"),
                contents: bytemuck::cast_slice(&edge_vertices),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });
            // Also need to update indices if quad_count changed
            let mut edge_indices = Vec::new();
            for i in 0..quad_count {
                let base = i * 4;
                edge_indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            }
            self.edge_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Edge Quad Index Buffer"),
                contents: bytemuck::cast_slice(&edge_indices),
                usage: wgpu::BufferUsages::INDEX,
            });
        } else {
            queue.write_buffer(
                &self.edge_vertex_buffer,
                0,
                bytemuck::cast_slice(&edge_vertices),
            );
        }
        self.quad_count = quad_count;
    }
}
