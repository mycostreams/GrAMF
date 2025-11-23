/// Handling updates of controls
/// args:
/// ---
/// camera_query: Combination of camera transform and projection
/// input: A list of keyboard and mouse inputs
/// time: An object containing time information.
use bevy::{
    input::mouse::{AccumulatedMouseScroll, MouseMotion, MouseWheel},
    math::ops::powf,
    prelude::*,
    transform,
};

pub(crate) fn controls(
    camera_query: Single<(&mut Transform, &mut Projection)>,
    input: Res<ButtonInput<KeyCode>>,
    time: Res<Time<Fixed>>,
) {
    // Get variables to adjust 
    let (mut transform, mut projection) = camera_query.into_inner();
    let Projection::Orthographic(projection2d) = &mut *projection else {
        return;
    };

    // Translation speed is adjusted by scale
    let fspeed = 600.0 * time.delta_secs() * projection2d.scale;

    // Camera movement controls
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

    // Camera zoom controls
    if input.pressed(KeyCode::Comma) {
        projection2d.scale *= powf(4.0f32, time.delta_secs());
    }

    if input.pressed(KeyCode::Period) {
        projection2d.scale *= powf(0.25f32, time.delta_secs());
    }

    
    
}
