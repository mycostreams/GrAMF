//! The Video registration workspace (DESIGN.md §10): sessions and the stage scatter on the
//! left, the selected position's runs on the right, a run slider at the bottom. R2 lists and
//! browses runs; placement, matching and corrections come in R3-R5.

use crate::store::Store;
use crate::time::{TimeAxis, TimeUnit};
use crate::videos::{Session, Videos};
use egui::{Color32, Pos2, Stroke};
use std::collections::HashSet;

/// Which workspace the window shows (the workflow buttons in the top bar).
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Workflow {
    #[default]
    Visualize,
    Registration,
}

/// The plate's video runs and what the user is looking at.
pub struct Registration {
    pub videos: Videos,
    pub sessions: Vec<Session>,
    pub session: usize,
    pub position: Option<usize>,
    /// Index into the selected position's runs.
    pub run: usize,
    /// Runs the store's flow layer already maps (`<position>/<timestamp>`).
    pub mapped_in_store: HashSet<String>,
}

impl Registration {
    pub fn new(videos: Videos, gap_minutes: f32, store: &Store) -> Self {
        let sessions = crate::videos::sessions(&videos.positions, gap_minutes);
        let mapped_in_store = store.flow_in_store.iter().flat_map(|f| f.run_paths.iter().cloned()).collect();
        Self { videos, sessions, session: 0, position: None, run: 0, mapped_in_store }
    }

    pub fn regroup(&mut self, gap_minutes: f32) {
        self.sessions = crate::videos::sessions(&self.videos.positions, gap_minutes);
        self.session = self.session.min(self.sessions.len().saturating_sub(1));
    }

    /// Acquisition time of the selected run.
    pub fn selected_run_utc(&self) -> Option<i64> {
        let p = &self.videos.positions[self.position?];
        p.runs.get(self.run).map(|r| r.utc)
    }

    fn position_mapped(&self, p: usize) -> bool {
        self.videos.positions[p].runs.iter().any(|r| self.mapped_in_store.contains(&r.rel_path))
    }
}

pub enum Action {
    ChooseFolder,
    None,
}

fn utc(t: i64) -> String {
    chrono::DateTime::from_timestamp(t, 0).map_or(t.to_string(), |d| d.format("%d %b %H:%M").to_string())
}

/// Left panel: where the runs come from, the sessions, and the stage scatter of the selected
/// session. Without runs, a button to pick the folder.
pub fn left_panel(ui: &mut egui::Ui, reg: Option<&mut Registration>, store: &Store) -> Action {
    let mut action = Action::None;
    ui.heading("Video runs");
    let Some(reg) = reg else {
        ui.label(egui::RichText::new("No polyflow_analysis folder found beside the store.").weak());
        if ui.button("Choose video runs folder…").clicked() {
            action = Action::ChooseFolder;
        }
        flow_layer_note(ui, store);
        return action;
    };
    let n_runs: usize = reg.videos.positions.iter().map(|p| p.runs.len()).sum();
    ui.label(format!("{} positions · {n_runs} runs · {}", reg.videos.positions.len(), reg.videos.setup.name()));
    if !reg.videos.skipped.is_empty() {
        ui.colored_label(Color32::YELLOW, format!("{} run folders skipped", reg.videos.skipped.len()))
            .on_hover_text(reg.videos.skipped.join("\n"));
    }
    if ui.small_button("Other folder…").on_hover_text(reg.videos.root.display().to_string()).clicked() {
        action = Action::ChooseFolder;
    }
    flow_layer_note(ui, store);

    ui.separator();
    ui.strong("Sessions");
    for (i, s) in reg.sessions.iter().enumerate() {
        let label = format!("{} – {} · {} pos", utc(s.start), utc(s.end), s.positions.len());
        if ui.selectable_label(reg.session == i, label).clicked() && reg.session != i {
            reg.session = i;
            reg.position = None;
        }
    }

    ui.separator();
    ui.strong("Stage positions");
    scatter(ui, reg);
    ui.horizontal(|ui| {
        // painted, not a glyph: the UI font has no "●"
        let dot = |ui: &mut egui::Ui, c: Color32, text: &str| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
            ui.painter().circle_filled(rect.center(), 4.0, c);
            ui.label(egui::RichText::new(text).small().weak());
        };
        dot(ui, MAPPED, "mapped in store");
        dot(ui, UNPLACED, "not mapped");
    });
    action
}

const MAPPED: Color32 = Color32::from_rgb(29, 158, 117);
const UNPLACED: Color32 = Color32::from_rgb(136, 135, 128);

fn flow_layer_note(ui: &mut egui::Ui, store: &Store) {
    if let Some(f) = &store.flow_in_store {
        ui.label(
            egui::RichText::new(format!(
                "store flow layer: {} runs, {} measurements ({}), read-only",
                f.run_paths.len(),
                f.n_measurements,
                if f.written_by.is_empty() { "unknown writer" } else { &f.written_by }
            ))
            .small()
            .weak(),
        );
    }
}

