//! Per-author annotation files, `annotations.<author>.h5` beside the store (DESIGN.md §3).
//! Annotations never touch the store: they are keyed on its `edge_id` / node ids, and
//! record the `backbone_fingerprint` they were made against.

use crate::video::matching::EdgeRef;
use hdf5_metno::types::VarLenUnicode;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::str::FromStr;

mod file;
mod publish;

pub use file::{load, save};
pub use publish::{Published, publish};

pub const SCHEMA_VERSION: i64 = 1;
/// Added edges get ids from here up, clear of any pipeline `edge_id`.
pub const ADDED_EDGE_ID_START: i64 = 1_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Target {
    Edge(usize),
    /// Pipeline node id
    Node(i64),
}

/// A user-drawn edge between two existing nodes (straight; no time or width data).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AddedEdge {
    pub id: i64,
    pub u: i64,
    pub v: i64,
}

/// A video edge of one run: (run path relative to `polyflow_analysis`, video `(u, v, k)`).
pub type RunEdge = (String, [i64; 3]);

/// The author's video registration (DESIGN.md §10): everything needed to reopen and redo it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RegistrationLayer {
    /// Position name -> where its video centre sits on the STG (µm).
    pub anchors: BTreeMap<String, [f64; 2]>,
    /// Verified sessions, by their first position's name.
    pub verified: BTreeSet<String>,
    /// Run path -> extra shift (µm) applied before the snap.
    pub nudges: BTreeMap<String, [f32; 2]>,
    /// A video edge's STG chain set by hand (replaces the automatic one).
    pub assignments: BTreeMap<RunEdge, Vec<(EdgeRef, i8)>>,
    /// Video edges marked as artifacts: no flow from them.
    pub disabled: BTreeSet<RunEdge>,
    /// Camera orientation, threshold (µm) and snap switch; `None`: the app's defaults.
    pub camera: Option<u8>,
    pub threshold_um: Option<f32>,
    pub snap: Option<bool>,
    /// STG µm per video µm, the setup's network-scale calibration this registration used;
    /// `None` (files from before it existed): the setup default, else 1.
    pub video_scale: Option<f32>,
}

impl RegistrationLayer {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// The flow layer in STHype's `/flow_runs` / `/flow_measurements` compound schema
/// (FLOW_SCHEMA_VERSION 1); `edge_id` may be an added edge's id.
#[derive(Clone, Default)]
pub struct FlowOutput {
    pub runs: Vec<FlowRunRow>,
    pub measurements: Vec<FlowMeasurementRow>,
    pub cutoff_um: f32,
    pub n_anchors: i64,
    pub n_pins: i64,
    /// STG µm per video µm the runs were placed with (`/flow_measurements` attr
    /// `video_scale`); NaN when read from a layer written before it was recorded.
    pub video_scale: f32,
}

#[derive(hdf5_metno::H5Type, Clone, Debug)]
#[repr(C)]
pub struct FlowRunRow {
    pub run_path: VarLenUnicode,
    pub mode: VarLenUnicode,
    pub run_nr: i64,
    pub video_epoch_s: f64,
    pub frame_timestamp: f64,
}

#[derive(hdf5_metno::H5Type, Clone, Copy, Debug)]
#[repr(C)]
pub struct FlowMeasurementRow {
    pub edge_id: i64,
    /// +1 along `/edges` (node1, node2), -1 against.
    pub direction: i8,
    pub run_index: i64,
    pub src_u: i64,
    pub src_v: i64,
    pub src_k: i64,
    pub median: f64,
    pub perc_25: f64,
    pub perc_75: f64,
    pub flux_ratio: f64,
    pub coverage: f64,
    pub pixel_count: f64,
    pub flux: f64,
    /// The stretch of the STG edge's arc the video edge lies on, in µm along `/arcs_points`
    /// of that edge (the `/arc_time_runs` axis); NaN when unknown. Additive to STHype's
    /// FLOW_SCHEMA_VERSION 1 fields.
    pub s_start_um: f64,
    pub s_end_um: f64,
}

/// One reversible change.
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    SetExcluded(usize, bool),
    /// `(edge, keeper)`: `edge` duplicates `keeper`; consumers drop `edge`.
    SetDuplicateOf(usize, Option<usize>),
    SetHyperedge(usize, Option<i64>),
    SetRoot(i64, bool),
    AddEdge(AddedEdge),
    RemoveAddedEdge(AddedEdge),
    SetNote(Target, Option<String>),
    /// A named trail's waypoints (node ids); `None` deletes the trail.
    SetTrail(String, Option<Vec<i64>>),
    /// Registration (video positions and their runs):
    SetAnchor(String, Option<[f64; 2]>),
    SetVerified(String, bool),
    SetNudge(String, Option<[f32; 2]>),
    SetAssignment(RunEdge, Option<Vec<(EdgeRef, i8)>>),
    SetDisabled(RunEdge, bool),
    /// Camera orientation, threshold (µm), snap.
    SetRegistrationParams(Option<u8>, Option<f32>, Option<bool>),
    /// STG µm per video µm.
    SetVideoScale(Option<f32>),
}

