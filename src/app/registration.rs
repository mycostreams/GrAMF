//! The Video registration workspace (DESIGN.md §10): sessions and the stage scatter on the
//! left, the selected position's runs on the right, a run slider at the bottom, videos placed
//! on the map. Matching (R4) and corrections (R5) come later.

use crate::placement::{self, FitKind, Similarity};
use crate::settings::SetupDefault;
use crate::store::Store;
use crate::time::{TimeAxis, TimeUnit};
use crate::videos::{Session, VideoGraph, Videos};
use egui::{Color32, Pos2, Stroke};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum RegTool {
    /// Click a video on the map to select its position.
    #[default]
    Select,
    /// Click the map where the selected position's video centre belongs.
    Anchor,
}

/// A session's placement: the stage-to-STG transform, how it came about, and each anchor's
/// residual (position, µm).
pub struct SessionFit {
    pub sim: Similarity,
    pub kind: FitKind,
    /// No anchors of its own: carried over from the previous session.
    pub inherited: bool,
    pub residuals: Vec<(usize, f64)>,
}

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
    pub tool: RegTool,
    /// Position -> where its video centre sits on the STG (µm).
    pub anchors: BTreeMap<usize, [f64; 2]>,
    /// Sessions the user confirmed, keyed by their first position's name (stable when
    /// sessions are regrouped).
    pub verified: HashSet<String>,
    /// Camera orientation (`placement::camera_orient` option) for this plate's setup.
    pub camera: u8,
    /// The setup's default transform, and whether its offset can be trusted without anchors
    /// (ARETHA: plate imaging and videos share one machine).
    pub default: Option<Similarity>,
    pub trust_default_offset: bool,
    /// Loaded video graphs by run path (`None`: could not be read).
    graphs: HashMap<String, Option<Rc<VideoGraph>>>,
    /// Fit score per session, with the (transform, camera) it was computed for.
    scores: HashMap<usize, (String, Option<f32>)>,
}

impl Registration {
    pub fn new(videos: Videos, gap_minutes: f32, store: &Store, default: Option<SetupDefault>) -> Self {
        let sessions = crate::videos::sessions(&videos.positions, gap_minutes);
        let mapped_in_store = store.flow_in_store.iter().flat_map(|f| f.run_paths.iter().cloned()).collect();
        let trust_default_offset = videos.setup == crate::videos::Setup::Aretha;
        Self {
            videos,
            sessions,
            session: 0,
            position: None,
            run: 0,
            mapped_in_store,
            tool: RegTool::Select,
            anchors: BTreeMap::new(),
            verified: HashSet::new(),
            camera: default.map_or(0, |d| d.camera),
            default: default.map(|d| d.transform),
            trust_default_offset,
            graphs: HashMap::new(),
            scores: HashMap::new(),
        }
    }

    /// Every session's fit, in order: anchors refine the previous session's fit (the setup
    /// default for the first); a session without anchors inherits it, or uses the default
    /// when its offset is trusted. `None`: not placed.
    pub fn fits(&self) -> Vec<Option<SessionFit>> {
        let mut out: Vec<Option<SessionFit>> = vec![];
        for s in &self.sessions {
            let previous = out.iter().rev().flatten().next().map(|f| f.sim);
            let base = previous.or(self.default).unwrap_or_default();
            let anchored: Vec<usize> = s.positions.iter().copied().filter(|p| self.anchors.contains_key(p)).collect();
            let pairs: Vec<([f64; 2], [f64; 2])> =
                anchored.iter().map(|&p| (self.videos.positions[p].stage_mm, self.anchors[&p])).collect();
            let fit = if !pairs.is_empty() {
                let (sim, kind, res) = placement::fit(&pairs, base);
                Some(SessionFit { sim, kind, inherited: false, residuals: anchored.into_iter().zip(res).collect() })
            } else if let Some(sim) = previous {
                Some(SessionFit { sim, kind: FitKind::Default, inherited: true, residuals: vec![] })
            } else if self.trust_default_offset {
                self.default.map(|sim| SessionFit { sim, kind: FitKind::Default, inherited: false, residuals: vec![] })
            } else {
                None
            };
            out.push(fit);
        }
        out
    }

    pub fn session_of(&self, p: usize) -> Option<usize> {
        self.sessions.iter().position(|s| s.positions.contains(&p))
    }

