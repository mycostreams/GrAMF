//! The Video registration workspace (DESIGN.md §10): sessions and the stage scatter on the
//! left, the selected position's runs on the right, a run slider at the bottom, videos placed
//! on the map. Matching (R4) and corrections (R5) come later.

use crate::annotations::{Op, RegistrationLayer};
use crate::flow::RunFlow;
use crate::matching::{self, AddedSegment, EdgeCover, EdgeRef, Rigid};
use crate::placement::{self, Similarity};
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
    /// With a video edge selected: click STG edges to add them to its chain or remove them.
    Assign,
    /// Click a video edge to mark it as an artifact (or unmark it).
    Disable,
    /// Drag the selected run's video graph; the snap then refines from there.
    Nudge,
}

impl RegTool {
    pub const ALL: [(RegTool, egui::Key, &'static str); 5] = [
        (RegTool::Select, egui::Key::S, "Select (S)"),
        (RegTool::Anchor, egui::Key::A, "Anchor (A)"),
        (RegTool::Assign, egui::Key::G, "Assign (G)"),
        (RegTool::Disable, egui::Key::D, "Disable (D)"),
        (RegTool::Nudge, egui::Key::N, "Nudge (N)"),
    ];
}

/// One run matched onto the STG: its snap, each video edge's placed (snapped) trail and cover,
/// and its flow values.
pub struct RunMatch {
    /// `None`: snap refused (too little to fit on, or an implausibly large move).
    pub snap: Option<Rigid>,
    pub placed: Vec<Vec<[f32; 2]>>,
    pub covers: Vec<EdgeCover>,
    pub flow: Result<RunFlow, String>,
    /// Per video edge: chain set by hand / marked as an artifact.
    pub assigned: Vec<bool>,
    pub disabled: Vec<bool>,
    /// The nudge applied before the snap (µm).
    pub nudge: [f32; 2],
}

/// Distinct colours for a run's video edges (and their STG chains).
pub const EDGE_COLOURS: [Color32; 8] = [
    Color32::from_rgb(80, 220, 240),
    Color32::from_rgb(240, 150, 40),
    Color32::from_rgb(170, 120, 240),
    Color32::from_rgb(110, 220, 110),
    Color32::from_rgb(240, 100, 160),
    Color32::from_rgb(240, 220, 80),
    Color32::from_rgb(120, 160, 255),
    Color32::from_rgb(255, 120, 90),
];
const GAP: Color32 = Color32::from_rgb(240, 170, 40);

/// A session's placement: the stage-to-STG transform, how it came about, and each anchor's
/// residual (position, µm).
pub struct SessionFit {
    pub sim: Similarity,
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
    /// The author's registration as last read from the annotation file in memory (the one
    /// source of truth); the workspace proposes changes as `pending` ops.
    pub layer: RegistrationLayer,
    pub pending: Vec<Op>,
    /// Position index -> anchor, rebuilt from `layer` (keyed there by position name).
    pub anchors: BTreeMap<usize, [f64; 2]>,
    /// The setup default's camera orientation, used until the layer sets one.
    default_camera: u8,
    /// A nudge being dragged (µm), shown live on the selected run before it is committed.
    pub drag_nudge: Option<[f32; 2]>,
    /// What "Copy corrections from previous run" did last, and for which run.
    pub copied_note: Option<(String, String)>,
    /// The setup's default transform, and whether its offset can be trusted without anchors
    /// (ARETHA: plate imaging and videos share one machine).
    pub default: Option<Similarity>,
    pub trust_default_offset: bool,
    /// Loaded video graphs by run path (`None`: could not be read).
    graphs: HashMap<String, Option<Rc<VideoGraph>>>,
    /// Fit score per session, with the (transform, camera) it was computed for.
    scores: HashMap<usize, (String, Option<f32>)>,
    /// The selected video edge of the selected run.
    pub video_edge: Option<[i64; 3]>,
    /// Matched runs by run path, with the inputs they were matched with.
    matches: HashMap<String, (String, Rc<RunMatch>)>,
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
            layer: RegistrationLayer::default(),
            pending: vec![],
            anchors: BTreeMap::new(),
            default_camera: default.map_or(0, |d| d.camera),
            drag_nudge: None,
            copied_note: None,
            default: default.map(|d| d.transform),
            trust_default_offset,
            graphs: HashMap::new(),
            scores: HashMap::new(),
            video_edge: None,
            matches: HashMap::new(),
        }
    }

    /// Takes the author's registration layer as the current state (anchors by position).
    pub fn sync(&mut self, layer: &RegistrationLayer) {
        if *layer == self.layer {
            return;
        }
        self.layer = layer.clone();
        self.anchors = self
            .videos
            .positions
            .iter()
            .enumerate()
            .filter_map(|(i, p)| Some((i, *layer.anchors.get(&p.name)?)))
            .collect();
    }

    pub fn camera(&self) -> u8 {
        self.layer.camera.unwrap_or(self.default_camera)
    }

    /// Distance (µm) within which a video edge counts as lying on an STG edge.
    pub fn threshold(&self) -> f32 {
        self.layer.threshold_um.unwrap_or(10.0)
    }

    pub fn snap(&self) -> bool {
        self.layer.snap.unwrap_or(true)
    }

    /// Proposes new camera / threshold / snap values (applied to the annotation file).
    pub fn set_params(&mut self, camera: u8, threshold: f32, snap: bool) {
        self.pending.push(Op::SetRegistrationParams(Some(camera), Some(threshold), Some(snap)));
    }

    pub fn run_path(&self, p: usize, run: usize) -> String {
        self.videos.positions[p].runs[run].rel_path.clone()
    }

    /// Run `run` of position `p` matched onto the STG under `sim`, recomputed only when the
    /// transform, camera, threshold, snap switch, added edges or the run's corrections change.
    pub fn run_match(&mut self, p: usize, run: usize, sim: &Similarity, store: &Store, added: &[AddedSegment]) -> Option<Rc<RunMatch>> {
        let rel = self.run_path(p, run);
        let live = if self.position == Some(p) && self.run == run { self.drag_nudge } else { None };
        let nudge = live.or_else(|| self.layer.nudges.get(&rel).copied()).unwrap_or([0.0; 2]);
        let assignments: Vec<_> = self.layer.assignments.iter().filter(|(k, _)| k.0 == rel).collect();
        let disabled: Vec<_> = self.layer.disabled.iter().filter(|k| k.0 == rel).collect();
        let key = format!(
            "{sim:?}/{}/{}/{}/{}/{nudge:?}/{assignments:?}/{disabled:?}",
            self.camera(),
            self.threshold(),
            self.snap(),
            added.len()
        );
        if let Some((k, m)) = self.matches.get(&rel) {
            if *k == key {
                return Some(m.clone());
            }
        }
        let graph = self.graph(p, run)?;
        let stage = self.videos.positions[p].stage_mm;
        let raw: Vec<Vec<[f32; 2]>> = graph
            .edges
            .iter()
            .map(|e| {
                e.points
                    .iter()
                    .map(|&q| {
                        let w = placement::place(sim, self.camera(), stage, q);
                        [w[0] + nudge[0], w[1] + nudge[1]]
                    })
                    .collect()
            })
            .collect();
        let snap = if self.snap() {
            let samples: Vec<[f32; 2]> = raw.iter().flat_map(|e| e.iter().step_by(5).copied()).collect();
            matching::snap(store, &samples)
        } else {
            None
        };
        let rigid = snap.unwrap_or_default();
        let placed: Vec<Vec<[f32; 2]>> = raw.iter().map(|e| e.iter().map(|&q| rigid.apply(q)).collect()).collect();
        let threshold = self.threshold();
        let mut covers: Vec<EdgeCover> =
            graph.edges.iter().zip(&placed).map(|(e, pts)| matching::cover(store, added, e.key, pts, threshold)).collect();
        let mut assigned = vec![false; covers.len()];
        for (c, a) in covers.iter_mut().zip(assigned.iter_mut()) {
            if let Some(chain) = self.layer.assignments.get(&(rel.clone(), c.key)) {
                c.chain = chain.clone();
                *a = true;
            }
        }
        let disabled = covers.iter().map(|c| self.layer.disabled.contains(&(rel.clone(), c.key))).collect();
        let flow = crate::flow::read(&self.videos.root.join(&rel));
        let m = Rc::new(RunMatch { snap, placed, covers, flow, assigned, disabled, nudge });
        self.matches.insert(rel, (key, m.clone()));
        Some(m)
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
                let (sim, _, res) = placement::fit(&pairs, base);
                Some(SessionFit { sim, inherited: false, residuals: anchored.into_iter().zip(res).collect() })
            } else if let Some(sim) = previous {
                Some(SessionFit { sim, inherited: true, residuals: vec![] })
            } else if self.trust_default_offset {
                self.default.map(|sim| SessionFit { sim, inherited: false, residuals: vec![] })
            } else {
                None
            };
            out.push(fit);
        }
        out
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
        let key = format!("{:?}/{}/{:?}", fit.sim, self.camera(), self.selected_run_utc());
        if let Some((k, s)) = self.scores.get(&i) {
            if *k == key {
                return *s;
            }
        }
        let points = self.placed_samples(i, &fit.sim, self.camera());
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
            self.set_params(best, self.threshold(), self.snap());
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
    let verified = reg.session_key(i).is_some_and(|k| reg.layer.verified.contains(&k));
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
            reg.camera()
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
                    let name = reg.videos.positions[p].name.clone();
                    reg.pending.push(Op::SetAnchor(name, None));
                }
            });
        }
        let key = reg.session_key(i).unwrap_or_default();
        let mut verified = reg.layer.verified.contains(&key);
        if ui.checkbox(&mut verified, "Verified").on_hover_text("you checked that these videos sit on the right hyphae").changed() {
            reg.pending.push(Op::SetVerified(key, verified));
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
pub fn right_panel(
    ui: &mut egui::Ui,
    reg: Option<&mut Registration>,
    axis: Option<&TimeAxis>,
    unit: TimeUnit,
    store: &Store,
    added: &[AddedSegment],
) {
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
                    let name = reg.videos.positions[p].name.clone();
                    reg.pending.push(Op::SetAnchor(name, None));
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
    let n_runs = pos.runs.len();
    let mut chosen = None;
    egui::CollapsingHeader::new(format!("All runs ({n_runs})")).show(ui, |ui| {
        egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
            for (i, run) in pos.runs.iter().enumerate() {
                let mark = if reg.mapped_in_store.contains(&run.rel_path) { " · mapped" } else { "" };
                if ui.selectable_label(reg.run == i, format!("{} · {}{mark}", utc(run.utc), run.mode)).clicked() {
                    chosen = Some(i);
                }
            }
        });
    });
    if let Some(i) = chosen {
        reg.run = i;
    }
    edges_section(ui, reg, store, added);
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
    for (tool, _, label) in RegTool::ALL {
        if ui.selectable_label(reg.tool == tool, label).clicked() {
            reg.tool = tool;
        }
    }
    ui.separator();
    let (mut threshold, mut snap) = (reg.threshold(), reg.snap());
    let t = ui
        .add(egui::DragValue::new(&mut threshold).range(2.0..=40.0).speed(0.2).suffix(" µm").prefix("threshold "))
        .on_hover_text("how close a video edge must lie to an STG edge to count as on it");
    let s = ui.checkbox(&mut snap, "snap").on_hover_text("fit each run onto the STG with a small rigid move");
    if t.changed() || s.changed() {
        reg.set_params(reg.camera(), threshold, snap);
    }
    ui.separator();
    let hint = match (reg.tool, reg.position) {
        (RegTool::Select, None) => "click a video on the map, or a dot in the stage scatter",
        (RegTool::Select, Some(_)) => "click a video edge to see its STG chain and flow",
        (RegTool::Anchor, None) => "select a position first (stage scatter or map)",
        (RegTool::Anchor, Some(_)) => "click the STG where this video's centre belongs (snaps to a node within 10 px)",
        (RegTool::Assign, _) if reg.video_edge.is_none() => "select a video edge first (right panel or Select tool)",
        (RegTool::Assign, _) => "click STG edges to add them to the selected video edge's chain, or remove them",
        (RegTool::Disable, _) => "click a video edge to mark it as an artifact (no flow), or unmark it",
        (RegTool::Nudge, None) => "select a position first",
        (RegTool::Nudge, Some(_)) => "drag to move this run's video graph; the snap refines from where you leave it",
    };
    ui.label(egui::RichText::new(hint).weak());
}

