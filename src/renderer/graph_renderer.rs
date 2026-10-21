use crate::{renderer::camera::Camera, renderer::vertex::SegmentInstance, store::Arcs};
use wgpu::util::DeviceExt;

/// How edges are drawn; set from the Layers panel.
#[derive(Clone, Copy)]
pub struct LineStyle {
    /// Thickness in pixels (of an edge at the reference width, when scaling by width).
    pub base_px: f32,
    pub by_width: bool,
    /// Colour edges from the per-edge style table (width colouring) instead of per segment.
    pub edge_colour: bool,
    /// Width (µm) drawn at `base_px`.
    pub reference_width_um: f32,
}

impl Default for LineStyle {
    fn default() -> Self {
        Self { base_px: 1.5, by_width: false, edge_colour: false, reference_width_um: 0.0 }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    mvp: [[f32; 4]; 4],
    time: [f32; 4],
    view: [f32; 4],
    style: [f32; 4],
}

/// Selection halo and evidence thickness (pixels; negative = styled thickness plus this).
const SELECTION_PX: f32 = -2.0;
const EVIDENCE_PX: f32 = 2.5;

/// One instance buffer of segments.
struct Segments {
    buffer: wgpu::Buffer,
    count: u32,
}

/// Draws a graph's arcs as thick segments through a 2D map camera.
pub struct GraphRenderer {
    pub camera: Camera,
    uniform: Uniform,
    uniform_buffer: wgpu::Buffer,
    /// Per edge `[r, g, b, width_um]` at the current timepoint.
    style_buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
    graph: Option<Segments>,
    /// Matcher evidence (developer view), drawn over the graph.
    evidence: Option<Segments>,
    /// Selected edges, drawn again on top in the highlight colour.
    selection: Option<Segments>,
}

impl GraphRenderer {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let uniform = Uniform {
            mvp: cgmath::Matrix4::from_scale(1.0).into(),
            time: [f32::MAX, 0.0, 0.0, 0.0],
            view: [1.0, 1.0, 1.5, 0.0],
            style: [0.0; 4],
        };
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Graph Uniform"),
            contents: bytemuck::cast_slice(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Graph Bind Group Layout"),
            entries: &[
                entry(0, wgpu::BufferBindingType::Uniform),
                entry(1, wgpu::BufferBindingType::Storage { read_only: true }),
            ],
        });
        let style_buffer = style_buffer(device, 1);
        let bind_group = bind_group(device, &bind_group_layout, &uniform_buffer, &style_buffer);
        let pipeline = create_pipeline(device, surface_format, &bind_group_layout);
        Self {
            camera: Camera::default(),
            uniform,
            uniform_buffer,
            style_buffer,
            bind_group_layout,
            bind_group,
            pipeline,
            graph: None,
            evidence: None,
            selection: None,
        }
    }

    /// Replaces the graph. `vertex_t` and `colours` are row-aligned with `arcs.points`.
    pub fn set_arcs(&mut self, device: &wgpu::Device, arcs: &Arcs, vertex_t: &[f32], colours: &[[f32; 4]]) {
        self.graph = Some(segments(device, arcs, vertex_t, 0..arcs.n_edges(), |v| colours[v], 0.0));
        self.selection = None;
        if self.style_buffer.size() != (arcs.n_edges().max(1) * 16) as u64 {
            self.style_buffer = style_buffer(device, arcs.n_edges());
            self.bind_group = bind_group(device, &self.bind_group_layout, &self.uniform_buffer, &self.style_buffer);
        }
    }

    /// Per-edge `[r, g, b, width_um]` for the current timepoint (width <= 0: unknown).
    pub fn set_edge_styles(&self, queue: &wgpu::Queue, styles: &[[f32; 4]]) {
        if !styles.is_empty() && (styles.len() * 16) as u64 <= self.style_buffer.size() {
            queue.write_buffer(&self.style_buffer, 0, bytemuck::cast_slice(styles));
        }
    }

    /// Highlights `edges` (empty clears the selection).
    pub fn set_selection(&mut self, device: &wgpu::Device, arcs: &Arcs, vertex_t: &[f32], edges: &[usize]) {
        self.selection = (!edges.is_empty()).then(|| {
            segments(device, arcs, vertex_t, edges.iter().copied(), |_| crate::colour::HIGHLIGHT, SELECTION_PX)
        });
    }

    /// Draws `(edge, s0, s1, colour)` spans — the part of each edge whose arc length lies in
    /// `[s0, s1]` µm — regardless of the current time. Empty clears.
    pub fn set_evidence(&mut self, device: &wgpu::Device, arcs: &Arcs, spans: &[(usize, f32, f32, [f32; 4])]) {
        let mut out = Vec::new();
        for &(e, s0, s1, colour) in spans {
            let mut s = 0.0_f32;
            for v in arcs.offsets[e]..arcs.offsets[e + 1].saturating_sub(1) {
                let (a, b) = (arcs.points[v], arcs.points[v + 1]);
                let s_next = s + ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
                if s >= s0 && s_next <= s1 {
                    out.push(SegmentInstance {
                        p0: a,
                        p1: b,
                        t: [f32::NEG_INFINITY; 2],
                        colour,
                        edge: e as u32,
                        fixed_px: EVIDENCE_PX,
                    });
                }
                s = s_next;
            }
        }
        self.evidence = (!out.is_empty()).then(|| upload(device, &out));
    }

    pub fn render<'rpass>(&'rpass self, renderpass: &mut wgpu::RenderPass<'rpass>) {
        renderpass.set_pipeline(&self.pipeline);
        renderpass.set_bind_group(0, &self.bind_group, &[]);
        for s in [&self.graph, &self.evidence, &self.selection].into_iter().flatten() {
            renderpass.set_vertex_buffer(0, s.buffer.slice(..));
            renderpass.draw(0..4, 0..s.count);
        }
    }

    /// Writes the camera, timepoint and line style for a viewport of `size_px`.
    pub fn update(&mut self, queue: &wgpu::Queue, size_px: [f32; 2], t: f32, line: LineStyle) {
        let flag = |b: bool| if b { 1.0 } else { 0.0 };
        self.uniform.mvp = self.camera.ortho_view_matrix(size_px[0] / size_px[1].max(1.0)).into();
        self.uniform.time[0] = t;
        self.uniform.view = [size_px[0], size_px[1], line.base_px, flag(line.by_width)];
        self.uniform.style = [flag(line.edge_colour), line.reference_width_um, 0.0, 0.0];
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[self.uniform]));
    }
}

