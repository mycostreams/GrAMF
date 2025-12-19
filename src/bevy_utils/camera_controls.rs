/// Handling updates of controls
/// args:
/// ---
/// camera_query: Combination of camera transform and projection
/// input: A list of keyboard and mouse inputs
/// time: An object containing time information.
use bevy::{
    input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel},
    math::ops::powf,
    prelude::*,
    window::PrimaryWindow,
};

use crate::bevy_utils::resource_config::EDGE_WIDTH_SCALE_VISIBLE;

#[derive(Message)]
pub struct ResetCameraEvent;

pub(crate) fn camera_controls_plugin(app: &mut App) {
    app.add_systems(Update, controls)
        .add_systems(Update, reset_camera)
        .add_message::<ResetCameraEvent>()
        .add_systems(Update, update_edge_scale);
}

#[derive(Debug)]
struct CameraSettings {
    translation_cont_sensitivity: f32,
    zoom_const_sensitivity: f32,
    zoom_scroll_line_sensitivity: f32,
    zoom_scroll_pixel_sensitivity: f32,
}

static CAMERA_SETTINGS: CameraSettings = CameraSettings {
    translation_cont_sensitivity: 600.0,
    zoom_const_sensitivity: 4.0,
    zoom_scroll_pixel_sensitivity: 1.0 + 1e-3,
    zoom_scroll_line_sensitivity: 1.0 + 1e-1,
};

/// Update function that handles all controls in the graph viewport
pub(crate) fn controls(
    camera_query: Single<(&mut Transform, &mut Projection)>,
    input: Res<ButtonInput<KeyCode>>,
    time: Res<Time<Fixed>>,
    mut mouse_scroll: MessageReader<MouseWheel>,
    mut mouse_motion: MessageReader<MouseMotion>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<PrimaryWindow>>,
) {
    // Get variables to adjust
    let window_center = window.size() / 2.0;
    let (mut transform, mut projection) = camera_query.into_inner();
    let Projection::Orthographic(projection2d) = &mut *projection else {
        return;
    };
    let mouse_pos = match window.cursor_position() {
        Some(x) => x,
        None => return,
    };

    // --- CONTINUOUS CONTROLS ---
    // Translation speed is adjusted by scale
    let fspeed =
        CAMERA_SETTINGS.translation_cont_sensitivity * time.delta_secs() * projection2d.scale;

    // Arrow keys functions
    if input.pressed(KeyCode::ArrowUp) {
        transform.translation.y += fspeed;
    }
    if input.pressed(KeyCode::ArrowDown) {
        transform.translation.y -= fspeed;
    }
    if input.pressed(KeyCode::ArrowLeft) {
        transform.translation.x -= fspeed;
    }
    if input.pressed(KeyCode::ArrowRight) {
        transform.translation.x += fspeed;
    }

    // Period-Comma controls zoom
    if input.pressed(KeyCode::Comma) {
        projection2d.scale *= powf(CAMERA_SETTINGS.zoom_const_sensitivity, time.delta_secs());
    }
    if input.pressed(KeyCode::Period) {
        projection2d.scale *= powf(
            1.0 / CAMERA_SETTINGS.zoom_const_sensitivity,
            time.delta_secs(),
        );
    }

    // --- SINGLE EVENT CONTROLS ---
    // Mouse scrolling
    for ev in mouse_scroll.read() {
        let zoom: f32 = match ev.unit {
            MouseScrollUnit::Line => powf(CAMERA_SETTINGS.zoom_scroll_line_sensitivity, ev.y),
            MouseScrollUnit::Pixel => powf(CAMERA_SETTINGS.zoom_scroll_pixel_sensitivity, -ev.y),
        };
        let origin_transform = get_origin_shift(window_center, mouse_pos, zoom);
        projection2d.scale *= zoom;
        transform.translation.x += origin_transform.x * projection2d.scale;
        transform.translation.y -= origin_transform.y * projection2d.scale;
    }

    if mouse_buttons.pressed(MouseButton::Right) {
        for ev in mouse_motion.read() {
            transform.translation.x -= ev.delta.x * projection2d.scale;
            transform.translation.y += ev.delta.y * projection2d.scale;
        }
    }
}

fn get_origin_shift(origin_2d: Vec2, mouse_loc: Vec2, zoom: f32) -> Vec2 {
    // Get mouse position from the center
    let mouse_pos_from_cent = mouse_loc - origin_2d;
    mouse_pos_from_cent - mouse_pos_from_cent * zoom
}

/// Reset camera to default position and zoom level.
pub(crate) fn reset_camera(
    camera_query: Single<(&mut Transform, &mut Projection)>,
    messages: MessageReader<ResetCameraEvent>,
) {
    if messages.is_empty() {
        return;
    }
    let (mut transform, mut projection) = camera_query.into_inner();
    let Projection::Orthographic(projection2d) = &mut *projection else {
        return;
    };
    transform.translation = bevy::math::Vec3::ZERO;
    projection2d.scale = 1.0;
}

/// Update the scale of edges based on the camera's zoom level to maintain consistent visual thickness.
pub(crate) fn update_edge_scale(
    camera_query: Single<&Projection>,
    mut edge_query: Query<&mut Transform, With<crate::bevy_utils::graph_entities::EdgeTag>>,
) {
    let Projection::Orthographic(proj) = &*camera_query else {
        return;
    };

    for mut transform in &mut edge_query {
        transform.scale.y = EDGE_WIDTH_SCALE_VISIBLE * proj.scale;
    }
}

#[test]
fn test_get_origin_shift() {
    let origin = Vec2::new(400.0, 300.0);
    let mouse_pos = Vec2::new(450.0, 350.0);
    let zoom = 1.1; // Zooming in by 10%
    let shift = get_origin_shift(origin, mouse_pos, zoom);

    println!("Shift: {:?}", shift);
    assert!((shift.x + 5.0).abs() < 1e-5);
    assert!((shift.y + 5.0).abs() < 1e-5);
}