impl Op {
    /// Stable text for `/history`.
    fn describe(&self) -> String {
        match self {
            Op::SetExcluded(e, v) => format!("set_excluded edge={e} value={v}"),
            Op::SetDuplicateOf(e, k) => format!("set_duplicate_of edge={e} of={}", opt(k)),
            Op::SetHyperedge(e, h) => format!("set_hyperedge edge={e} hyperedge={}", opt(h)),
            Op::SetRoot(n, v) => format!("set_root node={n} value={v}"),
            Op::AddEdge(a) => format!("add_edge id={} u={} v={}", a.id, a.u, a.v),
            Op::RemoveAddedEdge(a) => format!("remove_added_edge id={}", a.id),
            Op::SetNote(t, n) => format!("set_note {} text={:?}", target_name(*t), n.as_deref().unwrap_or("")),
            Op::SetTrail(name, w) => match w {
                Some(w) => format!("set_trail name={name:?} waypoints={w:?}"),
                None => format!("delete_trail name={name:?}"),
            },
            Op::SetAnchor(p, a) => format!("set_anchor position={p:?} at={a:?}"),
            Op::SetVerified(s, v) => format!("set_verified session={s:?} value={v}"),
            Op::SetNudge(r, n) => format!("set_nudge run={r:?} shift={n:?}"),
            Op::SetAssignment((r, k), c) => format!("set_assignment run={r:?} video_edge={k:?} chain={c:?}"),
            Op::SetDisabled((r, k), v) => format!("set_disabled run={r:?} video_edge={k:?} value={v}"),
            Op::SetRegistrationParams(c, t, s) => format!("set_registration camera={c:?} threshold_um={t:?} snap={s:?}"),
            Op::SetVideoScale(v) => format!("set_video_scale value={v:?}"),
        }
    }
}

fn opt<T: ToString>(v: &Option<T>) -> String {
    v.as_ref().map_or("none".into(), T::to_string)
}

fn target_name(t: Target) -> String {
    match t {
        Target::Edge(e) => format!("edge={e}"),
        Target::Node(n) => format!("node={n}"),
    }
}

pub struct HistoryRow {
    pub utc: i64,
    pub author: String,
    pub op: String,
}

#[derive(Default)]
pub struct Annotations {
    pub author: String,
    pub fingerprint: String,
    pub excluded: BTreeSet<usize>,
    pub duplicate_of: BTreeMap<usize, usize>,
    pub hyperedge: BTreeMap<usize, i64>,
    pub roots: BTreeSet<i64>,
    pub added: Vec<AddedEdge>,
    pub notes: BTreeMap<Target, String>,
    /// Trail name -> waypoints (node ids); the routed path is derived from the STG.
    pub trails: BTreeMap<String, Vec<i64>>,
    pub registration: RegistrationLayer,
    /// The flow layer: computed from the registration when it is saved, else as last loaded.
    pub flow: Option<FlowOutput>,
    pub history: Vec<HistoryRow>,
    undo: Vec<Op>,
    redo: Vec<Op>,
    /// Unsaved changes since the last load or save.
    pub dirty: bool,
}

impl Annotations {
    pub fn new(author: &str, fingerprint: &str) -> Self {
        Self {
            author: author.into(),
            fingerprint: fingerprint.into(),
            ..Default::default()
        }
    }

    pub fn next_added_id(&self) -> i64 {
        self.added.iter().map(|a| a.id + 1).max().unwrap_or(ADDED_EDGE_ID_START)
    }

