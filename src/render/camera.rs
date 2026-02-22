use glam::{Mat4, Vec2};
use winit::event::{ElementState, MouseButton, MouseScrollDelta};

pub struct Camera {
    pub center: Vec2,
    pub zoom_factor: f32,
    dragging: bool,
    last_cursor: Option<Vec2>,
}

impl Camera {
    pub fn new() -> Self {
        Self {
            center: Vec2::ZERO,
            zoom_factor: 1.0,
            dragging: false,
            last_cursor: None,
        }
    }

    pub fn matrix_equal_aspect(&self, width: f32, height: f32) -> Mat4 {
        let aspect = width / height;
        self.matrix(aspect)
    }

    fn matrix(&self, aspect: f32) -> Mat4 {
        let scale = 1.0 / self.zoom_factor;

        let proj =
            Mat4::orthographic_rh_gl(-aspect * scale, aspect * scale, -scale, scale, -1.0, 1.0);

        let view = Mat4::from_translation((-self.center).extend(0.0));

        proj * view
    }

    pub fn zoom(&mut self, delta: MouseScrollDelta, window_size: (u32, u32)) {
        let scroll = match delta {
            MouseScrollDelta::LineDelta(_, y) => -y,
            MouseScrollDelta::PixelDelta(p) => p.y as f32 / 15.0,
        };

        let window_center = Vec2::new(window_size.0 as f32 / 2.0, window_size.1 as f32 / 2.0);
        let mouse_pos = self.last_cursor.unwrap_or(window_center);
        let delta_zoom = (1.0 + scroll * 0.1).max(0.1);

        // Convert mouse position to world coordinates
        let aspect = window_size.0 as f32 / window_size.1 as f32;
        let scale = 1.0 / self.zoom_factor;
        let proj = glam::Mat4::orthographic_rh_gl(
            -aspect * scale,
            aspect * scale,
            -scale,
            scale,
            -1.0,
            1.0,
        );
        let view = glam::Mat4::from_translation((-self.center).extend(0.0));
        let inv = (proj * view).inverse();
        let ndc = Vec2::new(
            (mouse_pos.x / window_size.0 as f32) * 2.0 - 1.0,
            1.0 - (mouse_pos.y / window_size.1 as f32) * 2.0,
        );
        let world_before = inv.transform_point3(ndc.extend(0.0)).truncate();

        self.zoom_factor *= delta_zoom;

        // Recompute world position after zoom
        let scale_after = 1.0 / self.zoom_factor;
        let proj_after = glam::Mat4::orthographic_rh_gl(
            -aspect * scale_after,
            aspect * scale_after,
            -scale_after,
            scale_after,
            -1.0,
            1.0,
        );
        let view_after = glam::Mat4::from_translation((-self.center).extend(0.0));
        let inv_after = (proj_after * view_after).inverse();
        let world_after = inv_after.transform_point3(ndc.extend(0.0)).truncate();

        // Adjust camera center so the world point under the cursor stays fixed
        self.center += world_before - world_after;
    }

    pub fn mouse_input(&mut self, state: ElementState, button: MouseButton) {
        if button == MouseButton::Right {
            self.dragging = state == ElementState::Pressed;
            if !self.dragging {
                self.last_cursor = None;
            }
        }
    }

    pub fn cursor_moved(&mut self, pos: winit::dpi::PhysicalPosition<f64>) {
        let current = Vec2::new(pos.x as f32, pos.y as f32);

        if self.dragging {
            if let Some(last) = self.last_cursor {
                let delta = (current - last) * 0.002 / self.zoom_factor;
                self.center.x -= delta.x;
                self.center.y += delta.y;
            }
        }

        self.last_cursor = Some(current);
    }
}
