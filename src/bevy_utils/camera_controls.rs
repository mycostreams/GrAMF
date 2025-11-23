/// Handling updates of controls
/// args:
/// ---
/// camera_query: Combination of camera transform and projection
/// input: A list of keyboard and mouse inputs
/// time: An object containing time information.
use bevy::{
    input::mouse::{AccumulatedMouseScroll, MouseMotion, MouseScrollUnit, MouseWheel},
    math::ops::powf,
    prelude::*,
    transform,
};

#[derive(Debug)]
struct CameraSettings {
    translation_cont_sensitivity: f32,
    zoom_const_sensitivity: f32,
    zoom_scroll_line_sensitivity: f32,
    zoom_scroll_pixel_sensitivity:f32
}

static CAMERA_SETTINGS: CameraSettings = CameraSettings {
    translation_cont_sensitivity: 600.0,
    zoom_const_sensitivity: 4.0,
    zoom_scroll_pixel_sensitivity: 1.0 + 1e-3,
    zoom_scroll_line_sensitivity: 1.0 + 1e-1,
};

pub(crate) fn controls(
    camera_query: Single<(&mut Transform, &mut Projection)>,
    input: Res<ButtonInput<KeyCode>>,
    time: Res<Time<Fixed>>,
    mut mouse: MessageReader<MouseWheel>,
) {
    // Get variables to adjust
    let (mut transform, mut projection) = camera_query.into_inner();
    let Projection::Orthographic(projection2d) = &mut *projection else {
        return;
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
    for ev in mouse.read() {
        match ev.unit {
            MouseScrollUnit::Line => {
                println!("Scrolling by {}", ev.y);
                projection2d.scale *= powf(CAMERA_SETTINGS.zoom_scroll_line_sensitivity, ev.y);
            }
            MouseScrollUnit::Pixel => {
                println!("Scrolling by {}", ev.y);
                projection2d.scale *= powf(CAMERA_SETTINGS.zoom_scroll_pixel_sensitivity, ev.y)
            }
        }
    }
}