    /// Applies `op`, returning its inverse. Does not touch undo/redo or history.
    fn apply_raw(&mut self, op: Op) -> Op {
        fn set<K: Ord + Copy, V>(map: &mut BTreeMap<K, V>, k: K, v: Option<V>) -> Option<V> {
            match v {
                Some(v) => map.insert(k, v),
                None => map.remove(&k),
            }
        }
        match op {
            Op::SetExcluded(e, v) => {
                let was = self.excluded.contains(&e);
                if v { self.excluded.insert(e) } else { self.excluded.remove(&e) };
                Op::SetExcluded(e, was)
            }
            Op::SetDuplicateOf(e, k) => Op::SetDuplicateOf(e, set(&mut self.duplicate_of, e, k)),
            Op::SetHyperedge(e, h) => Op::SetHyperedge(e, set(&mut self.hyperedge, e, h)),
            Op::SetRoot(n, v) => {
                let was = self.roots.contains(&n);
                if v { self.roots.insert(n) } else { self.roots.remove(&n) };
                Op::SetRoot(n, was)
            }
            Op::AddEdge(a) => {
                self.added.push(a);
                Op::RemoveAddedEdge(a)
            }
            Op::RemoveAddedEdge(a) => {
                self.added.retain(|x| x.id != a.id);
                Op::AddEdge(a)
            }
            Op::SetNote(t, n) => Op::SetNote(t, set(&mut self.notes, t, n)),
            Op::SetTrail(name, w) => {
                let was = match w {
                    Some(w) => self.trails.insert(name.clone(), w),
                    None => self.trails.remove(&name),
                };
                Op::SetTrail(name, was)
            }
            Op::SetAnchor(p, a) => {
                let r = &mut self.registration.anchors;
                let was = match a {
                    Some(a) => r.insert(p.clone(), a),
                    None => r.remove(&p),
                };
                Op::SetAnchor(p, was)
            }
            Op::SetVerified(s, v) => {
                let was = self.registration.verified.contains(&s);
                if v { self.registration.verified.insert(s.clone()) } else { self.registration.verified.remove(&s) };
                Op::SetVerified(s, was)
            }
            Op::SetNudge(r, n) => {
                let m = &mut self.registration.nudges;
                let was = match n {
                    Some(n) => m.insert(r.clone(), n),
                    None => m.remove(&r),
                };
                Op::SetNudge(r, was)
            }
            Op::SetAssignment(k, c) => {
                let m = &mut self.registration.assignments;
                let was = match c {
                    Some(c) => m.insert(k.clone(), c),
                    None => m.remove(&k),
                };
                Op::SetAssignment(k, was)
            }
            Op::SetDisabled(k, v) => {
                let was = self.registration.disabled.contains(&k);
                if v { self.registration.disabled.insert(k.clone()) } else { self.registration.disabled.remove(&k) };
                Op::SetDisabled(k, was)
            }
            Op::SetRegistrationParams(c, t, s) => {
                let r = &mut self.registration;
                let was = Op::SetRegistrationParams(r.camera, r.threshold_um, r.snap);
                (r.camera, r.threshold_um, r.snap) = (c, t, s);
                was
            }
            Op::SetVideoScale(v) => Op::SetVideoScale(std::mem::replace(&mut self.registration.video_scale, v)),
        }
    }

    fn record(&mut self, op: &Op) {
        self.history.push(HistoryRow {
            utc: chrono::Utc::now().timestamp(),
            author: self.author.clone(),
            op: op.describe(),
        });
        self.dirty = true;
    }

    /// Applies a user edit; undoable. A no-op edit is dropped.
    pub fn edit(&mut self, op: Op) {
        let inverse = self.apply_raw(op.clone());
        if inverse == op {
            return;
        }
        self.record(&op);
        self.undo.push(inverse);
        self.redo.clear();
    }

    pub fn undo(&mut self) -> bool {
        self.step(true)
    }

    pub fn redo(&mut self) -> bool {
        self.step(false)
    }

    fn step(&mut self, undo: bool) -> bool {
        let Some(op) = (if undo { self.undo.pop() } else { self.redo.pop() }) else {
            return false;
        };
        self.record(&op);
        let inverse = self.apply_raw(op);
        if undo { self.redo.push(inverse) } else { self.undo.push(inverse) };
        true
    }

    pub fn is_empty(&self) -> bool {
        self.excluded.is_empty()
            && self.duplicate_of.is_empty()
            && self.hyperedge.is_empty()
            && self.roots.is_empty()
            && self.added.is_empty()
            && self.notes.is_empty()
            && self.trails.is_empty()
            && self.registration.is_empty()
    }
}