    pub fn session_key(&self, i: usize) -> Option<String> {
        self.sessions.get(i).map(|s| self.videos.positions[s.positions[0]].name.clone())
    }

    /// The run shown for position `p`: the selected run for the selected position, else the
    /// run closest in time to it (the latest run when nothing is selected).
    pub fn shown_run(&self, p: usize) -> usize {
        let runs = &self.videos.positions[p].runs;
        if self.position == Some(p) {
            return self.run.min(runs.len() - 1);
        }
        match self.selected_run_utc() {
            Some(t) => (0..runs.len()).min_by_key(|&i| (runs[i].utc - t).abs()).unwrap_or(0),
            None => runs.len() - 1,
        }
    }

    /// The video graph of run `run` of position `p`, loaded on first use.
    pub fn graph(&mut self, p: usize, run: usize) -> Option<Rc<VideoGraph>> {
        let r = &self.videos.positions[p].runs[run];
        let root = &self.videos.root;
        self.graphs
            .entry(r.rel_path.clone())
            .or_insert_with(|| crate::videos::load_graph(root, r).ok().map(Rc::new))
            .clone()
    }

    /// Sampled video-graph points of the session's positions (shown runs), placed by `sim`
    /// with camera option `camera`.
    fn placed_samples(&mut self, i: usize, sim: &Similarity, camera: u8) -> Vec<[f32; 2]> {
        let positions = self.sessions[i].positions.clone();
        let mut out = vec![];
        for p in positions {
            let stage = self.videos.positions[p].stage_mm;
            if let Some(g) = self.graph(p, self.shown_run(p)) {
                for e in &g.edges {
                    out.extend(e.points.iter().step_by(5).map(|&q| placement::place(sim, camera, stage, q)));
                }
            }
        }
        out
    }

    /// Fit score of session `i` (mean capped distance of its placed videos to the STG),
    /// recomputed only when its transform or the camera changed.
    pub fn score(&mut self, i: usize, fit: &SessionFit, store: &Store) -> Option<f32> {
        let key = format!("{:?}/{}/{:?}", fit.sim, self.camera, self.selected_run_utc());
        if let Some((k, s)) = self.scores.get(&i) {
            if *k == key {
                return *s;
            }
        }
        let points = self.placed_samples(i, &fit.sim, self.camera);
        let grid = store.grid();
        let s = placement::fit_score(points.into_iter(), |q| grid.nearest(&store.arcs, q, placement::SCORE_CAP_UM).map(|x| x.0));
        self.scores.insert(i, (key, s));
        s
    }

    /// Tries all 8 camera orientations on session `i` and keeps the best; returns the scores.
    pub fn detect_camera(&mut self, i: usize, sim: &Similarity, store: &Store) -> Vec<(u8, f32)> {
        let grid = store.grid();
        let mut scores: Vec<(u8, f32)> = (0..8u8)
            .filter_map(|k| {
                let points = self.placed_samples(i, sim, k);
                let s = placement::fit_score(points.into_iter(), |q| grid.nearest(&store.arcs, q, placement::SCORE_CAP_UM).map(|x| x.0))?;
                Some((k, s))
            })
            .collect();
        scores.sort_by(|a, b| a.1.total_cmp(&b.1));
        if let Some(&(best, _)) = scores.first() {
            self.camera = best;
        }
        scores
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
    DetectCamera,
    SaveDefault,
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
    let fits = reg.fits();
    for (i, s) in reg.sessions.iter().enumerate() {
        let status = session_status(reg, i, fits[i].as_ref());
        let label = format!("{} – {} · {} pos · {status}", utc(s.start), utc(s.end), s.positions.len());
        if ui.selectable_label(reg.session == i, label).clicked() && reg.session != i {
            reg.session = i;
            reg.position = None;
        }
    }
    if let Some(Some(fit)) = fits.get(reg.session) {
        session_details(ui, reg, fit, store, &mut action);
    }

    ui.separator();
    ui.strong("Stage positions");
    scatter(ui, reg, &fits);
    ui.horizontal_wrapped(|ui| {
        // painted, not a glyph: the UI font has no "●"
        let dot = |ui: &mut egui::Ui, c: Color32, text: &str| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
            ui.painter().circle_filled(rect.center(), 4.0, c);
            ui.label(egui::RichText::new(text).small().weak());
        };
        dot(ui, ANCHORED, "anchored");
        dot(ui, PLACED, "placed");
        dot(ui, UNPLACED, "not placed");
        dot(ui, MAPPED, "mapped in store");
    });
    action
}