fn style_buffer(device: &wgpu::Device, n_edges: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Edge Styles"),
        size: (n_edges.max(1) * 16) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniform: &wgpu::Buffer,
    styles: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Graph Bind Group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: styles.as_entire_binding() },
        ],
    })
}

/// One instance per consecutive vertex pair of every edge in `edges`.
// ponytail: 48 bytes per segment (~110 MB for a 2.3M-point plate); pack to f16 if VRAM bites
fn segments(
    device: &wgpu::Device,
    arcs: &Arcs,
    vertex_t: &[f32],
    edges: impl Iterator<Item = usize>,
    colour: impl Fn(usize) -> [f32; 4],
    fixed_px: f32,
) -> Segments {
    let mut out = Vec::with_capacity(arcs.points.len());
    for e in edges {
        for v in arcs.offsets[e]..arcs.offsets[e + 1].saturating_sub(1) {
            out.push(SegmentInstance {
                p0: arcs.points[v],
                p1: arcs.points[v + 1],
                t: [vertex_t[v], vertex_t[v + 1]],
                colour: colour(v),
                edge: e as u32,
                fixed_px,
            });
        }
    }
    upload(device, &out)
}

fn upload(device: &wgpu::Device, instances: &[SegmentInstance]) -> Segments {
    // An empty buffer is not allowed; one zeroed instance with count 0 draws nothing.
    let zero = [<SegmentInstance as bytemuck::Zeroable>::zeroed()];
    let contents = if instances.is_empty() { &zero[..] } else { instances };
    Segments {
        buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Segments"),
            contents: bytemuck::cast_slice(contents),
            usage: wgpu::BufferUsages::VERTEX,
        }),
        count: instances.len() as u32,
    }
}

fn create_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
    layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("segments.wgsl"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shader/segments.wgsl").into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[Some(layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Segments"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vertex_main"),
            buffers: &[SegmentInstance::layout()],
            compilation_options: Default::default(),
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Cw,
            cull_mode: None,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
            unclipped_depth: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            // no depth writes: everything is at z = 0 and later draws (selection) go on top
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::Always),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState { count: 1, mask: !0, alpha_to_coverage_enabled: false },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
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
