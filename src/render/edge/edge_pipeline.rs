pub(crate) fn edge_pipeline(
    device: &wgpu::Device,
    config: &wgpu::wgt::SurfaceConfiguration<Vec<wgpu::TextureFormat>>,
    camera_bind_group_layout: wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let edge_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Edge Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("edge.wgsl").into()),
    });
    let edge_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Edge Pipeline Layout"),
        bind_group_layouts: &[&camera_bind_group_layout],
        push_constant_ranges: &[],
    });
    let edge_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Edge Pipeline"),
        layout: Some(&edge_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &edge_shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<crate::render::buffers::EdgeVertex>()
                    as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32, 2 => Float32],
            }],
        },
        fragment: Some(wgpu::FragmentState {
            module: &edge_shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: config.format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });
    edge_pipeline
}
