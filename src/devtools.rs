//! Developer view data (DESIGN.md §7, M6): dataset listing, health checks, raw rows for an
//! edge, and the matcher evidence behind the dating. Everything reads the store read-only and
//! on demand, so the normal view pays nothing for it.

use crate::annotations::Annotations;
use crate::store::Store;
use hdf5_metno::types::VarLenUnicode;
use std::path::Path;

pub struct DatasetInfo {
    pub path: String,
    pub shape: Vec<usize>,
    pub dtype: String,
    pub attrs: Vec<(String, String)>,
}

/// Every dataset in the file (recursing into groups), with its attributes; root attributes
/// are listed under the path "/".
pub fn inspect(path: &Path) -> Result<Vec<DatasetInfo>, String> {
    let file = hdf5_metno::File::open(path).map_err(|e| e.to_string())?;
    let mut out = vec![DatasetInfo { path: "/".into(), shape: vec![], dtype: String::new(), attrs: attrs(&file) }];
    walk(&file, "", &mut out).map_err(|e| e.to_string())?;
    Ok(out)
}

fn walk(group: &hdf5_metno::Group, prefix: &str, out: &mut Vec<DatasetInfo>) -> hdf5_metno::Result<()> {
    for name in group.member_names()? {
        let path = format!("{prefix}/{name}");
        if let Ok(ds) = group.dataset(&name) {
            let dtype = ds.dtype().and_then(|t| t.to_descriptor()).map_or("?".into(), |d| d.to_string());
            out.push(DatasetInfo { path, shape: ds.shape(), dtype, attrs: attrs(&ds) });
        } else if let Ok(g) = group.group(&name) {
            walk(&g, &path, out)?;
        }
    }
    Ok(())
}

fn attrs(loc: &hdf5_metno::Location) -> Vec<(String, String)> {
    loc.attr_names()
        .unwrap_or_default()
        .into_iter()
        .map(|name| {
            let value = loc.attr(&name).map_or("?".into(), |a| {
                a.read_scalar::<VarLenUnicode>()
                    .map(|s| s.to_string())
                    .or_else(|_| a.read_scalar::<i64>().map(|v| v.to_string()))
                    .or_else(|_| a.read_scalar::<f64>().map(|v| v.to_string()))
                    .unwrap_or_else(|_| "<non-scalar>".into())
            });
            (name, value)
        })
        .collect()
}

/// sha256 over `/edges` (int64) then `/points` (float64), as STHype's `backbone_fingerprint`.
pub fn recompute_fingerprint(path: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let file = hdf5_metno::File::open(path).map_err(|e| e.to_string())?;
    let edges = file.dataset("edges").and_then(|d| d.read_raw::<i64>()).map_err(|e| format!("/edges: {e}"))?;
    let points = file.dataset("points").and_then(|d| d.read_raw::<f64>()).map_err(|e| format!("/points: {e}"))?;
    let mut h = Sha256::new();
    edges.iter().for_each(|x| h.update(x.to_le_bytes()));
    points.iter().for_each(|x| h.update(x.to_le_bytes()));
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Ok,
    Warn,
    Fail,
}

pub struct Check {
    pub level: Level,
    pub text: String,
}

fn check(ok: bool, warn_only: bool, text: String) -> Check {
    let level = if ok { Level::Ok } else if warn_only { Level::Warn } else { Level::Fail };
    Check { level, text }
}

/// Schema versions this GrAMF understands; a newer one is shown as a warning, never refused.
fn known_version(dataset: &str) -> Option<i64> {
    match dataset.trim_start_matches('/') {
        "edge_width_timesteps" => Some(2),
        _ => Some(1),
    }
}

/// Consistency checks over the store and the loaded annotation files.
pub fn health(path: &Path, store: &Store, datasets: &[DatasetInfo], annotations: &[&Annotations]) -> Vec<Check> {
    let mut out = vec![];
    let n_edges = store.n_edges();
    let attr = |d: &DatasetInfo, name: &str| d.attrs.iter().find(|a| a.0 == name).map(|a| a.1.clone());

    match recompute_fingerprint(path) {
        Ok(fp) => out.push(check(
            fp == store.fingerprint,
            false,
            format!("backbone_fingerprint matches /edges + /points ({}…)", &fp[..12.min(fp.len())]),
        )),
        Err(e) => out.push(check(false, true, format!("fingerprint not recomputable: {e}"))),
    }
    let mismatched: Vec<&str> = datasets
        .iter()
        .filter(|d| attr(d, "backbone_fingerprint").is_some_and(|f| f != store.fingerprint))
        .map(|d| d.path.as_str())
        .collect();
    out.push(check(
        mismatched.is_empty(),
        false,
        if mismatched.is_empty() {
            "every layer's backbone_fingerprint matches the store".into()
        } else {
            format!("stale layers (fingerprint differs): {}", mismatched.join(", "))
        },
    ));

    for name in ["/edges", "/points", "/t_first", "/hyperedges", "/edge_lengths_um", "/edges_initial_edge"] {
        if let Some(d) = datasets.iter().find(|d| d.path == name) {
            let rows = d.shape.first().copied().unwrap_or(0);
            out.push(check(rows == n_edges, false, format!("{name}: {rows} rows for {n_edges} edges")));
        }
    }
    out.push(check(true, false, format!("/arcs_offsets: monotone CSR over {} points (checked on load)", store.arcs.points.len())));

    let missing: usize = store
        .edge_nodes
        .iter()
        .flatten()
        .filter(|id| !store.node_index.contains_key(id))
        .count();
    out.push(check(missing == 0, false, format!("/edges endpoints missing from /nodes: {missing}")));

    let sorted = store.timepoints.windows(2).all(|w| w[0] < w[1]);
    out.push(check(sorted, false, format!("/timepoints strictly increasing ({} frames)", store.timepoints.len())));
    if !store.timepoints.is_empty() {
        let n = store.timepoints.len() as f32;
        let out_of_range = store.edge_t.iter().filter(|t| t.is_finite() && (**t < 0.0 || **t >= n)).count();
        out.push(check(out_of_range == 0, false, format!("edge appearance outside 0..{n}: {out_of_range} edges")));
    }

    for d in datasets {
        let version = attr(d, "schema_version").and_then(|v| v.parse::<i64>().ok());
        if let (Some(v), Some(known)) = (version, known_version(&d.path)) {
            if v > known {
                out.push(check(false, true, format!("{}: schema_version {v} is newer than GrAMF knows ({known})", d.path)));
            }
        }
    }

    for a in annotations {
        let ok = a.fingerprint.is_empty() || a.fingerprint == store.fingerprint;
        out.push(check(ok, true, format!("annotations.{}.h5 made against this backbone", a.author)));
    }
    out
}

