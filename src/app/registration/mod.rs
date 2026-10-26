//! The Video registration workspace (DESIGN.md §10): the workspace state, matching each run
//! onto the STG with its corrections, and the flow layer it produces. The panels and the map
//! overlay live in `panels` and `map`.

use crate::annotations::{Op, RegistrationLayer};
use crate::video::flow::RunFlow;
use crate::video::matching::{self, AddedSegment, EdgeCover, EdgeRef, Rigid};
use crate::video::placement::{self, Similarity};
use crate::settings::SetupDefault;
use crate::store::Store;
use crate::time::{TimeAxis, TimeUnit};
use crate::video::runs::{Session, VideoGraph, Videos};
use egui::{Color32, Pos2, Stroke};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;

mod map;
mod panels;

pub use map::{overlay, pick, toggle_in_chain, video_edge_at};
pub use panels::{left_panel, right_panel, run_bar, toolbar};

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

/// Stage scatter and anchor colours.
const MAPPED: Color32 = Color32::from_rgb(29, 158, 117);
const ANCHORED: Color32 = Color32::from_rgb(55, 138, 221);
const PLACED: Color32 = Color32::from_rgb(210, 210, 205);
const UNPLACED: Color32 = Color32::from_rgb(136, 135, 128);

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
    /// The setup default's video scale (STG µm per video µm), used until the layer sets one.
    pub default_video_scale: f32,
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
    /// Read flow values by run path.
    flows: HashMap<String, Result<RunFlow, String>>,
    /// Fit score per session, with the (transform, camera) it was computed for.
    scores: HashMap<usize, (String, Option<f32>)>,
    /// The selected video edge of the selected run.
    pub video_edge: Option<[i64; 3]>,
    /// Matched runs by run path, with the inputs they were matched with.
    matches: HashMap<String, (String, Rc<RunMatch>)>,
    /// Stage scatter zoom and pan (points), for the session it was set in.
    scatter_view: (usize, f32, egui::Vec2),
    /// Position -> the run last selected there, kept on the map once the position is
    /// deselected so its (per-run) nudge does not appear to jump back.
    last_run: HashMap<usize, usize>,
}