/// Placed videos of the selected session, in points: each shown run's snapped graph and frame
/// outline; for the selected position every video edge in its own colour, with its STG chain
/// in the same colour and its gaps dashed; anchors with a line to their video's centre.
pub fn overlay(reg: &mut Registration, store: &Store, added: &[AddedSegment], to_pt: impl Fn([f32; 2]) -> Pos2) -> Vec<egui::Shape> {
    use egui::Shape;
    let fits = reg.fits();
    let Some(Some(fit)) = fits.get(reg.session) else {
        return vec![];
    };
    let sim = fit.sim;
    let camera = reg.camera();
    let mut shapes = vec![];
    for p in reg.sessions[reg.session].positions.clone() {
        let run = reg.shown_run(p);
        let (stage, frame, dx) = {
            let pos = &reg.videos.positions[p];
            (pos.stage_mm, pos.runs[run].frame_size, pos.runs[run].delta_x_um)
        };
        let selected = reg.position == Some(p);
        let faint = Color32::from_rgba_unmultiplied(150, 190, 230, 150);
        let Some(m) = reg.run_match(p, run, &sim, store, added) else { continue };
        let rigid = m.snap.unwrap_or_default();
        let place = |q: [f32; 2]| to_pt(rigid.apply(placement::place(&sim, camera, stage, q)));
        let (hw, hh) = (frame[1] as f32 * dx / 2.0, frame[0] as f32 * dx / 2.0);
        let corners: Vec<Pos2> = [[-hw, -hh], [hw, -hh], [hw, hh], [-hw, hh], [-hw, -hh]].into_iter().map(place).collect();
        let outline = if selected { Color32::from_rgb(80, 220, 240) } else { faint };
        shapes.extend(Shape::dashed_line(&corners, Stroke::new(1.0, outline), 4.0, 3.0));
        for (i, (pts, cover)) in m.placed.iter().zip(&m.covers).enumerate() {
            let line: Vec<Pos2> = pts.iter().map(|&q| to_pt(q)).collect();
            if !selected {
                shapes.push(Shape::line(line, Stroke::new(1.5, faint)));
                continue;
            }
            let colour = EDGE_COLOURS[i % EDGE_COLOURS.len()];
            let chosen = reg.video_edge == Some(cover.key);
            for &(e, _) in &cover.chain {
                let stg: Vec<Pos2> = match e {
                    EdgeRef::Pipeline(e) => store.arcs.points[store.arcs.offsets[e]..store.arcs.offsets[e + 1]].iter().map(|&q| to_pt(q)).collect(),
                    EdgeRef::Added(id) => added.iter().filter(|a| a.id == id).flat_map(|a| [to_pt(a.a), to_pt(a.b)]).collect(),
                };
                let a = if chosen { 200 } else { 110 };
                shapes.push(Shape::line(stg, Stroke::new(if chosen { 6.0 } else { 4.0 }, Color32::from_rgba_unmultiplied(colour.r(), colour.g(), colour.b(), a))));
            }
            shapes.push(Shape::line(line, Stroke::new(if chosen { 3.5 } else { 2.0 }, colour)));
            for gap in &cover.gaps {
                let g: Vec<Pos2> = gap.iter().map(|&q| to_pt(q)).collect();
                shapes.extend(Shape::dashed_line(&g, Stroke::new(3.0, GAP), 5.0, 4.0));
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

/// A click with the Select tool: the selected position's video edge under `at` (µm) when
/// there is one, else the position whose placed video passes nearest.
pub fn pick(reg: &mut Registration, store: &Store, added: &[AddedSegment], at: [f32; 2], radius: f32) {
    let fits = reg.fits();
    let Some(Some(fit)) = fits.get(reg.session) else { return };
    let sim = fit.sim;
    let d = |pts: &[[f32; 2]]| pts.windows(2).map(|w| crate::store::segment_dist2(at, w[0], w[1]).sqrt()).fold(f32::INFINITY, f32::min);
    if let Some(p) = reg.position {
        let run = reg.shown_run(p);
        if let Some(m) = reg.run_match(p, run, &sim, store, added) {
            let hit = m.placed.iter().zip(&m.covers).map(|(pts, c)| (d(pts), c.key)).filter(|x| x.0 <= radius).min_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((_, key)) = hit {
                reg.video_edge = Some(key);
                return;
            }
        }
    }
    let mut best: Option<(f32, usize)> = None;
    for p in reg.sessions[reg.session].positions.clone() {
        let run = reg.shown_run(p);
        let Some(m) = reg.run_match(p, run, &sim, store, added) else { continue };
        let dist = m.placed.iter().map(|pts| d(pts)).fold(f32::INFINITY, f32::min);
        if dist <= radius && best.is_none_or(|(bd, _)| dist < bd) {
            best = Some((dist, p));
        }
    }
    if let Some((_, p)) = best {
        if reg.position != Some(p) {
            reg.position = Some(p);
            reg.run = reg.shown_run(p);
            reg.video_edge = None;
        }
    }
}

/// The selected run's video edges: coverage and chain per edge, the selected edge's chain and
/// flow values, and the snap.
pub fn edges_section(ui: &mut egui::Ui, reg: &mut Registration, store: &Store, added: &[AddedSegment]) {
    let Some(p) = reg.position else { return };
    let fits = reg.fits();
    let Some(Some(fit)) = fits.get(reg.session) else {
        ui.label(egui::RichText::new("place the session to match its videos").weak());
        return;
    };
    let run = reg.shown_run(p);
    let Some(m) = reg.run_match(p, run, &fit.sim, store, added) else {
        ui.colored_label(Color32::LIGHT_RED, "could not read this run's video graph");
        return;
    };
    ui.separator();
    match m.snap {
        Some(r) => ui.label(format!("snap: {:.1}, {:.1} µm · {:.2}°", r.shift[0], r.shift[1], r.theta.to_degrees())),
        None if reg.snap() => ui.colored_label(Color32::YELLOW, "snap refused (too little STG nearby, or a large move)"),
        None => ui.label(egui::RichText::new("snap off").weak()),
    };
    let rel = reg.run_path(p, run);
    if m.nudge != [0.0; 2] {
        ui.horizontal(|ui| {
            ui.label(format!("nudge: {:.1}, {:.1} µm", m.nudge[0], m.nudge[1]));
            if ui.small_button("reset").clicked() {
                reg.pending.push(Op::SetNudge(rel.clone(), None));
            }
        });
    }
    run_corrections(ui, reg, p, run);
    let flow = m.flow.as_ref().ok();
    if let Err(e) = &m.flow {
        ui.colored_label(Color32::LIGHT_RED, format!("flow: {e}"));
    }
    ui.strong("Video edges");
    egui::Grid::new("video_edges").num_columns(4).striped(true).show(ui, |ui| {
        for (i, c) in m.covers.iter().enumerate() {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 2.0, EDGE_COLOURS[i % EDGE_COLOURS.len()]);
            let has_flow = flow.is_some_and(|f| f.contains_key(&c.key));
            let mut label = egui::RichText::new(format!("({}, {}, {})", c.key[0], c.key[1], c.key[2]));
            if m.disabled[i] {
                label = label.strikethrough().weak();
            }
            if ui.selectable_label(reg.video_edge == Some(c.key), label).clicked() {
                reg.video_edge = Some(c.key);
            }
            let cov = format!("{:.0}%", c.coverage * 100.0);
            ui.label(if c.coverage < 0.9 { egui::RichText::new(cov).color(GAP) } else { egui::RichText::new(cov) });
            let mut chain = match (c.chain.len(), has_flow) {
                (0, _) => "no STG".to_string(),
                (n, true) => format!("{n} STG"),
                (n, false) => format!("{n} STG · no flow"),
            };
            if m.assigned[i] {
                chain += " · edited";
            }
            if m.disabled[i] {
                chain += " · artifact";
            }
            ui.label(egui::RichText::new(chain).small());
            ui.end_row();
        }
    });
    let Some(ci) = reg.video_edge.and_then(|k| m.covers.iter().position(|c| c.key == k)) else { return };
    let c = &m.covers[ci];
    ui.separator();
    ui.strong(format!("Video edge ({}, {}, {})", c.key[0], c.key[1], c.key[2]));
    let run_edge = (rel.clone(), c.key);
    let mut artifact = m.disabled[ci];
    if ui.checkbox(&mut artifact, "artifact (no flow)").changed() {
        reg.pending.push(Op::SetDisabled(run_edge.clone(), artifact));
    }
    for (j, (e, dir)) in c.chain.iter().enumerate() {
        ui.horizontal(|ui| {
            let id = match e {
                EdgeRef::Pipeline(e) => format!("edge {e}"),
                EdgeRef::Added(id) => format!("added {id}"),
            };
            ui.label(format!("{id} {}", if *dir > 0 { "(node1 to node2)" } else { "(node2 to node1)" }));
            if ui.small_button("flip").clicked() {
                let mut chain = c.chain.clone();
                chain[j].1 = -chain[j].1;
                reg.pending.push(Op::SetAssignment(run_edge.clone(), Some(chain)));
            }
            if ui.small_button("remove").clicked() {
                let mut chain = c.chain.clone();
                chain.remove(j);
                reg.pending.push(Op::SetAssignment(run_edge.clone(), Some(chain)));
            }
        });
    }
    if m.assigned[ci] && ui.small_button("Back to automatic").clicked() {
        reg.pending.push(Op::SetAssignment(run_edge.clone(), None));
    }
    let chain: Vec<String> = c
        .chain
        .iter()
        .map(|(e, dir)| {
            let id = match e {
                EdgeRef::Pipeline(e) => e.to_string(),
                EdgeRef::Added(id) => format!("{id} (added)"),
            };
            format!("{id}{}", if *dir > 0 { "" } else { " (reversed)" })
        })
        .collect();
    ui.label(egui::RichText::new(format!("STG chain: {}", if chain.is_empty() { "none".into() } else { chain.join(", ") })).small())
        .on_hover_text("in order from the video edge's u to v; reversed = the video runs node2 -> node1 of that STG edge");
    match flow.and_then(|f| f.get(&c.key)) {
        None => {
            ui.label(egui::RichText::new("no flow analysed for this edge").weak());
        }
        Some(dirs) => {
            egui::Grid::new("flow_values").num_columns(3).show(ui, |ui| {
                ui.label("");
                for d in dirs {
                    ui.label(egui::RichText::new(if d.dir == c.key { "u to v" } else { "v to u" }).strong());
                }
                ui.end_row();
                for (k, name) in crate::flow::VALUE_NAMES.iter().enumerate() {
                    ui.label(egui::RichText::new(*name).weak());
                    for d in dirs {
                        ui.monospace(format!("{:.3}", d.values[k]));
                    }
                    ui.end_row();
                }
            });
        }
    }
}

/// A video edge of the selected run near `at` (µm), within `radius`.
pub fn video_edge_at(reg: &mut Registration, store: &Store, added: &[AddedSegment], at: [f32; 2], radius: f32) -> Option<[i64; 3]> {
    let p = reg.position?;
    let fits = reg.fits();
    let sim = fits.get(reg.session)?.as_ref()?.sim;
    let m = reg.run_match(p, reg.shown_run(p), &sim, store, added)?;
    let d = |pts: &[[f32; 2]]| pts.windows(2).map(|w| crate::store::segment_dist2(at, w[0], w[1]).sqrt()).fold(f32::INFINITY, f32::min);
    m.placed
        .iter()
        .zip(&m.covers)
        .map(|(pts, c)| (d(pts), c.key))
        .filter(|x| x.0 <= radius)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|x| x.1)
}

/// The selected video edge's chain with `edge` added (in order along the video edge, with
/// its direction from the shared tangent) or removed if it was there.
pub fn toggle_in_chain(reg: &mut Registration, store: &Store, added: &[AddedSegment], edge: EdgeRef) -> Option<Op> {
    let (p, key) = (reg.position?, reg.video_edge?);
    let fits = reg.fits();
    let sim = fits.get(reg.session)?.as_ref()?.sim;
    let run = reg.shown_run(p);
    let m = reg.run_match(p, run, &sim, store, added)?;
    let i = m.covers.iter().position(|c| c.key == key)?;
    let mut chain = m.covers[i].chain.clone();
    let video = &m.placed[i];
    let rel = reg.run_path(p, run);
    if let Some(at) = chain.iter().position(|(e, _)| *e == edge) {
        chain.remove(at);
        return Some(Op::SetAssignment((rel, key), Some(chain)));
    }
    // geometry of the STG edge: its polyline, node1 -> node2
    let stg: Vec<[f32; 2]> = match edge {
        EdgeRef::Pipeline(e) => {
            let arc = store.arcs.points[store.arcs.offsets[e]..store.arcs.offsets[e + 1]].to_vec();
            if store.arc_reversed(e) { arc.into_iter().rev().collect() } else { arc }
        }
        EdgeRef::Added(id) => added.iter().find(|a| a.id == id).map(|a| vec![a.a, a.b])?,
    };
    let mid = stg[stg.len() / 2];
    // where along the video edge it sits, and which way the video runs there
    let along = |q: [f32; 2]| {
        (0..video.len().saturating_sub(1))
            .min_by(|&a, &b| {
                crate::store::segment_dist2(q, video[a], video[a + 1]).total_cmp(&crate::store::segment_dist2(q, video[b], video[b + 1]))
            })
            .unwrap_or(0)
    };
    let k = along(mid);
    let t = [video[k + 1][0] - video[k][0], video[k + 1][1] - video[k][1]];
    let j = (0..stg.len() - 1)
        .min_by(|&a, &b| crate::store::segment_dist2(mid, stg[a], stg[a + 1]).total_cmp(&crate::store::segment_dist2(mid, stg[b], stg[b + 1])))
        .unwrap_or(0);
    let s = [stg[j + 1][0] - stg[j][0], stg[j + 1][1] - stg[j][1]];
    let dir: i8 = if t[0] * s[0] + t[1] * s[1] >= 0.0 { 1 } else { -1 };
    let mid_of = |e: &EdgeRef| -> [f32; 2] {
        match *e {
            EdgeRef::Pipeline(e) => store.arcs.points[(store.arcs.offsets[e] + store.arcs.offsets[e + 1]) / 2],
            EdgeRef::Added(id) => added.iter().find(|a| a.id == id).map_or([0.0; 2], |a| [(a.a[0] + a.b[0]) / 2.0, (a.a[1] + a.b[1]) / 2.0]),
        }
    };
    let at = chain.iter().position(|(e, _)| along(mid_of(e)) > k).unwrap_or(chain.len());
    chain.insert(at, (edge, dir));
    Some(Op::SetAssignment((rel, key), Some(chain)))
}

/// Copy the previous run's corrections onto this run, or clear this run's corrections.
fn run_corrections(ui: &mut egui::Ui, reg: &mut Registration, p: usize, run: usize) {
    let rel = reg.run_path(p, run);
    let mine = reg.layer.nudges.contains_key(&rel)
        || reg.layer.assignments.keys().any(|k| k.0 == rel)
        || reg.layer.disabled.iter().any(|k| k.0 == rel);
    ui.horizontal(|ui| {
        if run > 0 && ui.small_button("Copy corrections from previous run").clicked() {
            let prev = reg.run_path(p, run - 1);
            let keys: HashSet<[i64; 3]> = reg.graph(p, run).map(|g| g.edges.iter().map(|e| e.key).collect()).unwrap_or_default();
            let mut ops = vec![];
            if let Some(n) = reg.layer.nudges.get(&prev) {
                ops.push(Op::SetNudge(rel.clone(), Some(*n)));
            }
            for ((r, k), chain) in &reg.layer.assignments {
                if *r == prev && keys.contains(k) {
                    ops.push(Op::SetAssignment((rel.clone(), *k), Some(chain.clone())));
                }
            }
            for (r, k) in &reg.layer.disabled {
                if *r == prev && keys.contains(k) {
                    ops.push(Op::SetDisabled((rel.clone(), *k), true));
                }
            }
            reg.copied_note = Some((rel.clone(), format!("copied {} corrections from run {}", ops.len(), run)));
            reg.pending.extend(ops);
        }
        if mine && ui.small_button("Clear this run's corrections").clicked() {
            let mut ops = vec![Op::SetNudge(rel.clone(), None)];
            ops.extend(reg.layer.assignments.keys().filter(|k| k.0 == rel).map(|k| Op::SetAssignment(k.clone(), None)));
            ops.extend(reg.layer.disabled.iter().filter(|k| k.0 == rel).map(|k| Op::SetDisabled(k.clone(), false)));
            reg.pending.extend(ops);
        }
    });
    if let Some((_, note)) = reg.copied_note.as_ref().filter(|(r, _)| *r == rel) {
        ui.label(egui::RichText::new(note).weak().small());
    }
}

/// The flow layer of every verified session: per run, each non-artifact video edge's flow
/// mapped onto every STG edge of its chain, with the direction in `/edges` terms. Returns the
/// layer, the number of verified sessions and of runs that contributed.
pub fn flow_output(reg: &mut Registration, store: &Store, added: &[AddedSegment]) -> (crate::annotations::FlowOutput, usize, usize) {
    use crate::annotations::{FlowMeasurementRow, FlowOutput, FlowRunRow};
    use hdf5_metno::types::VarLenUnicode;
    use std::str::FromStr;
    let text = |s: &str| VarLenUnicode::from_str(s).unwrap_or_default();
    let fits = reg.fits();
    let mut out = FlowOutput {
        cutoff_um: reg.threshold(),
        n_anchors: reg.layer.anchors.len() as i64,
        n_pins: reg.layer.assignments.len() as i64,
        ..Default::default()
    };
    let mut sessions = 0;
    for (i, fit) in fits.iter().enumerate() {
        let (Some(fit), Some(key)) = (fit, reg.session_key(i)) else { continue };
        if !reg.layer.verified.contains(&key) {
            continue;
        }
        sessions += 1;
        for p in reg.sessions[i].positions.clone() {
            for run in 0..reg.videos.positions[p].runs.len() {
                let Some(m) = reg.run_match(p, run, &fit.sim, store, added) else { continue };
                let Ok(flow) = &m.flow else { continue };
                let r = &reg.videos.positions[p].runs[run];
                let mut rows = vec![];
                for (c, disabled) in m.covers.iter().zip(&m.disabled) {
                    let Some(dirs) = flow.get(&c.key).filter(|_| !disabled) else { continue };
                    for &(e, dir) in &c.chain {
                        let edge_id = match e {
                            EdgeRef::Pipeline(e) => e as i64,
                            EdgeRef::Added(id) => id,
                        };
                        for d in dirs {
                            let along = d.dir == c.key;
                            let v = d.values;
                            rows.push(FlowMeasurementRow {
                                edge_id,
                                direction: if along { dir } else { -dir },
                                run_index: out.runs.len() as i64,
                                src_u: c.key[0],
                                src_v: c.key[1],
                                src_k: c.key[2],
                                median: v[0],
                                perc_25: v[1],
                                perc_75: v[2],
                                flux_ratio: v[3],
                                coverage: v[4],
                                pixel_count: v[5],
                                flux: v[6],
                            });
                        }
                    }
                }
                if rows.is_empty() {
                    continue;
                }
                let frame = crate::videos::stg_frame(r.utc, &store.timepoints).map_or(r.utc, |f| store.timepoints[f]);
                out.runs.push(FlowRunRow {
                    run_path: text(&r.rel_path),
                    mode: text(&r.mode),
                    run_nr: r.run_nr.unwrap_or(-1),
                    video_epoch_s: r.utc as f64,
                    frame_timestamp: frame as f64,
                });
                out.measurements.extend(rows);
            }
        }
    }
    let runs = out.runs.len();
    (out, sessions, runs)
}
