//! Per-author annotation files, `annotations.<author>.h5` beside the store (DESIGN.md §3).
//! Annotations never touch the store: they are keyed on its `edge_id` / node ids, and
//! record the `backbone_fingerprint` they were made against.

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

    let g = f.create_group("history")?;
    ints(&g, "utc", &a.history.iter().map(|h| h.utc).collect::<Vec<_>>())?;
    strs(&g, "author", &a.history.iter().map(|h| h.author.clone()).collect::<Vec<_>>())?;
    strs(&g, "op", &a.history.iter().map(|h| h.op.clone()).collect::<Vec<_>>())?;
    Ok(())
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
        assert_eq!(b.history.len(), 7);
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