fn session_status(reg: &Registration, i: usize, fit: Option<&SessionFit>) -> String {
    let Some(fit) = fit else {
        return "not placed".into();
    };
    let verified = reg.session_key(i).is_some_and(|k| reg.verified.contains(&k));
    let how = match (fit.inherited, fit.residuals.len()) {
        (true, _) => "from previous session".to_string(),
        (false, 0) => "setup default".to_string(),
        (false, 1) => "1 anchor".to_string(),
        (false, n) => format!("{n} anchors"),
    };
    format!("{how} · {}", if verified { "verified" } else { "unverified" })
}

/// The selected session's fit: score, anchors with residuals, and its buttons.
fn session_details(ui: &mut egui::Ui, reg: &mut Registration, fit: &SessionFit, store: &Store, action: &mut Action) {
    let i = reg.session;
    let score = reg.score(i, fit, store);
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.label(format!(
            "scale {:.1} µm/mm · rotation {:.2}° · {}mirrored · camera {}",
            fit.sim.scale,
            fit.sim.theta.to_degrees(),
            if fit.sim.mirror { "" } else { "not " },
            reg.camera
        ));
        match score {
            Some(s) => ui.label(format!("fit score {s:.1} µm"))
                .on_hover_text("mean distance from the placed video graphs to the nearest STG arc (capped at 50 µm); lower is better"),
            None => ui.label(egui::RichText::new("fit score: no video graphs").weak()),
        };
        for &(p, r) in &fit.residuals {
            ui.horizontal(|ui| {
                ui.label(format!("{} · {r:.1} µm", reg.videos.positions[p].name));
                if ui.small_button("remove").clicked() {
                    reg.anchors.remove(&p);
                }
            });
        }
        let key = reg.session_key(i).unwrap_or_default();
        let mut verified = reg.verified.contains(&key);
        if ui.checkbox(&mut verified, "Verified").on_hover_text("you checked that these videos sit on the right hyphae").changed() {
            if verified { reg.verified.insert(key) } else { reg.verified.remove(&key) };
        }
        // stacked, not side by side: a wide row would widen the whole panel
        ui.vertical(|ui| {
            if ui.small_button("Detect camera orientation").on_hover_text("tries the 8 rotations / mirrors of the camera and keeps the best fit").clicked() {
                *action = Action::DetectCamera;
            }
            if fit.residuals.len() >= 2 && ui.small_button(format!("Save as {} default", reg.videos.setup.name())).clicked() {
                *action = Action::SaveDefault;
            }
        });
    });
}

const MAPPED: Color32 = Color32::from_rgb(29, 158, 117);
const ANCHORED: Color32 = Color32::from_rgb(55, 138, 221);
const PLACED: Color32 = Color32::from_rgb(210, 210, 205);
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

