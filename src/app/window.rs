//! The winit event loop: window creation, input, and drawing each frame.

use super::*;

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
            self.update_check = Some(crate::update::spawn_check());
            #[cfg(target_os = "macos")]
            {
                self.native_menu = native_menu::NativeMenu::install();
            }
        }
        let renderer =
            pollster::block_on(
                async move { AppRenderer::new(window_handle.clone(), width, height).await },
            );
        self.renderer = Some(renderer);

        self.gui_state = Some(gui_state);
        self.last_render_time = Some(Instant::now());
        self.initialized = true;

        if let Some(path) = self.pending_open.take() {
            self.open_store(&path);
        }
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

        // Acted on after the event, once the borrows above are released
        let mut ui_actions = None;
        let mut click = None;
        let mut boxed = None;
        let mut escape = false;
        let mut close_requested = false;

        // Handle specific window events such as keyboard input, resizing, and redraw requests
        match event {
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: winit::keyboard::PhysicalKey::Code(key_code),
                        state,
                        repeat,
                        ..
                    },
                ..
            } => match (key_code, state) {
                (winit::keyboard::KeyCode::Escape, ElementState::Pressed) => escape = true,
                // Space held: left-drag pans with any tool; a tap without a drag plays / pauses
                (winit::keyboard::KeyCode::Space, ElementState::Pressed) => {
                    if !repeat {
                        (self.space_held, self.space_panned) = (true, false);
                    }
                }
                (winit::keyboard::KeyCode::Space, ElementState::Released) => {
                    self.space_held = false;
                    if self.space_panned {
                        self.dragging = false;
                    } else if let Some(timeline) = self.timeline.as_mut() {
                        timeline.toggle_play();
                    }
                }
                (winit::keyboard::KeyCode::Home, ElementState::Pressed) => renderer.fit(),
                // Video registration: left / right switch between the selected position's runs
                (key @ (winit::keyboard::KeyCode::ArrowLeft | winit::keyboard::KeyCode::ArrowRight), ElementState::Pressed)
                    if self.workflow == registration::Workflow::Registration
                        && self.reg.as_ref().is_some_and(|r| r.position.is_some()) =>
                {
                    if let Some(reg) = self.reg.as_mut() {
                        let n = reg.position.map_or(1, |p| reg.videos.positions[p].runs.len());
                        reg.run = if key == winit::keyboard::KeyCode::ArrowLeft { reg.run.saturating_sub(1) } else { (reg.run + 1).min(n - 1) };
                    }
                }
                (key, ElementState::Pressed) => {
                    if let Some(timeline) = self.timeline.as_mut() {
                        match key {
                            winit::keyboard::KeyCode::ArrowLeft => timeline.step(-1),
                            winit::keyboard::KeyCode::ArrowRight => timeline.step(1),
                            _ => (),
                        }
                    }
                }
                _ => (),
            },
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                let registering = self.workflow == registration::Workflow::Registration;
                let boxing = !registering && self.annot.as_ref().is_some_and(|a| a.tool == Tool::BoxSelect);
                let nudging = registering && self.reg.as_ref().is_some_and(|r| r.tool == registration::RegTool::Nudge);
                let cursor = [self.cursor.x as f32, self.cursor.y as f32];
                let pressed = state == ElementState::Pressed;
                if self.space_held {
                    // the hand: pan, whatever the tool
                    self.dragging = pressed;
                    self.space_panned |= pressed;
                } else if let (true, Some(reg), Some(store)) = (nudging, self.reg.as_mut(), self.store.as_ref()) {
                    // Nudge: a press inside a video frame selects that run and drags it; a press
                    // outside every frame does nothing.
                    if pressed {
                        let at = renderer.screen_to_world(cursor);
                        if let Some((p, run)) = reg.frame_at(at, store, &added_segments(store, self.annot.as_ref())) {
                            if (reg.position, reg.run) != (Some(p), run) {
                                reg.video_edge = None;
                            }
                            reg.position = Some(p);
                            reg.run = run;
                            let base = reg.layer.nudges.get(&reg.run_path(p, run)).copied().unwrap_or([0.0; 2]);
                            self.nudge_drag = Some((at, base));
                            self.press = Some(self.cursor);
                        }
                    } else if self.nudge_drag.take().is_some() {
                        let moved = self.press.take().is_some_and(|p| (p.x - self.cursor.x).hypot(p.y - self.cursor.y) >= 3.0);
                        if let (true, Some(n), Some(p)) = (moved, reg.drag_nudge.take(), reg.position) {
                            reg.pending.push(Op::SetNudge(reg.run_path(p, reg.run), Some(n)));
                        }
                        reg.drag_nudge = None; // a click without a drag only selects
                    }
                } else if boxing {
                    if pressed {
                        self.box_start = Some(renderer.screen_to_world(cursor));
                    } else if let Some(start) = self.box_start.take() {
                        boxed = Some((start, renderer.screen_to_world(cursor)));
                    }
                } else {
                    self.dragging = pressed;
                    if pressed {
                        self.press = Some(self.cursor);
                    } else if let Some(p) = self.press.take() {
                        if (p.x - self.cursor.x).hypot(p.y - self.cursor.y) < 4.0 {
                            click = Some(cursor);
                        }
                    }
                }
            }
            // Right-drag pans with any tool (left-drag draws boxes with Box select).
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Right,
                ..
            } => self.dragging = state == ElementState::Pressed,
            WindowEvent::CursorMoved { position, .. } => {
                if let (Some((start, base)), Some(reg)) = (self.nudge_drag, self.reg.as_mut()) {
                    let now = renderer.screen_to_world([position.x as f32, position.y as f32]);
                    reg.drag_nudge = Some([base[0] + now[0] - start[0], base[1] + now[1] - start[1]]);
                } else if self.dragging {
                    let (dx, dy) = (position.x - self.cursor.x, position.y - self.cursor.y);
                    renderer.pan(dx as f32, dy as f32);
                }
                self.cursor = position;
            }
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            // Adobe style: scroll pans (Shift: sideways), ⌥ or ⌘ (Ctrl) + scroll zooms at the
            // cursor, and so does a trackpad pinch.
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (x * LINE_PX, y * LINE_PX),
                    MouseScrollDelta::PixelDelta(p) => (p.x as f32, p.y as f32),
                };
                let m = self.modifiers;
                let command = if cfg!(target_os = "macos") { m.super_key() } else { m.control_key() };
                if m.alt_key() || command {
                    // ponytail: fixed zoom speed (a 40 px notch = 1.15x), a preference if it feels off
                    let d = if dy != 0.0 { dy } else { dx };
                    renderer.zoom((-d * 0.0035).exp(), [self.cursor.x as f32, self.cursor.y as f32]);
                } else if m.shift_key() && dx == 0.0 {
                    renderer.pan(dy, 0.0);
                } else {
                    renderer.pan(dx, dy);
                }
            }
            WindowEvent::PinchGesture { delta, .. } => {
                renderer.zoom((-delta as f32).exp(), [self.cursor.x as f32, self.cursor.y as f32]);
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
            WindowEvent::CloseRequested => close_requested = true,
            WindowEvent::RedrawRequested => {
                // Get time delta sice last draw
                let now = Instant::now();
                let delta_time = now - *last_render_time;
                *last_render_time = now;
                let frame_ms = delta_time.as_secs_f64() * 1000.0;
                self.dev.timings.frame_ms = 0.9 * self.dev.timings.frame_ms + 0.1 * frame_ms;

                //take gui input
                let gui_input = gui_state.take_egui_input(window);
                let mut actions = ui::UiActions::default();
                let ppp = gui_state.egui_ctx().pixels_per_point();
                let labels = self
                    .store
                    .as_ref()
                    .filter(|_| self.layers.node_ids || self.layers.hyperedge_ids)
                    .and_then(|store| id_labels(store, &self.layers, renderer, ppp));
                let selected_added = match self.selection {
                    Some(ui::Selection::Added(id)) => Some(id),
                    _ => None,
                };
                let to_pt = |p: [f32; 2]| {
                    let [x, y] = renderer.world_to_screen(p);
                    egui::pos2(x / ppp, y / ppp)
                };
                let registering = self.workflow == registration::Workflow::Registration;
                let added_segments =
                    self.store.as_ref().map_or_else(Vec::new, |s| added_segments(s, self.annot.as_ref()));
                let mut map_texts = vec![];
                let overlay = match (registering, self.store.as_ref(), self.annot.as_ref(), self.reg.as_mut()) {
                    (true, Some(store), _, Some(reg)) => {
                        let (shapes, texts) = registration::overlay(reg, store, &added_segments, to_pt);
                        map_texts = texts;
                        shapes
                    }
                    (false, Some(store), Some(st), _) => annotate::overlay(store, st, renderer.time, selected_added, to_pt),
                    _ => vec![],
                };
                let mut overlay = overlay;
                if let Some(store) = self.store.as_ref() {
                    overlay.extend(detached_overlay(store, renderer, to_pt));
                }
                if let (false, Some(store), Some(st)) = (registering, self.store.as_ref(), self.annot.as_ref()) {
                    let cursor = [self.cursor.x as f32, self.cursor.y as f32];
                    let (trail_shapes, texts) = trail_overlay(store, st, &self.trail_routes, renderer, ppp);
                    overlay.extend(trail_shapes);
                    map_texts = texts;
                    overlay.extend(tool_overlay(store, st, renderer, ppp, cursor, self.box_start));
                }
                let unreachable_trails: Vec<String> = self
                    .annot
                    .as_ref()
                    .map(|st| {
                        st.own
                            .trails
                            .iter()
                            .filter(|(name, w)| {
                                w.len() >= 2
                                    && self.trail_routes.get(&(st.author.clone(), (*name).clone())).is_some_and(|r| r.2.is_none())
                            })
                            .map(|(name, _)| name.clone())
                            .collect()
                    })
                    .unwrap_or_default();
                if let Some(timeline) = self.timeline.as_mut() {
                    timeline.advance(delta_time.as_secs_f32());
                }

                //TODO: move this title to a constant or config file
                let title = "GrAMF - Graphs for Arbuscular Mycorrhizal Fungi";

                // Build the egui UI
                let egui_winit::egui::FullOutput {
                    //textures_delta is used to track changes to textures that egui uses (e.g., for images in the UI). It contains information about which textures were added, changed, or removed during this frame.
                    textures_delta,
                    shapes,
                    pixels_per_point,
                    platform_output,
                    ..
                } = gui_state.egui_ctx().run_ui(gui_input, |ui| {
                    // Build the UI using the `build_ui` function defined in the `ui` module
                    actions = ui::build_ui(
                        ui,
                        ui::FrameState {
                            title,
                            status: &self.status,
                            timeline: self.timeline.as_mut(),
                            layers: &mut self.layers,
                            store: self.store.as_ref(),
                            selection: self.selection,
                            labels: labels.as_deref(),
                            annot: self.annot.as_mut(),
                            overlay: &overlay,
                            confirm: self.confirm.is_some(),
                            export: &mut self.export,
                            dev: &mut self.dev,
                            update: self.update.as_deref(),
                            raw: self.raw.as_ref(),
                            boxed: &self.boxed,
                            reveal: self.reveal.is_some(),
                            settings_window: &mut self.settings_window,
                            unreachable_trails: &unreachable_trails,
                            map_texts: &map_texts,
                            workflow: &mut self.workflow,
                            added_segments: &added_segments,
                            reg: self.reg.as_mut(),
                        },
                    );
                });

                // Handle any platform output from egui (e.g., opening URLs, copying to clipboard, etc.)
                gui_state.handle_platform_output(window, platform_output);

                // Tessellate the egui shapes into paint jobs that can be rendered by the renderer
                let paint_jobs = gui_state.egui_ctx().tessellate(shapes, pixels_per_point);

                // Get descriptor for current screen
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

                // In registration the STG time follows the selected run.
                if let (registration::Workflow::Registration, Some(reg), Some(timeline)) =
                    (self.workflow, self.reg.as_ref(), self.timeline.as_mut())
                {
                    if let Some(f) = reg.selected_run_utc().and_then(|t| crate::video::runs::stg_frame(t, &timeline.axis.timepoints)) {
                        timeline.index = f;
                    }
                }
                if let Some(timeline) = &self.timeline {
                    renderer.time = timeline.index as f32;
                }
                if let Some(store) = self.store.as_ref() {
                    let key = (self.timeline.as_ref().map(|t| t.index), self.layers.colour_by, self.layers.scale);
                    if self.style_key != Some(key) {
                        renderer.set_edge_styles(&crate::colour::edge_styles(store, key.0, key.2));
                        self.style_key = Some(key);
                    }
                    renderer.line = crate::renderer::LineStyle {
                        base_px: self.layers.line_px,
                        by_width: self.layers.line_by_width,
                        edge_colour: self.layers.colour_by == crate::colour::ColourBy::Width,
                        reference_width_um: self.reference_width,
                    };
                }

                // Map area must be known before rendering (fit uses its aspect)
                if let Some(r) = actions.map_rect.map(|r| r * pixels_per_point) {
                    renderer.set_map_rect([r.min.x, r.min.y, r.width(), r.height()]);
                }

                // Render frame
                /*
                Screen descriptor has width, height and dpi for all renderers to use
                Paint jobs and textures delta are used to render the egui UI
                delta_time is used to update the scene based on time (e.g., for animations)

                This function will render the graph scene, then the EGUI ui                
                 */
                renderer.render_frame(screen_descriptor, paint_jobs, textures_delta, delta_time);
                ui_actions = Some(actions);
            }
            _ => (),
        }

        // Request a redraw of the window after processing the event to ensure that the UI is updated
        window.request_redraw();

        if escape {
            self.box_start = None;
            let cancelled_step = self.annot.as_mut().and_then(|a| a.first.take()).is_some();
            if !cancelled_step {
                self.select(None);
            }
        }
        if let Some(px) = click {
            self.pick(px);
        }
        if let Some((a, b)) = boxed {
            self.box_select(a, b);
        }
        if let Some(actions) = ui_actions {
            self.apply(actions);
        }
        if close_requested {
            self.request_quit();
        }
        #[cfg(target_os = "macos")]
        self.handle_native_menu();
        // Registration edits go through the annotation file (undo, dirty, save) and come
        // back into the workspace as its current state.
        if let (Some(reg), Some(st)) = (self.reg.as_mut(), self.annot.as_mut()) {
            for op in reg.pending.drain(..) {
                st.own.edit(op);
            }
            reg.sync(&st.own.registration);
        }
        if let Some(path) = self.pending_open.take() {
            self.open_store(&path);
        }
        self.refresh_dev();
        self.refresh_trail_routes();
        if let Some(tag) = self.update_check.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.update = Some(tag);
        }

        let title = match (&self.store_path, self.dirty()) {
            (Some(p), dirty) => format!("{}GrAMF — {}", if dirty { "• " } else { "" }, p.display()),
            (None, _) => "GrAMF".into(),
        };
        if title != self.window_title {
            if let Some(window) = self.window.as_ref() {
                window.set_title(&title);
            }
            self.window_title = title;
        }
        if self.quit {
            event_loop.exit();
        }
    }
}
