//! Per-author annotation files, `annotations.<author>.h5` beside the store (DESIGN.md §3).
//! Annotations never touch the store: they are keyed on its `edge_id` / node ids, and
//! record the `backbone_fingerprint` they were made against.

use crate::matching::EdgeRef;
use hdf5_metno::types::VarLenUnicode;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::str::FromStr;

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

type H5 = hdf5_metno::Result<()>;

fn vlu(s: &str) -> VarLenUnicode {
    // ponytail: interior NULs are the only failure; strip them rather than error
    VarLenUnicode::from_str(&s.replace('\0', "")).unwrap_or_default()
}

/// Writes `a` to `path` atomically: a `.tmp` sibling, then a rename over the target, so a
/// crash or a Dropbox sync never sees a half-written HDF5 file. `trail_paths` holds each
/// trail's routed node path, written for readers that cannot route themselves.
pub fn save(a: &Annotations, path: &Path, trail_paths: &BTreeMap<String, Vec<i64>>) -> Result<(), String> {
    let tmp = path.with_extension("h5.tmp");
    write_file(a, &tmp, trail_paths).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// What Publish to store wrote, for the status line.
#[derive(Debug, PartialEq)]
pub struct Published {
    pub measurements: usize,
    /// Measurements on added edges: the store has no `edge_id` for them, so they stay in
    /// the annotation file only.
    pub on_added_edges: usize,
    pub trails: usize,
    pub roots: usize,
}

/// Publish to store (DESIGN.md §10, a temporary bridge): writes the flow layer, trails and
/// roots into `store` in STHype's format (`write_flow_mappings`, `write_root_nodes`),
/// replacing what is there. The one place GrAMF writes a store, so it refuses a store whose
/// backbone changed, and works on a copy renamed over the original only when complete.
pub fn publish(a: &Annotations, store: &Path, n_edges: usize, trail_paths: &BTreeMap<String, Vec<i64>>) -> Result<Published, String> {
    let flow = a.flow.as_ref().ok_or("no flow layer: verify a session and save first")?;
    let tmp = store.with_extension("h5.publishing");
    std::fs::copy(store, &tmp).map_err(|e| e.to_string())?;
    let result = publish_into(a, flow, &tmp, n_edges, trail_paths).map_err(|e| e.to_string());
    match result {
        Ok(Ok(p)) => std::fs::rename(&tmp, store).map(|_| p).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            e.to_string()
        }),
        Ok(Err(e)) | Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

fn publish_into(a: &Annotations, flow: &FlowOutput, path: &Path, n_edges: usize, trail_paths: &BTreeMap<String, Vec<i64>>) -> hdf5_metno::Result<Result<Published, String>> {
    let f = hdf5_metno::File::append(path)?;
    let fingerprint = f.attr("backbone_fingerprint")?.read_scalar::<VarLenUnicode>()?.to_string();
    if fingerprint != a.fingerprint {
        return Ok(Err("the store's backbone changed since these annotations were made".into()));
    }
    for name in ["flow_runs", "flow_measurements", "flow_trails_names", "flow_trails_nodes", "flow_trails_offsets", "root_nodes"] {
        if f.link_exists(name) {
            f.unlink(name)?;
        }
    }
    let attr_i = |d: &hdf5_metno::Dataset, name: &str, v: i64| -> H5 { d.new_attr::<i64>().shape(()).create(name)?.as_writer().write_scalar(&v) };
    let attr_s = |d: &hdf5_metno::Dataset, name: &str, v: &str| -> H5 { d.new_attr::<VarLenUnicode>().shape(()).create(name)?.as_writer().write_scalar(&vlu(v)) };

    let measurements: Vec<FlowMeasurementRow> = flow.measurements.iter().filter(|m| (m.edge_id as usize) < n_edges).copied().collect();
    let runs = f.new_dataset_builder().deflate(4).with_data(&flow.runs[..]).create("flow_runs")?;
    attr_i(&runs, "schema_version", 1)?;
    attr_s(&runs, "backbone_fingerprint", &fingerprint)?;
    let m = f.new_dataset_builder().deflate(4).with_data(&measurements[..]).create("flow_measurements")?;
    attr_i(&m, "schema_version", 1)?;
    attr_s(&m, "backbone_fingerprint", &fingerprint)?;
    m.new_attr::<f64>().shape(()).create("cutoff_um")?.as_writer().write_scalar(&(flow.cutoff_um as f64))?;
    attr_i(&m, "n_anchors", flow.n_anchors)?;
    attr_i(&m, "n_pins", flow.n_pins)?;
    attr_s(&m, "written_by", "GrAMF")?;

    let names: Vec<VarLenUnicode> = a.trails.keys().map(|n| vlu(n)).collect();
    let (mut offsets, mut nodes) = (vec![0i64], vec![]);
    for n in a.trails.keys() {
        nodes.extend(trail_paths.get(n).cloned().unwrap_or_default());
        offsets.push(nodes.len() as i64);
    }
    let d = f.new_dataset_builder().with_data(&names[..]).create("flow_trails_names")?;
    attr_i(&d, "schema_version", 1)?;
    f.new_dataset_builder().with_data(&nodes[..]).create("flow_trails_nodes")?;
    f.new_dataset_builder().with_data(&offsets[..]).create("flow_trails_offsets")?;

    let roots: Vec<i64> = a.roots.iter().copied().collect();
    let d = f.new_dataset_builder().deflate(4).with_data(&roots[..]).create("root_nodes")?;
    attr_i(&d, "schema_version", 1)?;
    attr_s(&d, "backbone_fingerprint", &fingerprint)?;
    Ok(Ok(Published {
        measurements: measurements.len(),
        on_added_edges: flow.measurements.len() - measurements.len(),
        trails: names.len(),
        roots: roots.len(),
    }))
}

fn write_file(a: &Annotations, path: &Path, trail_paths: &BTreeMap<String, Vec<i64>>) -> H5 {
    let f = hdf5_metno::File::create(path)?;
    let attr_str = |name: &str, value: &str| -> H5 {
        f.new_attr::<VarLenUnicode>().shape(()).create(name)?.as_writer().write_scalar(&vlu(value))
    };
    attr_str("author", &a.author)?;
    attr_str("backbone_fingerprint", &a.fingerprint)?;
    attr_str("gramf_version", env!("CARGO_PKG_VERSION"))?;
    f.new_attr::<i64>().shape(()).create("annotation_schema_version")?.as_writer().write_scalar(&SCHEMA_VERSION)?;

    let ints = |g: &hdf5_metno::Group, name: &str, v: &[i64]| -> H5 {
        g.new_dataset_builder().with_data(v).create(name).map(drop)
    };
    let strs = |g: &hdf5_metno::Group, name: &str, v: &[String]| -> H5 {
        let v: Vec<VarLenUnicode> = v.iter().map(|s| vlu(s)).collect();
        g.new_dataset_builder().with_data(&v[..]).create(name).map(drop)
    };
    let as_i64 = |it: &mut dyn Iterator<Item = usize>| -> Vec<i64> { it.map(|x| x as i64).collect() };

    let g = f.create_group("excluded")?;
    ints(&g, "edge_id", &as_i64(&mut a.excluded.iter().copied()))?;

    let g = f.create_group("duplicate_of")?;
    ints(&g, "edge_id", &as_i64(&mut a.duplicate_of.keys().copied()))?;
    ints(&g, "of_edge_id", &as_i64(&mut a.duplicate_of.values().copied()))?;

    let g = f.create_group("hyperedge_overrides")?;
    ints(&g, "edge_id", &as_i64(&mut a.hyperedge.keys().copied()))?;
    ints(&g, "hyperedge", &a.hyperedge.values().copied().collect::<Vec<_>>())?;

    let g = f.create_group("roots")?;
    ints(&g, "node_id", &a.roots.iter().copied().collect::<Vec<_>>())?;

    let g = f.create_group("added_edges")?;
    ints(&g, "id", &a.added.iter().map(|e| e.id).collect::<Vec<_>>())?;
    ints(&g, "u", &a.added.iter().map(|e| e.u).collect::<Vec<_>>())?;
    ints(&g, "v", &a.added.iter().map(|e| e.v).collect::<Vec<_>>())?;

    let g = f.create_group("notes")?;
    let (kind, id): (Vec<String>, Vec<i64>) = a
        .notes
        .keys()
        .map(|t| match t {
            Target::Edge(e) => ("edge".to_string(), *e as i64),
            Target::Node(n) => ("node".to_string(), *n),
        })
        .unzip();
    strs(&g, "kind", &kind)?;
    ints(&g, "id", &id)?;
    strs(&g, "text", &a.notes.values().cloned().collect::<Vec<_>>())?;

    // Trails: names, waypoints and routed node paths, both CSR (offsets has n + 1 entries).
    let g = f.create_group("trails")?;
    let csr = |lists: &mut dyn Iterator<Item = Vec<i64>>| -> (Vec<i64>, Vec<i64>) {
        let (mut offsets, mut values) = (vec![0], vec![]);
        for l in lists {
            values.extend(l);
            offsets.push(values.len() as i64);
        }
        (offsets, values)
    };
    strs(&g, "name", &a.trails.keys().cloned().collect::<Vec<_>>())?;
    let (wo, wv) = csr(&mut a.trails.values().cloned());
    ints(&g, "waypoint_offsets", &wo)?;
    ints(&g, "waypoints", &wv)?;
    let (no, nv) = csr(&mut a.trails.keys().map(|n| trail_paths.get(n).cloned().unwrap_or_default()));
    ints(&g, "node_offsets", &no)?;
    ints(&g, "nodes", &nv)?;

    write_registration(&f, &a.registration, &ints, &strs)?;
    if let Some(flow) = &a.flow {
        f.new_dataset_builder().with_data(&flow.runs[..]).create("flow_runs")?;
        let m = f.new_dataset_builder().with_data(&flow.measurements[..]).create("flow_measurements")?;
        let attr_i = |name: &str, v: i64| -> H5 { m.new_attr::<i64>().shape(()).create(name)?.as_writer().write_scalar(&v) };
        attr_i("schema_version", 1)?;
        attr_i("n_anchors", flow.n_anchors)?;
        attr_i("n_pins", flow.n_pins)?;
        m.new_attr::<f64>().shape(()).create("cutoff_um")?.as_writer().write_scalar(&(flow.cutoff_um as f64))?;
        for (name, value) in [("backbone_fingerprint", a.fingerprint.as_str()), ("written_by", "GrAMF")] {
            m.new_attr::<VarLenUnicode>().shape(()).create(name)?.as_writer().write_scalar(&vlu(value))?;
        }
    }

    let g = f.create_group("history")?;
    ints(&g, "utc", &a.history.iter().map(|h| h.utc).collect::<Vec<_>>())?;
    strs(&g, "author", &a.history.iter().map(|h| h.author.clone()).collect::<Vec<_>>())?;
    strs(&g, "op", &a.history.iter().map(|h| h.op.clone()).collect::<Vec<_>>())?;
    Ok(())
}

type Ints<'a> = dyn Fn(&hdf5_metno::Group, &str, &[i64]) -> H5 + 'a;
type Strs<'a> = dyn Fn(&hdf5_metno::Group, &str, &[String]) -> H5 + 'a;

fn write_registration(f: &hdf5_metno::File, r: &RegistrationLayer, ints: &Ints, strs: &Strs) -> H5 {
    let g = f.create_group("registration")?;
    let floats = |g: &hdf5_metno::Group, name: &str, v: &[f64]| -> H5 { g.new_dataset_builder().with_data(v).create(name).map(drop) };
    let attr_f = |name: &str, v: f64| -> H5 { g.new_attr::<f64>().shape(()).create(name)?.as_writer().write_scalar(&v) };
    attr_f("camera", r.camera.map_or(-1.0, f64::from))?;
    attr_f("threshold_um", r.threshold_um.map_or(f64::NAN, f64::from))?;
    attr_f("snap", r.snap.map_or(-1.0, |s| s as u8 as f64))?;

    let a = g.create_group("anchors")?;
    strs(&a, "position", &r.anchors.keys().cloned().collect::<Vec<_>>())?;
    floats(&a, "x_um", &r.anchors.values().map(|p| p[0]).collect::<Vec<_>>())?;
    floats(&a, "y_um", &r.anchors.values().map(|p| p[1]).collect::<Vec<_>>())?;

    let v = g.create_group("verified")?;
    strs(&v, "session", &r.verified.iter().cloned().collect::<Vec<_>>())?;

    let n = g.create_group("nudges")?;
    strs(&n, "run_path", &r.nudges.keys().cloned().collect::<Vec<_>>())?;
    floats(&n, "dx_um", &r.nudges.values().map(|p| p[0] as f64).collect::<Vec<_>>())?;
    floats(&n, "dy_um", &r.nudges.values().map(|p| p[1] as f64).collect::<Vec<_>>())?;

    let run_edges = |g: &hdf5_metno::Group, keys: &mut dyn Iterator<Item = &RunEdge>| -> H5 {
        let keys: Vec<&RunEdge> = keys.collect();
        strs(g, "run_path", &keys.iter().map(|k| k.0.clone()).collect::<Vec<_>>())?;
        for (i, name) in ["u", "v", "k"].iter().enumerate() {
            ints(g, name, &keys.iter().map(|k| k.1[i]).collect::<Vec<_>>())?;
        }
        Ok(())
    };
    let s = g.create_group("assignments")?;
    run_edges(&s, &mut r.assignments.keys())?;
    let (mut offsets, mut edge, mut added, mut dir) = (vec![0i64], vec![], vec![], vec![]);
    for chain in r.assignments.values() {
        for (e, d) in chain {
            let (id, is_added) = match e {
                EdgeRef::Pipeline(e) => (*e as i64, 0),
                EdgeRef::Added(id) => (*id, 1),
            };
            edge.push(id);
            added.push(is_added);
            dir.push(*d as i64);
        }
        offsets.push(edge.len() as i64);
    }
    ints(&s, "chain_offsets", &offsets)?;
    ints(&s, "chain_edge", &edge)?;
    ints(&s, "chain_added", &added)?;
    ints(&s, "chain_direction", &dir)?;

    let d = g.create_group("disabled")?;
    run_edges(&d, &mut r.disabled.iter())
}

fn read_registration(g: &hdf5_metno::Group) -> hdf5_metno::Result<RegistrationLayer> {
    let ints = |name: &str| g.dataset(name).and_then(|d| d.read_raw::<i64>());
    let floats = |name: &str| g.dataset(name).and_then(|d| d.read_raw::<f64>());
    let strs = |name: &str| -> hdf5_metno::Result<Vec<String>> {
        Ok(g.dataset(name)?.read_raw::<VarLenUnicode>()?.iter().map(|s| s.to_string()).collect())
    };
    let attr = |name: &str| g.attr(name).and_then(|a| a.read_scalar::<f64>()).unwrap_or(f64::NAN);
    let mut r = RegistrationLayer {
        camera: Some(attr("camera")).filter(|c| *c >= 0.0).map(|c| c as u8),
        threshold_um: Some(attr("threshold_um")).filter(|t| t.is_finite()).map(|t| t as f32),
        snap: Some(attr("snap")).filter(|s| *s >= 0.0).map(|s| s != 0.0),
        ..Default::default()
    };
    let (names, xs, ys) = (strs("anchors/position")?, floats("anchors/x_um")?, floats("anchors/y_um")?);
    r.anchors = (0..names.len()).map(|i| (names[i].clone(), [xs[i], ys[i]])).collect();
    r.verified = strs("verified/session")?.into_iter().collect();
    let (runs, dx, dy) = (strs("nudges/run_path")?, floats("nudges/dx_um")?, floats("nudges/dy_um")?);
    r.nudges = (0..runs.len()).map(|i| (runs[i].clone(), [dx[i] as f32, dy[i] as f32])).collect();
    let run_edges = |p: &str| -> hdf5_metno::Result<Vec<RunEdge>> {
        let (runs, u, v, k) = (strs(&format!("{p}/run_path"))?, ints(&format!("{p}/u"))?, ints(&format!("{p}/v"))?, ints(&format!("{p}/k"))?);
        Ok((0..runs.len()).map(|i| (runs[i].clone(), [u[i], v[i], k[i]])).collect())
    };
    let keys = run_edges("assignments")?;
    let (offsets, edge, added, dir) =
        (ints("assignments/chain_offsets")?, ints("assignments/chain_edge")?, ints("assignments/chain_added")?, ints("assignments/chain_direction")?);
    r.assignments = keys
        .into_iter()
        .enumerate()
        .map(|(i, key)| {
            let chain = (offsets[i] as usize..offsets[i + 1] as usize)
                .map(|j| {
                    let e = if added[j] != 0 { EdgeRef::Added(edge[j]) } else { EdgeRef::Pipeline(edge[j] as usize) };
                    (e, if dir[j] >= 0 { 1 } else { -1 })
                })
                .collect();
            (key, chain)
        })
        .collect();
    r.disabled = run_edges("disabled")?.into_iter().collect();
    Ok(r)
}

pub fn load(path: &Path) -> Result<Annotations, String> {
    read_file(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn read_file(path: &Path) -> hdf5_metno::Result<Annotations> {
    let f = hdf5_metno::File::open(path)?;
    let attr_str = |name: &str| -> String {
        f.attr(name)
            .and_then(|a| a.read_scalar::<VarLenUnicode>())
            .map(|s| s.to_string())
            .unwrap_or_default()
    };
    let ints = |name: &str| f.dataset(name).and_then(|d| d.read_raw::<i64>());
    let strs = |name: &str| -> hdf5_metno::Result<Vec<String>> {
        Ok(f.dataset(name)?.read_raw::<VarLenUnicode>()?.iter().map(|s| s.to_string()).collect())
    };
    let idx = |v: i64| v as usize;

    let mut a = Annotations::new(&attr_str("author"), &attr_str("backbone_fingerprint"));
    a.excluded = ints("excluded/edge_id")?.into_iter().map(idx).collect();
    a.duplicate_of = ints("duplicate_of/edge_id")?
        .into_iter()
        .zip(ints("duplicate_of/of_edge_id")?)
        .map(|(e, k)| (idx(e), idx(k)))
        .collect();
    a.hyperedge = ints("hyperedge_overrides/edge_id")?
        .into_iter()
        .map(idx)
        .zip(ints("hyperedge_overrides/hyperedge")?)
        .collect();
    a.roots = ints("roots/node_id")?.into_iter().collect();
    let (ids, us, vs) = (ints("added_edges/id")?, ints("added_edges/u")?, ints("added_edges/v")?);
    a.added = (0..ids.len()).map(|i| AddedEdge { id: ids[i], u: us[i], v: vs[i] }).collect();
    a.notes = strs("notes/kind")?
        .into_iter()
        .zip(ints("notes/id")?)
        .zip(strs("notes/text")?)
        .map(|((kind, id), text)| {
            let target = if kind == "node" { Target::Node(id) } else { Target::Edge(idx(id)) };
            (target, text)
        })
        .collect();
    if let Ok(g) = f.group("registration") {
        a.registration = read_registration(&g)?;
    }
    if let (Ok(runs), Ok(m)) = (f.dataset("flow_runs"), f.dataset("flow_measurements")) {
        let attr_i = |n: &str| m.attr(n).and_then(|x| x.read_scalar::<i64>()).unwrap_or(0);
        a.flow = Some(FlowOutput {
            runs: runs.read_raw::<FlowRunRow>()?,
            measurements: m.read_raw::<FlowMeasurementRow>()?,
            cutoff_um: m.attr("cutoff_um").and_then(|x| x.read_scalar::<f64>()).unwrap_or(f64::NAN) as f32,
            n_anchors: attr_i("n_anchors"),
            n_pins: attr_i("n_pins"),
        });
    }
    // Optional: files written before trails existed have no /trails.
    if let (Ok(names), Ok(offsets), Ok(points)) =
        (strs("trails/name"), ints("trails/waypoint_offsets"), ints("trails/waypoints"))
    {
        a.trails = names
            .into_iter()
            .enumerate()
            .map(|(i, name)| (name, points[offsets[i] as usize..offsets[i + 1] as usize].to_vec()))
            .collect();
    }
    let (utc, authors, ops) = (ints("history/utc")?, strs("history/author")?, strs("history/op")?);
    a.history = (0..utc.len())
        .map(|i| HistoryRow { utc: utc[i], author: authors[i].clone(), op: ops[i].clone() })
        .collect();
    Ok(a)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        };
        let run = FlowRunRow { run_path: vlu("pos/run"), mode: vlu("brightfield"), run_nr: -1, video_epoch_s: 1.0, frame_timestamp: 2.0 };
        a.flow = Some(FlowOutput { runs: vec![run], measurements: vec![m(4), m(ADDED_EDGE_ID_START)], cutoff_um: 8.0, n_anchors: 2, n_pins: 0 });
        let paths = BTreeMap::from([("main".to_string(), vec![1, 5, 2])]);

        let p = publish(&a, &store, 10, &paths).unwrap();
        assert_eq!(p, Published { measurements: 1, on_added_edges: 1, trails: 1, roots: 1 });
        let f = hdf5_metno::File::open(&store).unwrap();
        assert_eq!(f.dataset("root_nodes").unwrap().read_raw::<i64>().unwrap(), [3]);
        assert_eq!(f.dataset("flow_trails_nodes").unwrap().read_raw::<i64>().unwrap(), [1, 5, 2]);
        assert_eq!(f.dataset("flow_measurements").unwrap().read_raw::<FlowMeasurementRow>().unwrap()[0].edge_id, 4);
        drop(f);

        a.fingerprint = "changed".into();
        assert!(publish(&a, &store, 10, &paths).is_err());
        assert!(!store.with_extension("h5.publishing").exists());
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
        let row = |s: &str| VarLenUnicode::from_str(s).unwrap();
        a.flow = Some(FlowOutput {
            runs: vec![FlowRunRow { run_path: row("pos/20260609_150109"), mode: row("brightfield"), run_nr: -1, video_epoch_s: 1.0, frame_timestamp: 2.0 }],
            measurements: vec![FlowMeasurementRow {
                edge_id: 23158, direction: -1, run_index: 0, src_u: 1, src_v: 6, src_k: 0,
                median: 1.5, perc_25: 0.5, perc_75: 3.0, flux_ratio: 0.5, coverage: 0.4, pixel_count: 100.0, flux: 150.0,
            }],
            cutoff_um: 12.5,
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
        assert_eq!(b.history.len(), 13);
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