/// Every row about edge `e`: row `e` of each per-edge dataset, and the rows of edge-keyed
/// tables whose first column is `e`. Returns `(dataset, rows)` pairs, rows as text.
pub fn raw_rows(path: &Path, datasets: &[DatasetInfo], n_edges: usize, e: usize) -> Vec<(String, Vec<String>)> {
    let Ok(file) = hdf5_metno::File::open(path) else {
        return vec![];
    };
    let fmt = |v: &[f64]| v.iter().map(|x| format!("{x}")).collect::<Vec<_>>().join(", ");
    let mut out = vec![];
    for d in datasets.iter().filter(|d| d.path != "/") {
        let name = d.path.trim_start_matches('/');
        let numeric = d.dtype.starts_with("int") || d.dtype.starts_with("uint") || d.dtype.starts_with("float");
        let cols = d.shape.get(1).copied().unwrap_or(1).max(1);
        if !numeric || d.shape.len() > 2 {
            continue;
        }
        let Ok(values) = file.dataset(name).and_then(|ds| ds.read_raw::<f64>()) else {
            continue;
        };
        let rows: Vec<&[f64]> = values.chunks_exact(cols).collect();
        let picked: Vec<String> = if rows.len() == n_edges && name != "arcs_offsets" {
            vec![fmt(rows[e])]
        } else if cols >= 2 && ["edge_time_runs", "arc_time_runs", "arc_frame_matches"].contains(&name) {
            rows.iter().filter(|r| r[0] as usize == e).map(|r| fmt(r)).collect()
        } else {
            continue;
        };
        out.push((d.path.clone(), picked));
    }
    if let Ok(rows) = crate::store::read_width_timesteps(path) {
        let picked = rows
            .iter()
            .filter(|r| r.edge_id as usize == e)
            .map(|r| format!("t={} width_um={} width_seg_um={}", r.timepoint_index, r.width_um, r.width_seg_um))
            .collect();
        out.push(("/edge_width_timesteps".into(), picked));
    }
    out
}

/// What the matcher saw: per frame, `(edge_id, a, b)` where `a, b` is the matched arc span in
/// µm (arc-unit stores) or `(score, NaN)` (anchor-seg stores, one hit per edge and frame).
pub struct Evidence {
    pub arc_spans: bool,
    pub by_frame: Vec<Vec<(usize, f32, f32)>>,
    /// Point-matcher distance threshold (µm), to scale scores; lower distance = better.
    pub threshold: f32,
    pub score_semantic: String,
}

pub fn read_evidence(path: &Path, n_frames: usize, n_edges: usize) -> Result<Evidence, String> {
    let file = hdf5_metno::File::open(path).map_err(|e| e.to_string())?;
    let (name, cols, arc_spans) = if file.dataset("arc_frame_matches").is_ok() {
        ("arc_frame_matches", 4, true)
    } else {
        ("edge_time_runs", 3, false)
    };
    let ds = file.dataset(name).map_err(|_| "no matcher evidence (/edge_time_runs or /arc_frame_matches)".to_string())?;
    let attr_f = |n: &str| ds.attr(n).and_then(|a| a.read_scalar::<f64>()).ok();
    let attr_s = |n: &str| ds.attr(n).and_then(|a| a.read_scalar::<VarLenUnicode>()).map(|s| s.to_string()).ok();
    let values = ds.read_raw::<f64>().map_err(|e| format!("/{name}: {e}"))?;
    let mut by_frame = vec![vec![]; n_frames];
    for r in values.chunks_exact(cols) {
        let (e, f) = (r[0] as usize, r[1] as usize);
        if e < n_edges && f < n_frames {
            let (a, b) = if arc_spans { (r[2] as f32, r[3] as f32) } else { (r[2] as f32, f32::NAN) };
            by_frame[f].push((e, a, b));
        }
    }
    Ok(Evidence {
        arc_spans,
        by_frame,
        threshold: attr_f("threshold_um").unwrap_or(172.5) as f32,
        score_semantic: attr_s("score_semantic").unwrap_or_else(|| "distance_um".into()),
    })
}

#[cfg(test)]
mod tests {
    use super::{check, Level};

    #[test]
    fn check_levels() {
        assert!(check(true, false, String::new()).level == Level::Ok);
        assert!(check(false, true, String::new()).level == Level::Warn);
        assert!(check(false, false, String::new()).level == Level::Fail);
    }
}
