//! View > Developer: a bottom dock with the store inspector, health checks, raw rows for the
//! selected edge, timings, and the matcher-evidence overlay toggle.

use crate::devtools::{Check, DatasetInfo, Evidence, Level};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Store,
    Health,
    RawRows,
    Timings,
}

/// Milliseconds of the last occurrence of each, and a smoothed frame time.
#[derive(Default)]
pub struct Timings {
    pub load_ms: f64,
    pub annotations_ms: f64,
    pub pick_ms: f64,
    pub export_ms: f64,
    pub evidence_ms: f64,
    pub frame_ms: f64,
}

#[derive(Default)]
pub struct DevState {
    pub open: bool,
    pub tab: Tab,
    /// Filled lazily the first time the dock opens for a store.
    pub datasets: Option<Result<Vec<DatasetInfo>, String>>,
    pub health: Vec<Check>,
    /// Raw rows and the edge they belong to.
    pub raw: Option<(usize, Vec<(String, Vec<String>)>)>,
    pub evidence_on: bool,
    pub evidence: Option<Result<Evidence, String>>,
    /// Frame the evidence overlay was last built for.
    pub evidence_frame: Option<usize>,
    pub timings: Timings,
}

impl DevState {
    /// Drops everything derived from the previous store.
    pub fn reset_store(&mut self) {
        self.datasets = None;
        self.health.clear();
        self.raw = None;
        self.evidence = None;
        self.evidence_frame = None;
    }
}

/// Draws the dock; returns true when the evidence toggle changed.
pub fn show(ui: &mut egui::Ui, d: &mut DevState) -> bool {
    let mut toggled = false;
    ui.horizontal(|ui| {
        for (tab, name) in [(Tab::Store, "Store"), (Tab::Health, "Health"), (Tab::RawRows, "Raw rows"), (Tab::Timings, "Timings")] {
            ui.selectable_value(&mut d.tab, tab, name);
        }
        ui.separator();
        toggled = ui.checkbox(&mut d.evidence_on, "Evidence overlay").changed();
        match &d.evidence {
            Some(Err(e)) if d.evidence_on => {
                ui.colored_label(egui::Color32::LIGHT_RED, e);
            }
            Some(Ok(ev)) if d.evidence_on => {
                ui.label(egui::RichText::new(if ev.arc_spans {
                    "cyan = arc spans the matcher found in this frame (dated or not); grey = rest"
                } else {
                    "matcher hits in this frame, coloured by score (yellow = close, purple = at threshold); grey = rest"
                })
                .weak());
            }
            _ => (),
        }
    });
    ui.separator();
    egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| match d.tab {
        Tab::Store => match &d.datasets {
            Some(Ok(list)) => {
                egui::Grid::new("datasets").striped(true).num_columns(4).show(ui, |ui| {
                    for name in ["dataset", "shape", "dtype", "attributes"] {
                        ui.strong(name);
                    }
                    ui.end_row();
                    for ds in list {
                        ui.monospace(&ds.path);
                        ui.monospace(format!("{:?}", ds.shape));
                        ui.monospace(&ds.dtype);
                        let attrs: Vec<String> = ds.attrs.iter().map(|(k, v)| format!("{k}={v}")).collect();
                        let full = attrs.join("\n");
                        let short: String = attrs.join("; ").chars().take(140).collect();
                        ui.label(egui::RichText::new(short).small()).on_hover_text(full);
                        ui.end_row();
                    }
                });
            }
            Some(Err(e)) => {
                ui.colored_label(egui::Color32::LIGHT_RED, e);
            }
            None => {
                ui.label("open a store");
            }
        },
        Tab::Health => {
            for c in &d.health {
                let (tag, colour) = match c.level {
                    Level::Ok => ("OK  ", egui::Color32::LIGHT_GREEN),
                    Level::Warn => ("WARN", egui::Color32::YELLOW),
                    Level::Fail => ("FAIL", egui::Color32::LIGHT_RED),
                };
                ui.horizontal(|ui| {
                    ui.monospace(egui::RichText::new(tag).color(colour));
                    ui.label(&c.text);
                });
            }
        }
        Tab::RawRows => match &d.raw {
            Some((e, rows)) => {
                ui.strong(format!("Every row about edge {e}"));
                for (name, lines) in rows {
                    egui::CollapsingHeader::new(format!("{name}  ({} rows)", lines.len()))
                        .id_salt(name)
                        .default_open(lines.len() <= 3)
                        .show(ui, |ui| {
                            for line in lines {
                                ui.monospace(line);
                            }
                        });
                }
            }
            None => {
                ui.label(egui::RichText::new("select an edge").weak());
            }
        },
        Tab::Timings => {
            let t = &d.timings;
            egui::Grid::new("timings").num_columns(2).show(ui, |ui| {
                for (name, ms) in [
                    ("frame (smoothed)", t.frame_ms),
                    ("store load + upload", t.load_ms),
                    ("annotation files + registry", t.annotations_ms),
                    ("last pick", t.pick_ms),
                    ("last export", t.export_ms),
                    ("last evidence build", t.evidence_ms),
                ] {
                    ui.label(name);
                    ui.monospace(format!("{ms:.1} ms"));
                    ui.end_row();
                }
            });
        }
    });
    toggled
}
