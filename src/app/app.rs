use egui_wgpu::wgpu::SurfaceError;
use egui_wgpu::{ScreenDescriptor, wgpu};
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

use crate::app::state::AppState;
// use crate::render::renderer;

pub struct App {
    instance: wgpu::Instance,
    state: Option<AppState>,
    window: Option<Arc<Window>>,
}

impl App {
    pub fn new() -> Self {
        let instance = egui_wgpu::wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        Self {
            instance,
            state: None,
            window: None,
        }
    }

    async fn set_window(&mut self, window: Window) {
        let window = Arc::new(window);
        let initial_width = 1360;
        let initial_height = 768;

        let _ = window.request_inner_size(PhysicalSize::new(initial_width, initial_height));
        window.set_title("GrAMF - Graph Analysis and Modeling Framework");

        let surface = self
            .instance
            .create_surface(window.clone())
            .expect("Failed to create surface!");

        let state = AppState::new(
            &self.instance,
            surface,
            &window,
            initial_width,
            initial_height,
        )
        .await;

        self.window.get_or_insert(window);
        self.state.get_or_insert(state);
    }

    fn handle_resized(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.state.as_mut().unwrap().resize_surface(width, height);
        }
    }

    fn handle_redraw(&mut self) {
        // Attempt to handle minimizing window
        if let Some(window) = self.window.as_ref()
            && let Some(min) = window.is_minimized()
            && min
        {
            println!("Window is minimized");
            return;
        }

        let state = self.state.as_mut().unwrap();

        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [state.surface_config.width, state.surface_config.height],
            pixels_per_point: self.window.as_ref().unwrap().scale_factor() as f32
                * state.scale_factor,
        };

        let surface_texture = state.surface.get_current_texture();

        match surface_texture {
            Err(SurfaceError::Outdated) => {
                // Ignoring outdated to allow resizing and minimization
                println!("wgpu surface outdated");
                return;
            }
            Err(_) => {
                surface_texture.expect("Failed to acquire next swap chain texture");
                return;
            }
            Ok(_) => {}
        };

        let surface_texture = surface_texture.unwrap();

        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = state
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        // Update camera buffer with correct aspect ratio
        let camera_matrix = state.camera.matrix_equal_aspect(
            state.surface_config.width as f32,
            state.surface_config.height as f32,
        );
        let camera_uniform = crate::render::buffers::CameraUniform {
            view_proj: camera_matrix.to_cols_array_2d(),
        };
        state.queue.write_buffer(
            &state.graph_renderer.camera_buffer,
            0,
            bytemuck::cast_slice(&[camera_uniform]),
        );

        state
            .graph_renderer
            .node_buffers
            .update_node_instances(&state.device, &state.queue, &state.graph.topology);
        state
            .graph_renderer
            .edge_buffers
            .update_edge_vertices(&state.device, &state.queue, &state.graph.topology);

        state.graph_renderer.render(
            // &state.queue,
            // &state.graph.topology,
            &mut encoder,
            &surface_view,
        );

        let window = self.window.as_ref().unwrap();

        {
            state.egui_renderer.begin_frame(window);

            egui::TopBottomPanel::top("Menu Panel").show(state.egui_renderer.context(), |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("Quit").clicked() {
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                            std::process::exit(0);
                        }
                    });
                    ui.menu_button("View", |ui| {
                        if ui.button("Reset camera").clicked() {
                            state.camera.reset();
                        }
                    })
                });
            });

            // Context menu for graph area
            let ctx = state.egui_renderer.context();
            egui::Area::new(egui::Id::new("graph_context_menu"))
                .interactable(true)
                .show(ctx, |ui| {
                    let rect = ui.ctx().content_rect();
                    let response = ui.allocate_rect(rect, egui::Sense::click());
                    response.context_menu(|ui| {
                        if ui.button("Add Node").clicked() {
                            if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                                let window_size = (state.surface_config.width, state.surface_config.height);
                                let world_pos = state.camera.screen_to_world(
                                    glam::Vec2::new(pos.x, pos.y),
                                    window_size
                                );
                                let visual_node = crate::graph::topology::VisualNode {
                                    position: world_pos,
                                    color: [0.2, 0.7, 1.0], // default color
                                    radius: 0.05,
                                };
                                state.graph.add_node(visual_node, vec![]);
                                ui.close();
                            }
                        }
                    });
                });

            state.egui_renderer.end_frame_and_draw(
                &state.device,
                &state.queue,
                &mut encoder,
                window,
                &surface_view,
                screen_descriptor,
            );
        }

        state.queue.submit(Some(encoder.finish()));
        surface_texture.present();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = event_loop
            .create_window(Window::default_attributes())
            .unwrap();
        pollster::block_on(self.set_window(window));
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        // let egui render to process the event first
        self.state
            .as_mut()
            .unwrap()
            .egui_renderer
            .handle_input(self.window.as_ref().unwrap(), &event);

        match event {
            WindowEvent::CloseRequested => {
                println!("The close button was pressed; stopping");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                self.handle_redraw();
                self.window.as_ref().unwrap().request_redraw();
            }
            WindowEvent::Resized(new_size) => {
                self.handle_resized(new_size.width, new_size.height);
            }
            WindowEvent::MouseInput {
                state: mouse_state,
                button,
                ..
            } => {
                if button == MouseButton::Left {
                    if let Some(state) = self.state.as_mut() {
                        state.camera.mouse_input(mouse_state, button);
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(state) = self.state.as_mut() {
                    state.camera.cursor_moved(position);
                    // Convert position to logical coordinates
                    let mouse_pos = glam::Vec2::new(position.x as f32, position.y as f32);
                    // Set highlight colors
                    let highlight_node = [1.0, 0.5, 0.0]; // orange
                    let highlight_edge = [1.0, 0.0, 0.0]; // red
                    let default_node = [0.2, 0.7, 1.0]; // blue
                    let default_edge = [1.0, 1.0, 1.0]; // white
                    self.handle_redraw();
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if let Some(state) = self.state.as_mut() {
                    state
                        .camera
                        .zoom(delta, self.window.as_ref().unwrap().inner_size().into());
                }
            }
            _ => (),
        }
    }
}
