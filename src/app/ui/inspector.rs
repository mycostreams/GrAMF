//! The Inspector panel: the selected edge, node or hyperedge, its annotations and raw rows.

use super::*;

/// Property rows for the selection, then the annotation fields. Returns a new selection if
/// the user asked for one, and any annotation edits.
pub(super) fn inspector(
    ui: &mut egui::Ui,
    store: &Store,
    sel: Selection,
    timeline: Option<&Timeline>,
    annot: Option<&mut AnnotationState>,
    raw: Option<&(usize, Vec<crate::devtools::RawTable>)>,
    raw_wanted: &mut bool,
    boxed: &[usize],
) -> (Option<Selection>, Vec<Op>) {
    let mut select = None;
    let mut ops = vec![];
    let row = |ui: &mut egui::Ui, key: &str, value: String| {
        ui.label(egui::RichText::new(key).weak());
        ui.label(value);
        ui.end_row();
    };
    // Two rows: the frame index, and the time in the slider's current unit.
    let appears = |ui: &mut egui::Ui, t: f32| {
        if !t.is_finite() {
            return row(ui, "appears", "—".into());
        }
        row(ui, "appears", format!("frame {}", t as usize));
        if let Some(tl) = timeline.filter(|tl| (t as usize) < tl.axis.len()) {
            if tl.unit != TimeUnit::Frame {
                row(ui, "", tl.axis.label(t as usize, tl.unit));
            }
        }
    };
    match sel {
        Selection::Edge(e) => {
            ui.strong(format!("Edge {e}"));
            egui::Grid::new("edge").num_columns(2).show(ui, |ui| {
                if let Some([u, v]) = store.edge_nodes.get(e) {
                    row(ui, "nodes", format!("{u} – {v}"));
                }
                row(ui, "length", format!("{:.1} µm", store.edge_length_um[e]));
                appears(ui, store.edge_t[e]);
                if let (false, Some(tl)) = (store.width_t.is_empty(), timeline) {
                    row(ui, "width now", match store.width_at(e, Some(tl.index)) {
                        Some(w) => format!("{w:.2} µm (frame {})", tl.index),
                        None => "not measured yet".into(),
                    });
                }
                row(ui, "width (pooled)", match store.width[e] {
                    Some((w, se)) => format!("{w:.2} ± {se:.2} µm"),
                    None => "not measured".into(),
                });
                let h = store.hyperedge[e];
                row(ui, "hyperedge", if h >= 0 { h.to_string() } else { "none".into() });
            });
            let h = store.hyperedge[e];
            if h >= 0 && ui.button(format!("Select hyperedge {h}")).clicked() {
                select = Some(Selection::Hyperedge(h));
            }
            if let Some(st) = annot {
                ops = annotate::inspector_section(ui, st, store, Target::Edge(e));
            }
            ui.separator();
            let section = egui::CollapsingHeader::new("Raw rows").id_salt("raw_rows").show(ui, |ui| match raw {
                Some((re, tables)) if *re == e => raw_tables(ui, tables),
                _ => {
                    ui.label(egui::RichText::new("loading…").weak());
                }
            });
            *raw_wanted = section.body_returned.is_some();
        }
        Selection::Node(n) => {
            let edges = store.incident_edges(n);
            ui.strong(format!("Node {}", store.node_ids[n]));
            egui::Grid::new("node").num_columns(2).show(ui, |ui| {
                let [x, y] = store.node_pos[n];
                row(ui, "position", format!("{x:.1}, {y:.1} µm"));
                row(ui, "degree", edges.len().to_string());
                appears(ui, store.node_t[n]);
            });
            ui.label(egui::RichText::new("edges").weak());
            ui.horizontal_wrapped(|ui| {
                for e in edges {
                    if ui.link(e.to_string()).clicked() {
                        select = Some(Selection::Edge(e));
                    }
                }
            });
            if let Some(st) = annot {
                ops = annotate::inspector_section(ui, st, store, Target::Node(store.node_ids[n]));
            }
        }
        Selection::Box => {
            let length: f32 = boxed.iter().map(|&e| store.edge_length_um[e]).sum();
            ui.strong("Box selection");
            egui::Grid::new("box").num_columns(2).show(ui, |ui| {
                row(ui, "edges", boxed.len().to_string());
                row(ui, "length", format!("{length:.1} µm"));
            });
            ui.label(egui::RichText::new("File > Export tables… can export just these edges").weak().small());
        }
        Selection::Added(id) => {
            ui.strong(format!("Added edge {id}"));
            let Some(st) = annot else {
                return (select, ops);
            };
            let mine = st.own.added.iter().find(|a| a.id == id).copied();
            let found = mine.map(|a| (st.author.clone(), a)).or_else(|| {
                st.others.iter().find_map(|(o, _)| o.added.iter().find(|a| a.id == id).map(|a| (o.author.clone(), *a)))
            });
            if let Some((author, a)) = found {
                egui::Grid::new("added").num_columns(2).show(ui, |ui| {
                    row(ui, "nodes", format!("{} – {}", a.u, a.v));
                    row(ui, "author", author);
                });
            }
            if let Some(a) = mine {
                if ui.button("Remove").clicked() {
                    ops.push(Op::RemoveAddedEdge(a));
                }
            }
        }
        Selection::Hyperedge(h) => {
            let edges = store.hyperedge_edges(h);
            let length: f32 = edges.iter().map(|&e| store.edge_length_um[e]).sum();
            let first = edges.iter().map(|&e| store.edge_t[e]).fold(f32::INFINITY, f32::min);
            ui.strong(format!("Hyperedge {h}"));
            egui::Grid::new("hyperedge").num_columns(2).show(ui, |ui| {
                row(ui, "edges", edges.len().to_string());
                row(ui, "length", format!("{length:.1} µm"));
                appears(ui, first);
            });
        }
    }
    (select, ops)
}

/// One small table per dataset: column names, then the rows about the selected edge.
fn raw_tables(ui: &mut egui::Ui, tables: &[crate::devtools::RawTable]) {
    for t in tables.iter().filter(|t| !t.rows.is_empty()) {
        ui.label(egui::RichText::new(format!("{}  ({} rows)", t.dataset, t.rows.len())).strong().small());
        egui::Grid::new(&t.dataset).striped(true).show(ui, |ui| {
            for c in &t.columns {
                ui.label(egui::RichText::new(c).weak().small());
            }
            ui.end_row();
            for row in &t.rows {
                for v in row {
                    ui.monospace(v);
                }
                ui.end_row();
            }
        });
        ui.add_space(4.0);
    }
}
