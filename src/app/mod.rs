mod ui;

use crate::renderer::Renderer;
use egui_winit::State;
use std::sync::Arc;
use web_time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{KeyEvent, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Theme, Window, WindowId};

#[derive(Default)]
pub struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    gui_state: Option<State>,
    last_render_time: Option<Instant>,
    last_size: (u32, u32),
    initialized: bool,
}

impl ApplicationHandler for App {
    /// Called when the application is suspended (e.g., minimized or sent to the background).
    fn suspended(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        self.renderer = None;
        self.window = None;
    }

    /// Called when the application is resumed (e.g., restored or brought to the foreground).
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let mut attributes = Window::default_attributes();

        attributes = attributes.with_title("GrAMF");

        let Ok(window) = event_loop.create_window(attributes) else {
            return;
        };

        let window_handle = Arc::new(window);
        self.window = Some(window_handle.clone());

        let gui_context = egui::Context::default();

        let inner_size = window_handle.inner_size();
        self.last_size = (inner_size.width, inner_size.height);

        let viewport_id = gui_context.viewport_id();
        let gui_state = State::new(
            gui_context,
            viewport_id,
            &window_handle,
            Some(window_handle.scale_factor() as _),
            Some(Theme::Dark),
            None,
        );

        let (width, height) = (
            window_handle.inner_size().width,
            window_handle.inner_size().height,
        );

        if !self.initialized {
            env_logger::init();
        }
        let renderer =
            pollster::block_on(
                async move { Renderer::new(window_handle.clone(), width, height).await },
            );
        self.renderer = Some(renderer);

        self.gui_state = Some(gui_state);
        self.last_render_time = Some(Instant::now());
        self.initialized = true;
    }

    /// Called when a window event occurs (e.g., keyboard input, resizing, etc.).
    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        // Ensure that the GUI state, renderer, window, and last render time are all available before processing the event
        let (Some(gui_state), Some(renderer), Some(window), Some(last_render_time)) = (
            self.gui_state.as_mut(),
            self.renderer.as_mut(),
            self.window.as_ref(),
            self.last_render_time.as_mut(),
        ) else {
            return;
        };

        // Pass the event to the GUI state for handling, and if it was consumed, return early
        if gui_state.on_window_event(window, &event).consumed {
            return;
        }

        // Handle specific window events such as keyboard input, resizing, and redraw requests
        match event {
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: winit::keyboard::PhysicalKey::Code(key_code),
                        ..
                    },
                ..
            } => {
                if matches!(key_code, winit::keyboard::KeyCode::Escape) {
                    event_loop.exit();
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                let scale_factor = window.scale_factor() as f32;
                gui_state.egui_ctx().set_pixels_per_point(scale_factor);
            }
            WindowEvent::Resized(PhysicalSize { width, height }) => {
                if width == 0 || height == 0 {
                    return;
                }

                log::info!("Resizing renderer surface to: ({width}, {height})");
                renderer.resize(width, height);
                self.last_size = (width, height);

                let scale_factor = window.scale_factor() as f32;
                gui_state.egui_ctx().set_pixels_per_point(scale_factor);
            }
            WindowEvent::CloseRequested => {
                log::info!("Close requested. Exiting...");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let delta_time = now - *last_render_time;
                *last_render_time = now;

                let gui_input = gui_state.take_egui_input(window);

                let title = "GrAMF - Graphs for Arbuscular Mycorrhizal Fungi";

                let egui_winit::egui::FullOutput {
                    textures_delta,
                    shapes,
                    pixels_per_point,
                    platform_output,
                    ..
                } = gui_state.egui_ctx().run_ui(gui_input, |ui| {
                    ui::build_ui(ui, title);
                });

                gui_state.handle_platform_output(window, platform_output);

                let paint_jobs = gui_state.egui_ctx().tessellate(shapes, pixels_per_point);

                let screen_descriptor = {
                    let (width, height) = self.last_size;
                    if width == 0 || height == 0 {
                        return;
                    }
                    egui_wgpu::ScreenDescriptor {
                        size_in_pixels: [width, height],
                        pixels_per_point,
                    }
                };

                renderer.render_frame(screen_descriptor, paint_jobs, textures_delta, delta_time);
            }
            _ => (),
        }

        // Request a redraw of the window after processing the event to ensure that the UI is updated
        window.request_redraw();
    }
}
