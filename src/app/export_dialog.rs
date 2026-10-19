//! File > Export…: which tables, which edges, whose annotations (DESIGN.md §5).

use crate::export::Tables;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Scope {
    #[default]
    Plate,
    /// Edges in the current view that have appeared by the current timepoint.
    View,
    Selection,
}

#[derive(Default)]
pub struct ExportDialog {
    pub open: bool,
    pub tables: Tables,
    pub scope: Scope,
    /// Author whose annotations to apply; `None` exports pipeline data only.
    pub author: Option<String>,
    /// Keep excluded / duplicate edges with a flag (default), or drop them.
    pub drop_flagged: bool,
}

/// Shows the dialog; true when the user pressed Export.
pub fn show(ctx: &egui::Context, d: &mut ExportDialog, authors: &[String], has_selection: bool) -> bool {
    let mut export = false;
    let mut open = d.open;
    egui::Window::new("Export tables").open(&mut open).resizable(false).collapsible(false).show(ctx, |ui| {
        ui.strong("Tables");
        ui.checkbox(&mut d.tables.edges, "edges.csv");
        ui.checkbox(&mut d.tables.nodes, "nodes.csv");
        ui.checkbox(&mut d.tables.widths_t, "edge_widths_t.csv (width per edge and timepoint)");
        ui.checkbox(&mut d.tables.timepoints, "timepoints.csv");
        ui.checkbox(&mut d.tables.hyperedges, "hyperedges.csv");
        ui.checkbox(&mut d.tables.geometry, "geometry.geojson (polylines, for QGIS / shapely)");

        ui.separator();
        ui.strong("Edges");
        ui.radio_value(&mut d.scope, Scope::Plate, "whole plate");
        ui.radio_value(&mut d.scope, Scope::View, "in view, appeared by the current timepoint");
        ui.add_enabled_ui(has_selection, |ui| {
            ui.radio_value(&mut d.scope, Scope::Selection, "current selection");
        });
        if !has_selection && d.scope == Scope::Selection {
            d.scope = Scope::Plate;
        }

        ui.separator();
        ui.strong("Annotations");
        egui::ComboBox::from_id_salt("export_author")
            .selected_text(d.author.as_deref().unwrap_or("none (pipeline data only)"))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut d.author, None, "none (pipeline data only)");
                for a in authors {
                    ui.selectable_value(&mut d.author, Some(a.clone()), a);
                }
            });
        ui.add_enabled_ui(d.author.is_some(), |ui| {
            ui.radio_value(&mut d.drop_flagged, false, "keep excluded / duplicate edges, flagged");
            ui.radio_value(&mut d.drop_flagged, true, "drop them");
        });

        ui.separator();
        if ui.button("Export to folder…").clicked() {
            export = true;
        }
    });
    d.open = open && !export;
    export
}
