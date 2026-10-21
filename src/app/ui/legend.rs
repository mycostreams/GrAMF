//! The map legend and colour bar.

use super::*;

/// Colour scale with its end values, for the continuous colour-by modes. Clicking the bar
/// opens a menu of colormaps.
pub(super) fn legend(ui: &mut egui::Ui, store: &Store, layers: &mut Layers, timeline: Option<&Timeline>) {
    let (lo, hi) = match layers.colour_by {
        ColourBy::TFirst => {
            let last = timeline.map_or(0, |t| t.axis.len().saturating_sub(1));
            ("frame 0".to_string(), format!("frame {last}"))
        }
        ColourBy::Width => match crate::colour::width_range(store) {
            Some((lo, hi)) => (format!("≤ {lo:.1} µm"), format!("≥ {hi:.1} µm")),
            None => {
                ui.label(egui::RichText::new("no widths in this store").weak());
                return;
            }
        },
        _ => return,
    };
    let bar = ui
        .horizontal(|ui| {
            let size = egui::vec2((ui.available_width() - 28.0).clamp(60.0, 160.0), 10.0);
            let bar = colour_bar(ui, layers.scale, size, egui::Sense::click())
                .on_hover_text(format!("{} — click to change the colormap", layers.scale.cmap.name()))
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            // ⇄ is only in the bundled monospace font (Hack), so ask for monospace explicitly
            let icon = egui::RichText::new("⇄").monospace();
            if ui.selectable_label(layers.scale.reversed, icon).on_hover_text("reverse the colormap").clicked() {
                layers.scale.reversed = !layers.scale.reversed;
            }
            bar
        })
        .inner;
    egui::Popup::menu(&bar).show(|ui| {
        for cmap in Cmap::ALL {
            let row = ui.horizontal(|ui| {
                let preview = Scale { cmap, reversed: layers.scale.reversed };
                colour_bar(ui, preview, egui::vec2(90.0, 10.0), egui::Sense::hover());
                ui.selectable_label(layers.scale.cmap == cmap, cmap.name())
            });
            if row.inner.clicked() || row.response.interact(egui::Sense::click()).clicked() {
                layers.scale.cmap = cmap;
                ui.close();
            }
        }
    });
    ui.horizontal(|ui| {
        ui.small(lo);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| ui.small(hi));
    });
}

/// Paints the scale left to right into a new `size` rect.
fn colour_bar(ui: &mut egui::Ui, cmap: Scale, size: egui::Vec2, sense: egui::Sense) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, sense);
    const STEPS: usize = 32;
    for i in 0..STEPS {
        let [r, g, b, _] = cmap.sample(i as f32 / (STEPS - 1) as f32);
        let x0 = rect.left() + rect.width() * i as f32 / STEPS as f32;
        let x1 = rect.left() + rect.width() * (i + 1) as f32 / STEPS as f32;
        ui.painter().rect_filled(
            egui::Rect::from_x_y_ranges(x0..=x1, rect.y_range()),
            0.0,
            egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8),
        );
    }
    response
}