/// Stage positions of the selected session in stage mm (x right, y down); click selects.
fn scatter(ui: &mut egui::Ui, reg: &mut Registration) {
    let Some(session) = reg.sessions.get(reg.session) else {
        return;
    };
    let size = egui::vec2(ui.available_width(), 170.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let painter = ui.painter_at(rect);
    painter.rect_stroke(rect, 4.0, Stroke::new(0.5, ui.visuals().weak_text_color()), egui::StrokeKind::Inside);

    let stage = |p: usize| reg.videos.positions[p].stage_mm;
    let (mut min, mut max) = ([f64::MAX; 2], [f64::MIN; 2]);
    for &p in &session.positions {
        for k in 0..2 {
            min[k] = min[k].min(stage(p)[k]);
            max[k] = max[k].max(stage(p)[k]);
        }
    }
    // equal scale on both axes, centred, with a margin
    let inner = rect.shrink(14.0);
    let span = (max[0] - min[0]).max(max[1] - min[1]).max(1e-3);
    let scale = (inner.width().min(inner.height()) as f64) / span;
    let mid = [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0];
    let to_screen = |s: [f64; 2]| {
        Pos2::new(
            inner.center().x + ((s[0] - mid[0]) * scale) as f32,
            inner.center().y + ((s[1] - mid[1]) * scale) as f32,
        )
    };

    let mut hovered = None;
    for &p in &session.positions {
        let at = to_screen(stage(p));
        let colour = if reg.position_mapped(p) { MAPPED } else { UNPLACED };
        painter.circle_filled(at, 5.0, colour);
        if reg.position == Some(p) {
            painter.circle_stroke(at, 8.0, Stroke::new(2.0, Color32::from_rgb(55, 138, 221)));
        }
        if response.hover_pos().is_some_and(|h| h.distance(at) < 9.0) {
            hovered = Some(p);
        }
    }
    if let Some(p) = hovered {
        let pos = &reg.videos.positions[p];
        response.clone().on_hover_text(format!("{} · {} runs", pos.name, pos.runs.len()));
        if response.clicked() && reg.position != Some(p) {
            reg.position = Some(p);
            reg.run = 0;
        }
    }
}

/// Right panel: the selected position and its runs.
pub fn right_panel(ui: &mut egui::Ui, reg: Option<&mut Registration>, axis: Option<&TimeAxis>, unit: TimeUnit) {
    ui.heading("Position");
    let Some(reg) = reg else {
        return;
    };
    let Some(p) = reg.position else {
        ui.label(egui::RichText::new("Click a position in the stage scatter").weak());
        return;
    };
    let pos = &reg.videos.positions[p];
    ui.strong(&pos.name);
    ui.label(format!("stage x {:.3}  y {:.3} mm · {} runs", pos.stage_mm[0], pos.stage_mm[1], pos.runs.len()));
    if let Some(run) = pos.runs.get(reg.run) {
        ui.separator();
        ui.strong(format!("Run {} / {}", reg.run + 1, pos.runs.len()));
        egui::Grid::new("run").num_columns(2).show(ui, |ui| {
            let row = |ui: &mut egui::Ui, k: &str, v: String| {
                ui.label(egui::RichText::new(k).weak());
                ui.label(v);
                ui.end_row();
            };
            row(ui, "time", utc(run.utc));
            row(ui, "mode", run.mode.clone());
            row(ui, "run nr", run.run_nr.map_or("—".into(), |n| n.to_string()));
            row(ui, "scale", format!("{:.3} µm/px", run.delta_x_um));
            if let Some(axis) = axis {
                if let Some(f) = crate::videos::stg_frame(run.utc, &axis.timepoints) {
                    let offset_h = (axis.timepoints[f] - run.utc) as f64 / 3600.0;
                    row(ui, "STG frame", format!("{f} ({offset_h:+.1} h)"));
                    if unit != TimeUnit::Frame {
                        row(ui, "", axis.label(f, unit));
                    }
                }
            }
            row(ui, "in store", if reg.mapped_in_store.contains(&run.rel_path) { "mapped".into() } else { "no".into() });
        });
    }
    ui.separator();
    egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
        for (i, run) in pos.runs.iter().enumerate() {
            let mark = if reg.mapped_in_store.contains(&run.rel_path) { " · mapped" } else { "" };
            if ui.selectable_label(reg.run == i, format!("{} · {}{mark}", utc(run.utc), run.mode)).clicked() {
                reg.run = i;
            }
        }
    });
}

/// Bottom bar: step through the selected position's runs; the STG time follows the run.
pub fn run_bar(ui: &mut egui::Ui, reg: &mut Registration) {
    let Some(p) = reg.position else {
        ui.label(egui::RichText::new("Select a position to step through its runs").weak());
        return;
    };
    let n = reg.videos.positions[p].runs.len();
    ui.horizontal(|ui| {
        if ui.button("◀").on_hover_text("Previous run").clicked() {
            reg.run = reg.run.saturating_sub(1);
        }
        if ui.button("▶").on_hover_text("Next run").clicked() {
            reg.run = (reg.run + 1).min(n - 1);
        }
        ui.spacing_mut().slider_width = (ui.available_width() - 220.0).max(80.0);
        ui.add(egui::Slider::new(&mut reg.run, 0..=n - 1).show_value(false));
        let run = &reg.videos.positions[p].runs[reg.run];
        ui.label(format!("run {} / {n} · {}", reg.run + 1, utc(run.utc)));
    });
}