/// Keeps author names safe as part of a file name.
pub fn sanitize_author(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

pub fn path_for(store: &Path, author: &str) -> PathBuf {
    store.with_file_name(format!("annotations.{author}.h5"))
}

/// Every `annotations.<author>.h5` beside `store`, as `(author, path)`.
pub fn list(store: &Path) -> Vec<(String, PathBuf)> {
    let Some(dir) = store.parent() else {
        return vec![];
    };
    let mut out: Vec<(String, PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| {
            let path = e.ok()?.path();
            let name = path.file_name()?.to_str()?;
            let author = name.strip_prefix("annotations.")?.strip_suffix(".h5")?;
            Some((author.to_string(), path))
        })
        .collect();
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use file::vlu;

    #[test]
    fn publish_writes_flow_trails_roots_and_refuses_another_backbone() {
        let dir = std::env::temp_dir().join(format!("gramf_publish_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = dir.join("stg_store.h5");
        let f = hdf5_metno::File::create(&store).unwrap();
        f.new_attr::<VarLenUnicode>().shape(()).create("backbone_fingerprint").unwrap().as_writer().write_scalar(&vlu("fp")).unwrap();
        f.new_dataset_builder().with_data(&[7i64]).create("root_nodes").unwrap(); // replaced
        drop(f);

        let mut a = Annotations::new("simon", "fp");
        a.edit(Op::SetRoot(3, true));
        a.edit(Op::SetTrail("main".into(), Some(vec![1, 2])));
        let m = |edge_id| FlowMeasurementRow {
            edge_id, direction: 1, run_index: 0, src_u: 0, src_v: 1, src_k: 0,
            median: 1.0, perc_25: 0.5, perc_75: 1.5, flux_ratio: f64::NAN, coverage: 1.0, pixel_count: 9.0, flux: 2.0,
            s_start_um: 3.0, s_end_um: 40.0,
        };
        let run = FlowRunRow { run_path: vlu("pos/run"), mode: vlu("brightfield"), run_nr: -1, video_epoch_s: 1.0, frame_timestamp: 2.0 };
        a.flow = Some(FlowOutput { runs: vec![run], measurements: vec![m(4), m(ADDED_EDGE_ID_START)], cutoff_um: 8.0, n_anchors: 2, n_pins: 0, video_scale: 1.1 });
        let paths = BTreeMap::from([("main".to_string(), vec![1, 5, 2])]);

        let p = publish::publish_with_backup(&a, &store, 10, &paths, "bak1").unwrap();
        let backup = dir.join("stg_store.h5.bak1");
        assert_eq!(p, Published { measurements: 1, on_added_edges: 1, trails: 1, roots: 1, backup: backup.clone() });
        // the backup is the store as it was
        let b = hdf5_metno::File::open(&backup).unwrap();
        assert_eq!(b.dataset("root_nodes").unwrap().read_raw::<i64>().unwrap(), [7]);
        assert!(!b.link_exists("flow_runs"));
        drop(b);
        let f = hdf5_metno::File::open(&store).unwrap();
        assert_eq!(f.dataset("root_nodes").unwrap().read_raw::<i64>().unwrap(), [3]);
        assert_eq!(f.dataset("flow_trails_nodes").unwrap().read_raw::<i64>().unwrap(), [1, 5, 2]);
        assert_eq!(f.dataset("flow_measurements").unwrap().read_raw::<FlowMeasurementRow>().unwrap()[0].edge_id, 4);
        let scale = f.dataset("flow_measurements").unwrap().attr("video_scale").unwrap().read_scalar::<f64>().unwrap();
        assert!((scale - 1.1).abs() < 1e-6, "{scale}");
        drop(f);

        a.fingerprint = "changed".into();
        assert!(publish::publish_with_backup(&a, &store, 10, &paths, "bak2").is_err());
        assert!(!store.with_extension("h5.publishing").exists());
        assert!(!dir.join("stg_store.h5.bak2").exists(), "a refused publish leaves no backup");
        assert!(store.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn undo_redo_and_noop() {
        let mut a = Annotations::new("simon", "fp");
        a.edit(Op::SetExcluded(5, true));
        a.edit(Op::SetExcluded(5, true)); // no-op: not recorded
        a.edit(Op::SetNote(Target::Node(7), Some("tip".into())));
        assert_eq!(a.history.len(), 2);
        assert!(a.undo() && a.undo() && !a.undo());
        assert!(a.is_empty());
        assert!(a.redo());
        assert!(a.excluded.contains(&5));
        a.edit(Op::SetRoot(3, true)); // a new edit clears redo
        assert!(!a.redo());
    }

    #[test]
    fn save_load_round_trip() {
        let mut a = Annotations::new("simon", "abc123");
        a.edit(Op::SetExcluded(812, true));
        a.edit(Op::SetDuplicateOf(10, Some(11)));
        a.edit(Op::SetHyperedge(4, Some(41)));
        a.edit(Op::SetRoot(9188, true));
        a.edit(Op::AddEdge(AddedEdge { id: a.next_added_id(), u: 1, v: 2 }));
        a.edit(Op::SetNote(Target::Edge(812), Some("false bridge".into())));
        a.edit(Op::SetTrail("main".into(), Some(vec![1, 2, 9188])));
        a.edit(Op::SetAnchor("x_n10_38_y_4_484".into(), Some([31256.0, 37779.5])));
        a.edit(Op::SetVerified("x_n10_886_y_3_999".into(), true));
        a.edit(Op::SetNudge("pos/20260609_150109".into(), Some([3.0, -2.5])));
        let run_edge = ("pos/20260609_150109".to_string(), [1, 6, 0]);
        a.edit(Op::SetAssignment(run_edge.clone(), Some(vec![(EdgeRef::Pipeline(23158), 1), (EdgeRef::Added(1_000_000), -1)])));
        a.edit(Op::SetDisabled(("pos/20260609_150109".to_string(), [2, 1, 0]), true));
        a.edit(Op::SetRegistrationParams(Some(3), Some(12.5), Some(false)));
        a.edit(Op::SetVideoScale(Some(1.1458)));
        let row = |s: &str| VarLenUnicode::from_str(s).unwrap();
        a.flow = Some(FlowOutput {
            runs: vec![FlowRunRow { run_path: row("pos/20260609_150109"), mode: row("brightfield"), run_nr: -1, video_epoch_s: 1.0, frame_timestamp: 2.0 }],
            measurements: vec![FlowMeasurementRow {
                edge_id: 23158, direction: -1, run_index: 0, src_u: 1, src_v: 6, src_k: 0,
                median: 1.5, perc_25: 0.5, perc_75: 3.0, flux_ratio: 0.5, coverage: 0.4, pixel_count: 100.0, flux: 150.0,
                s_start_um: 12.5, s_end_um: 140.0,
            }],
            cutoff_um: 12.5,
            video_scale: 1.1458,
            n_anchors: 1,
            n_pins: 1,
        });

        let dir = std::env::temp_dir().join(format!("gramf-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("annotations.simon.h5");
        save(&a, &path, &BTreeMap::new()).unwrap();
        let b = load(&path).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();

        assert_eq!((b.author.as_str(), b.fingerprint.as_str()), ("simon", "abc123"));
        assert_eq!(b.excluded, a.excluded);
        assert_eq!(b.duplicate_of, a.duplicate_of);
        assert_eq!(b.hyperedge, a.hyperedge);
        assert_eq!(b.roots, a.roots);
        assert_eq!(b.added, [AddedEdge { id: ADDED_EDGE_ID_START, u: 1, v: 2 }]);
        assert_eq!(b.notes, a.notes);
        assert_eq!(b.trails, a.trails);
        assert_eq!(b.registration, a.registration);
        let flow = b.flow.as_ref().unwrap();
        assert_eq!((flow.runs.len(), flow.measurements[0].edge_id, flow.measurements[0].direction), (1, 23158, -1));
        assert_eq!((flow.measurements[0].s_start_um, flow.measurements[0].s_end_um), (12.5, 140.0));
        assert_eq!(flow.video_scale, 1.1458);
        assert_eq!(b.registration.video_scale, Some(1.1458));
        assert_eq!(b.history.len(), 14);
        assert!(!b.dirty);
    }

    #[test]
    fn empty_file_round_trip_and_names() {
        let dir = std::env::temp_dir().join(format!("gramf-test-empty-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = dir.join("stg_store.h5");
        let path = path_for(&store, "anna");
        save(&Annotations::new("anna", ""), &path, &BTreeMap::new()).unwrap();
        assert!(load(&path).unwrap().is_empty());
        assert_eq!(list(&store), [("anna".to_string(), path)]);
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(sanitize_author("Anna M./x"), "AnnaMx");
    }
}