/// Positions of the selected session, oriented like the map; click selects.
fn scatter(ui: &mut egui::Ui, reg: &mut Registration, fits: &[Option<SessionFit>]) {
    let Some(session) = reg.sessions.get(reg.session) else {
        return;
    };
    let size = egui::vec2(ui.available_width(), 170.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let painter = ui.painter_at(rect);
    painter.rect_stroke(rect, 4.0, Stroke::new(0.5, ui.visuals().weak_text_color()), egui::StrokeKind::Inside);

    // Drawn the way the map shows them: placed positions where the session's placement puts
    // them (STG axes, rows down); unplaced sessions in stage axes with y up.
    let fit = fits.get(reg.session).and_then(|f| f.as_ref()).map(|f| f.sim);
    let stage = |p: usize| {
        let s = reg.videos.positions[p].stage_mm;
        fit.map_or([s[0], -s[1]], |sim| sim.apply(s))
    };
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
        let colour = if reg.anchors.contains_key(&p) {
            ANCHORED
        } else if fits.get(reg.session).is_some_and(|f| f.is_some()) {
            PLACED
        } else {
            UNPLACED
        };
        painter.circle_filled(at, 5.0, colour);
        if reg.position_mapped(p) {
            painter.circle_stroke(at, 6.5, Stroke::new(1.5, MAPPED));
        }
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
    match reg.anchors.get(&p) {
        Some(w) => {
            let w = *w;
            ui.horizontal(|ui| {
                ui.label(format!("anchored at {:.0}, {:.0} µm", w[0], w[1]));
                if ui.small_button("remove").clicked() {
                    reg.anchors.remove(&p);
                }
            });
        }
        None => {
            ui.label(egui::RichText::new("no anchor: Anchor tool (A), then click where this video's centre belongs").weak().small());
        }
    }
    let pos = &reg.videos.positions[p];
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

/// Tool buttons for the registration workspace.
pub fn toolbar(ui: &mut egui::Ui, reg: &mut Registration) {
    for (tool, label) in [(RegTool::Select, "Select (S)"), (RegTool::Anchor, "Anchor (A)")] {
        if ui.selectable_label(reg.tool == tool, label).clicked() {
            reg.tool = tool;
        }
    }
    ui.separator();
    let hint = match (reg.tool, reg.position) {
        (RegTool::Select, _) => "click a video on the map, or a dot in the stage scatter",
        (RegTool::Anchor, None) => "select a position first (stage scatter or map)",
        (RegTool::Anchor, Some(_)) => "click the STG where this video's centre belongs (snaps to a node within 10 px)",
    };
    ui.label(egui::RichText::new(hint).weak());
}

/// Placed videos of the selected session, in points: each shown run's graph (the selected
/// position bright), its frame outline, and anchors with a line to their video's centre.
pub fn overlay(reg: &mut Registration, to_pt: impl Fn([f32; 2]) -> Pos2) -> Vec<egui::Shape> {
    use egui::Shape;
    let fits = reg.fits();
    let Some(Some(fit)) = fits.get(reg.session) else {
        return vec![];
    };
    let sim = fit.sim;
    let camera = reg.camera;
    let mut shapes = vec![];
    for p in reg.sessions[reg.session].positions.clone() {
        let run = reg.shown_run(p);
        let (stage, frame, dx) = {
            let pos = &reg.videos.positions[p];
            (pos.stage_mm, pos.runs[run].frame_size, pos.runs[run].delta_x_um)
        };
        let selected = reg.position == Some(p);
        let colour = if selected { Color32::from_rgb(80, 220, 240) } else { Color32::from_rgba_unmultiplied(150, 190, 230, 150) };
        let place = |q: [f32; 2]| to_pt(placement::place(&sim, camera, stage, q));
        let (hw, hh) = (frame[1] as f32 * dx / 2.0, frame[0] as f32 * dx / 2.0);
        let corners: Vec<Pos2> = [[-hw, -hh], [hw, -hh], [hw, hh], [-hw, hh], [-hw, -hh]].into_iter().map(place).collect();
        shapes.extend(Shape::dashed_line(&corners, Stroke::new(1.0, colour), 4.0, 3.0));
        if let Some(g) = reg.graph(p, run) {
            for e in &g.edges {
                shapes.push(Shape::line(e.points.iter().map(|&q| place(q)).collect(), Stroke::new(if selected { 2.5 } else { 1.5 }, colour)));
            }
        }
        if let Some(&a) = reg.anchors.get(&p) {
            let centre = to_pt({
                let c = sim.apply(stage);
                [c[0] as f32, c[1] as f32]
            });
            let at = to_pt([a[0] as f32, a[1] as f32]);
            shapes.push(Shape::line_segment([centre, at], Stroke::new(1.5, ANCHORED)));
            shapes.push(Shape::circle_stroke(at, 6.0, Stroke::new(2.0, ANCHORED)));
        }
    }
    shapes
}

/// Selects the position whose placed video graph passes nearest `at` (µm), within `radius`.
pub fn pick_video(reg: &mut Registration, at: [f32; 2], radius: f32) -> Option<usize> {
    let fits = reg.fits();
    let fit = fits.get(reg.session)?.as_ref()?;
    let mut best: Option<(f32, usize)> = None;
    for p in reg.sessions[reg.session].positions.clone() {
        let stage = reg.videos.positions[p].stage_mm;
        let Some(g) = reg.graph(p, reg.shown_run(p)) else { continue };
        for q in g.edges.iter().flat_map(|e| e.points.iter()) {
            let w = placement::place(&fit.sim, reg.camera, stage, *q);
            let d = ((w[0] - at[0]).powi(2) + (w[1] - at[1]).powi(2)).sqrt();
            if d <= radius && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, p));
            }
        }
    }
    best.map(|(_, p)| p)
}
