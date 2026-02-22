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

    pub fn matrix(&self, aspect: f32) -> Mat4 {
        let scale = 1.0 / self.zoom_factor;

        let proj = Mat4::orthographic_rh_gl(
            -aspect * scale,
            aspect * scale,
            -scale,
            scale,
            -1.0,
            1.0,
        );

        let view = Mat4::from_translation((-self.center).extend(0.0));

        proj * view
    }

    pub fn zoom(&mut self, delta: MouseScrollDelta) {
        let scroll = match delta {
            MouseScrollDelta::LineDelta(_, y) => y,
            MouseScrollDelta::PixelDelta(p) => p.y as f32,
        };

        self.zoom_factor *= (1.0 + scroll * 0.1).max(0.1);
    }

    pub fn mouse_input(&mut self, state: ElementState, button: MouseButton) {
        if button == MouseButton::Left {
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
                self.center -= delta;
            }
        }

        self.last_cursor = Some(current);
    }
}