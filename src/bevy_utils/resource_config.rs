pub static EDGE_WIDTH_SCALE_VISIBLE: f32 = 2.0;

#[derive(Debug)]
pub(super) struct CameraSettings {
    pub translation_cont_sensitivity: f32,
    pub zoom_const_sensitivity: f32,
    pub zoom_scroll_line_sensitivity: f32,
    pub zoom_scroll_pixel_sensitivity: f32,
}

pub(super) static CAMERA_SETTINGS: CameraSettings = CameraSettings {
    translation_cont_sensitivity: 600.0,
    zoom_const_sensitivity: 4.0,
    zoom_scroll_pixel_sensitivity: 1.0 + 1e-3,
    zoom_scroll_line_sensitivity: 1.0 + 1e-1,
};