impl Registration {
    pub fn new(videos: Videos, gap_minutes: f32, store: &Store, default: Option<SetupDefault>) -> Self {
        let sessions = crate::video::runs::sessions(&videos.positions, gap_minutes);
        let mapped_in_store = store.flow_in_store.iter().flat_map(|f| f.run_paths.iter().cloned()).collect();
        let trust_default_offset = videos.setup == crate::video::runs::Setup::Aretha;
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
            default_video_scale: default.map_or(1.0, |d| d.video_scale),
            drag_nudge: None,
            copied_note: None,
            default: default.map(|d| d.transform),
            trust_default_offset,
            graphs: HashMap::new(),
            flows: HashMap::new(),
            scores: HashMap::new(),
            video_edge: None,
            matches: HashMap::new(),
            scatter_view: (0, 1.0, egui::Vec2::ZERO),
            last_run: HashMap::new(),
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

    /// STG µm per video µm (DESIGN.md §10): the registration's own value, else the setup's.
    pub fn video_scale(&self) -> f32 {
        self.layer.video_scale.unwrap_or(self.default_video_scale)
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
            "{sim:?}/{}/{}/{}/{}/{}/{nudge:?}/{assignments:?}/{disabled:?}",
            self.camera(),
            self.video_scale(),
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
        let video_scale = self.video_scale();
        let raw: Vec<Vec<[f32; 2]>> = graph
            .edges
            .iter()
            .map(|e| {
                e.points
                    .iter()
                    .map(|&q| {
                        let w = placement::place(sim, self.camera(), video_scale, stage, q);
                        [w[0] + nudge[0], w[1] + nudge[1]]
                    })
                    .collect()
            })
            .collect();
        let snap = if self.snap() {
            let samples: Vec<[f32; 2]> = raw.iter().flat_map(|e| e.iter().step_by(5).copied()).collect();
            let reach = if nudge == [0.0; 2] { matching::SNAP_WIDE } else { matching::SNAP_LOCAL };
            matching::snap(store, &samples, reach)
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
                // keep the matcher's span where it found the same edge; unknown elsewhere
                c.spans = chain
                    .iter()
                    .map(|(e, _)| c.chain.iter().position(|(f, _)| f == e).map_or([f32::NAN; 2], |i| c.spans[i]))
                    .collect();
                c.chain = chain.clone();
                *a = true;
            }
        }
        let disabled = covers.iter().map(|c| self.layer.disabled.contains(&(rel.clone(), c.key))).collect();
        let root = &self.videos.root;
        let flow = self.flows.entry(rel.clone()).or_insert_with(|| crate::video::flow::read(&root.join(&rel))).clone();
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
    /// run last selected there, else the run closest in time to the selected one (the latest
    /// run when nothing is selected).
    pub fn shown_run(&self, p: usize) -> usize {
        let runs = &self.videos.positions[p].runs;
        if self.position == Some(p) {
            return self.run.min(runs.len() - 1);
        }
        if let Some(&run) = self.last_run.get(&p) {
            return run.min(runs.len() - 1);
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
            .or_insert_with(|| crate::video::runs::load_graph(root, r).ok().map(Rc::new))
            .clone()
    }

    /// Sampled video-graph points of the session's positions (shown runs), placed by `sim`
    /// with camera option `camera`.
    fn placed_samples(&mut self, i: usize, sim: &Similarity, camera: u8) -> Vec<[f32; 2]> {
        let positions = self.sessions[i].positions.clone();
        let mut out = vec![];
        let video_scale = self.video_scale();
        for p in positions {
            let stage = self.videos.positions[p].stage_mm;
            if let Some(g) = self.graph(p, self.shown_run(p)) {
                for e in &g.edges {
                    out.extend(e.points.iter().step_by(5).map(|&q| placement::place(sim, camera, video_scale, stage, q)));
                }
            }
        }
        out
    }

    /// Fit score of session `i` (mean capped distance of its placed videos to the STG),
    /// recomputed only when its transform or the camera changed.
    pub fn score(&mut self, i: usize, fit: &SessionFit, store: &Store) -> Option<f32> {
        let key = format!("{:?}/{}/{}/{:?}", fit.sim, self.camera(), self.video_scale(), self.selected_run_utc());
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
        self.sessions = crate::video::runs::sessions(&self.videos.positions, gap_minutes);
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
    /// A run was selected from a list: fit the view to its frame.
    ZoomRun,
    None,
}

/// The flow layer of every verified session: per run, each non-artifact video edge's flow
/// mapped onto every STG edge of its chain, with the direction in `/edges` terms. Returns the
/// layer, the number of verified sessions and of runs that contributed, and the verified
/// sessions' runs that contributed nothing (`(position, run)`).
pub fn flow_output(reg: &mut Registration, store: &Store, added: &[AddedSegment]) -> (crate::annotations::FlowOutput, usize, usize, Vec<(usize, usize)>) {
    use crate::annotations::{FlowOutput, FlowRunRow};
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
    let (mut sessions, mut missing) = (0, vec![]);
    for (i, fit) in fits.iter().enumerate() {
        let (Some(fit), Some(key)) = (fit, reg.session_key(i)) else { continue };
        if !reg.layer.verified.contains(&key) {
            continue;
        }
        sessions += 1;
        for p in reg.sessions[i].positions.clone() {
            for run in 0..reg.videos.positions[p].runs.len() {
                let mut rows = reg.run_rows(p, run, &fit.sim, store, added);
                if rows.is_empty() {
                    missing.push((p, run));
                    continue;
                }
                rows.iter_mut().for_each(|r| r.run_index = out.runs.len() as i64);
                let r = &reg.videos.positions[p].runs[run];
                let frame = crate::video::runs::stg_frame(r.utc, &store.timepoints).map_or(r.utc, |f| store.timepoints[f]);
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
    (out, sessions, runs, missing)
}

impl Registration {
    /// Run `run` of position `p`'s flow rows under `sim` (`run_index` 0): each non-artifact
    /// video edge's flow on every STG edge of its chain. Empty: the run gives no flow (no
    /// graph, no flow file, or no video edge on the STG).
    pub fn run_rows(&mut self, p: usize, run: usize, sim: &Similarity, store: &Store, added: &[AddedSegment]) -> Vec<crate::annotations::FlowMeasurementRow> {
        let Some(m) = self.run_match(p, run, sim, store, added) else { return vec![] };
        let Ok(flow) = &m.flow else { return vec![] };
        let mut rows = vec![];
        for (c, disabled) in m.covers.iter().zip(&m.disabled) {
            let Some(dirs) = flow.get(&c.key).filter(|_| !disabled) else { continue };
            for (&(e, dir), span) in c.chain.iter().zip(&c.spans) {
                let edge_id = match e {
                    EdgeRef::Pipeline(e) => e as i64,
                    EdgeRef::Added(id) => id,
                };
                for d in dirs {
                    let along = d.dir == c.key;
                    let v = d.values;
                    rows.push(crate::annotations::FlowMeasurementRow {
                        edge_id,
                        direction: if along { dir } else { -dir },
                        run_index: 0,
                        // the directed video key, as the registrator wrote it: readers such as
                        // transport_paper's apply_streaks take the streak's sign from it
                        src_u: d.dir[0],
                        src_v: d.dir[1],
                        src_k: d.dir[2],
                        median: v[0],
                        perc_25: v[1],
                        perc_75: v[2],
                        flux_ratio: v[3],
                        coverage: v[4],
                        pixel_count: v[5],
                        flux: v[6],
                        s_start_um: span[0] as f64,
                        s_end_um: span[1] as f64,
                    });
                }
            }
        }
        rows
    }

    /// Run `run` of position `p` in the selected session's placement: how many of its video
    /// edges have an STG chain (artifacts left out). `None`: not placed, or no video graph.
    pub fn chained_edges(&mut self, p: usize, run: usize, store: &Store, added: &[AddedSegment]) -> Option<usize> {
        let sim = self.fits().get(self.session)?.as_ref()?.sim;
        let m = self.run_match(p, run, &sim, store, added)?;
        Some(m.covers.iter().zip(&m.disabled).filter(|&(c, d)| !d && !c.chain.is_empty()).count())
    }

    /// The selected session's runs that give no flow in its current placement (`(position,
    /// run)`): what Publish would leave out. Empty when the session is not placed.
    pub fn runs_without_flow(&mut self, store: &Store, added: &[AddedSegment]) -> Vec<(usize, usize)> {
        let Some(Some(fit)) = self.fits().get(self.session).map(|f| f.as_ref().map(|f| f.sim)) else { return vec![] };
        let mut out = vec![];
        for p in self.sessions[self.session].positions.clone() {
            for run in 0..self.videos.positions[p].runs.len() {
                if self.run_rows(p, run, &fit, store, added).is_empty() {
                    out.push((p, run));
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn run_number_labels() {
        let run = |nr| crate::video::runs::Run {
            rel_path: String::new(),
            utc: 0,
            mode: String::new(),
            run_nr: nr,
            delta_x_um: 1.0,
            frame_size: [1, 1],
            camera: String::new(),
            stage_mm: None,
        };
        assert_eq!(super::panels::run_numbers(&[run(Some(24)), run(Some(25))]).as_deref(), Some("24-25"));
        assert_eq!(super::panels::run_numbers(&[run(Some(3)), run(None), run(Some(7))]).as_deref(), Some("3, 7"));
        assert_eq!(super::panels::run_numbers(&[run(Some(9))]).as_deref(), Some("9"));
        assert_eq!(super::panels::run_numbers(&[run(None)]), None);
    }

    /// A verified session's run whose video edge lands off the STG gives no rows and is
    /// reported as missing, beside a run that does give rows.
    #[test]
    fn flow_output_reports_runs_without_flow() {
        use crate::store::{Arcs, Store};
        use crate::video::flow::DirectionFlow;
        use crate::video::runs::{Position, Run, Setup, VideoEdge, VideoGraph, Videos};
        let mut store = Store {
            arcs: Arcs { offsets: vec![0, 2], points: vec![[0.0, 0.0], [200.0, 0.0]], min: [0.0, 0.0], max: [200.0, 0.0] },
            node_ids: vec![1, 2],
            node_pos: vec![[0.0, 0.0], [200.0, 0.0]],
            edge_nodes: vec![[1, 2]],
            ..Default::default()
        };
        store.node_index = store.node_ids.iter().enumerate().map(|(i, &id)| (id, i)).collect();
        let run = |rel: &str, utc| Run {
            rel_path: rel.into(),
            utc,
            mode: "brightfield".into(),
            run_nr: None,
            delta_x_um: 1.0,
            frame_size: [100, 100],
            camera: String::new(),
            stage_mm: None,
        };
        let videos = Videos {
            root: "/nonexistent".into(),
            positions: vec![Position { name: "p".into(), stage_mm: [0.0, 0.0], runs: vec![run("p/on", 0), run("p/off", 60)] }],
            setup: Setup::Morrison,
            skipped: vec![],
        };
        let mut reg = super::Registration::new(videos, 30.0, &store, None);
        let mut layer = crate::annotations::RegistrationLayer { snap: Some(false), ..Default::default() };
        layer.anchors.insert("p".into(), [100.0, 0.0]);
        layer.verified.insert("p".into());
        reg.sync(&layer);
        // "on" runs along the arc; "off" sits 40 µm beside it, beyond the 10 µm threshold
        for (rel, y) in [("p/on", 0.0), ("p/off", 40.0)] {
            let edge = VideoEdge { key: [0, 1, 0], points: vec![[-40.0, y], [40.0, y]] };
            reg.graphs.insert(rel.into(), Some(std::rc::Rc::new(VideoGraph { edges: vec![edge] })));
            let flow = DirectionFlow { dir: [0, 1, 0], values: [1.0; 7] };
            reg.flows.insert(rel.into(), Ok([([0, 1, 0], vec![flow])].into()));
        }
        let (out, sessions, runs, missing) = super::flow_output(&mut reg, &store, &[]);
        assert_eq!((sessions, runs), (1, 1));
        assert_eq!(out.runs[0].run_path.as_str(), "p/on");
        assert!(!out.measurements.is_empty() && out.measurements.iter().all(|m| m.run_index == 0));
        assert_eq!(missing, vec![(0, 1)]);
        // the panel's live view agrees
        assert_eq!(reg.runs_without_flow(&store, &[]), vec![(0, 1)]);
        assert_eq!((reg.chained_edges(0, 0, &store, &[]), reg.chained_edges(0, 1, &store, &[])), (Some(1), Some(0)));
    }
}
