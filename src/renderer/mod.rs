mod graph_renderer;
mod camera;
mod gpu;
use crate::renderer::gpu::Gpu;
use egui_wgpu::{Renderer as EguiRenderer, ScreenDescriptor, wgpu};
use web_time::Duration;
use graph_renderer::GraphRenderer;

/// The `AppRenderer` struct is responsible for managing the GPU, depth texture, egui renderer, and 3D scene. It handles rendering frames, resizing, and updating the scene based on time and aspect ratio.
pub struct AppRenderer {
    gpu: Gpu,
    depth_texture_view: wgpu::TextureView,
    egui_renderer: EguiRenderer,
    graph_renderer: GraphRenderer,
    /// Bounding box of the loaded graph in µm, for fit-to-view.
    bounds: Option<([f32; 2], [f32; 2])>,
    /// Map area in physical pixels `[x, y, width, height]`; the rest is covered by panels.
    map_rect: [f32; 4],
    /// Fit once the map area is known (a store can load before the first UI frame).
    fit_pending: bool,
}

impl AppRenderer {
    const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    /// Asynchronously creates a new `AppRenderer` instance by initializing the GPU, depth texture, egui renderer, and 3D scene based on the provided window and dimensions.
    pub async fn new(
        window: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
    ) -> Self {
        let gpu = Gpu::new_async(window, width, height).await;
        let depth_texture_view = gpu.create_depth_texture(width, height);

        let egui_renderer = EguiRenderer::new(
            &gpu.device,
            gpu.surface_config.format,
            egui_wgpu::RendererOptions {
                depth_stencil_format: Some(Self::DEPTH_FORMAT),
                msaa_samples: 1,
                ..Default::default()
            },
        );

        let graph_renderer = GraphRenderer::new(&gpu.device, gpu.surface_format);

        Self {
            gpu,
            depth_texture_view,
            egui_renderer,
            graph_renderer,
            bounds: None,
            map_rect: [0.0, 0.0, width as f32, height as f32],
            fit_pending: false,
        }
    }

    pub fn load_arcs(&mut self, arcs: &crate::store::Arcs) {
        self.graph_renderer.set_arcs(&self.gpu.device, arcs);
        self.bounds = Some((arcs.min, arcs.max));
        self.fit_pending = true;
    }

    /// Clamped to the surface: egui can report a rect slightly past it while resizing.
    pub fn set_map_rect(&mut self, rect: [f32; 4]) {
        let (sw, sh) = (
            self.gpu.surface_config.width as f32,
            self.gpu.surface_config.height as f32,
        );
        let x = rect[0].clamp(0.0, sw - 1.0);
        let y = rect[1].clamp(0.0, sh - 1.0);
        self.map_rect = [x, y, rect[2].clamp(1.0, sw - x), rect[3].clamp(1.0, sh - y)];
    }

    fn map_aspect(&self) -> f32 {
        self.map_rect[2] / self.map_rect[3]
    }

    pub fn fit(&mut self) {
        if let Some((min, max)) = self.bounds {
            let aspect = self.map_aspect();
            self.graph_renderer.camera.fit(min, max, aspect);
        }
    }

    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.graph_renderer.camera.pan(dx, dy, self.map_rect[3]);
    }

    /// `cursor` is in window pixels.
    pub fn zoom(&mut self, factor: f32, cursor: [f32; 2]) {
        let [x, y, w, h] = self.map_rect;
        let local = [cursor[0] - x, cursor[1] - y];
        self.graph_renderer.camera.zoom(factor, local, [w, h]);
    }

    /// Resizes the renderer by updating the GPU's surface configuration and recreating the depth texture with the new dimensions.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.gpu.resize(width, height);
        self.depth_texture_view = self.gpu.create_depth_texture(width, height);
    }

    /// Renders a frame by updating the scene based on the delta time, processing egui textures and paint jobs, acquiring the current surface texture, and executing the render pass to draw both the 3D scene and the egui UI.
    pub fn render_frame(
        &mut self,
        screen_descriptor: ScreenDescriptor,
        paint_jobs: Vec<egui::epaint::ClippedPrimitive>,
        textures_delta: egui::TexturesDelta,
        delta_time: Duration,
    ) {
        let _ = delta_time; // for time playback (M2)

        if std::mem::take(&mut self.fit_pending) {
            self.fit();
        }
        self.graph_renderer
            .update(&self.gpu.queue, self.map_aspect());

        for (id, image_delta) in &textures_delta.set {
            self.egui_renderer
                .update_texture(&self.gpu.device, &self.gpu.queue, *id, image_delta);
        }

        for id in &textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // Update the egui renderer's buffers with the current paint jobs and screen descriptor before rendering
        self.egui_renderer.update_buffers(
            &self.gpu.device,
            &self.gpu.queue,
            &mut encoder,
            &paint_jobs,
            &screen_descriptor,
        );

        // Acquire the current surface texture for rendering, handling potential errors such as outdated or occluded surfaces by reconfiguring or returning early as needed
        let surface_texture = match self.gpu.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.gpu
                    .surface
                    .configure(&self.gpu.device, &self.gpu.surface_config);
                match self.gpu.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(frame)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
                    wgpu::CurrentSurfaceTexture::Occluded => return,
                    other => {
                        panic!("Failed to get surface texture after reconfiguration: {other:?}")
                    }
                }
            }
            wgpu::CurrentSurfaceTexture::Occluded => return,
            other => panic!("Failed to get surface texture: {other:?}"),
        };

        // Create a texture view from the acquired surface texture to be used as the render target for the render pass
        let surface_texture_view =
            surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor {
                    label: wgpu::Label::default(),
                    aspect: wgpu::TextureAspect::default(),
                    format: Some(self.gpu.surface_format),
                    dimension: None,
                    base_mip_level: 0,
                    mip_level_count: None,
                    base_array_layer: 0,
                    array_layer_count: None,
                    usage: None,
                });

        encoder.insert_debug_marker("Render scene");

        // Begin a render pass with the surface texture view as the color attachment and the depth texture view as the depth attachment, clearing both at the start of the pass. Then render the 3D scene followed by the egui UI using the respective renderers.
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                // Set up the color attachment to render to the surface texture, clearing it with a specified color at the start of the pass
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &surface_texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.05,
                            g: 0.06,
                            b: 0.07,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                // Set up the depth attachment to use the depth texture view, clearing it to a default depth value at the start of the pass
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            // Render the graph into the map area only
            let [x, y, w, h] = self.map_rect;
            render_pass.set_viewport(x, y, w, h, 0.0, 1.0);
            self.graph_renderer.render(&mut render_pass);

            // Render the egui UI on top of the 3D scene
            self.egui_renderer.render(
                &mut render_pass.forget_lifetime(),
                &paint_jobs,
                &screen_descriptor,
            );
        }

        self.gpu.queue.submit(std::iter::once(encoder.finish()));
        surface_texture.present();
    }
}
