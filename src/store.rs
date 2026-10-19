//! Read-only access to STHype's `stg_store.h5`. GrAMF never opens a store for writing
//! (DESIGN.md §2); `hdf5_metno::File::open` is read-only.

use std::collections::HashMap;
use std::path::Path;

#[derive(Default)]
/// Edge polylines in µm, from `/arcs_points` + `/arcs_offsets` (N+1 CSR):
/// edge `i` is `points[offsets[i]..offsets[i + 1]]`. `x` is the image column, `y` the row.
pub struct Arcs {
    pub offsets: Vec<usize>,
    pub points: Vec<[f32; 2]>,
    pub min: [f32; 2],
    pub max: [f32; 2],
}

impl Arcs {
    pub fn n_edges(&self) -> usize {
        self.offsets.len() - 1
    }
}

/// What GrAMF reads from one store.
#[derive(Default)]
pub struct Store {
    pub arcs: Arcs,
    /// Timepoint index at which each arc vertex first appears, row-aligned with
    /// `arcs.points`. All zero when the store carries no dating.
    pub vertex_t: Vec<f32>,
    /// `/timepoints` (epoch seconds) and `/timepoints_quality_weight` (0 = gated frame);
    /// empty when absent.
    pub timepoints: Vec<i64>,
    pub quality: Vec<f64>,

    /// `/nodes`: pipeline node id and position (µm).
    pub node_ids: Vec<i64>,
    pub node_pos: Vec<[f32; 2]>,
    /// Node id -> row in `node_ids`.
    pub node_index: HashMap<i64, usize>,
    /// `/edges`: endpoint node ids, row = `edge_id`. Empty when absent.
    pub edge_nodes: Vec<[i64; 2]>,
    /// Per edge (row = `edge_id`):
    pub edge_length_um: Vec<f32>,
    /// Earliest appearance along the edge (timepoint index).
    pub edge_t: Vec<f32>,
    /// `/hyperedges`; -1 = none (also when the dataset is absent).
    pub hyperedge: Vec<i64>,
    /// Length-weighted `(width_um, width_se_um)` over `/edge_width_segments`.
    pub width: Vec<Option<(f32, f32)>>,
    /// Per node: appearance time of its earliest incident edge (-inf without edges).
    pub node_t: Vec<f32>,
    /// Per hyperedge: id, label anchor (middle of its longest edge) and first appearance.
    pub hyperedge_anchors: Vec<(i64, [f32; 2], f32)>,
    /// Root attr `backbone_fingerprint`; annotation files record it to detect staleness.
    pub fingerprint: String,
    /// `/root_nodes` written by the old registrator, offered for import into annotations.
    pub root_nodes: Vec<i64>,
}

/// The `/edge_width_segments` columns GrAMF uses (HDF5 converts compound members by name).
#[derive(hdf5_metno::H5Type, Clone, Copy)]
#[repr(C)]
struct WidthSegment {
    edge_id: i64,
    s_start_um: f64,
    s_end_um: f64,
    width_um: f64,
    width_se_um: f64,
}

/// One `/edge_width_timesteps` row (schema v2): both width estimators per (edge, frame).
#[derive(hdf5_metno::H5Type, Clone, Copy)]
#[repr(C)]
pub struct WidthTimestep {
    pub edge_id: i64,
    pub timepoint_index: i64,
    pub width_um: f64,
    pub width_seg_um: f64,
}

/// Read on demand (exports only): ~0.6M rows on a large plate.
pub fn read_width_timesteps(path: &Path) -> Result<Vec<WidthTimestep>, String> {
    let file = hdf5_metno::File::open(path).map_err(|e| e.to_string())?;
    file.dataset("edge_width_timesteps")
        .and_then(|d| d.read_raw::<WidthTimestep>())
        .map_err(|e| format!("/edge_width_timesteps: {e}"))
}

pub fn read_store(path: &Path) -> Result<Store, String> {
    let file = hdf5_metno::File::open(path).map_err(|e| e.to_string())?;
    let read_f64 = |name: &str| file.dataset(name).and_then(|d| d.read_raw::<f64>());
    let read_i64 = |name: &str| file.dataset(name).and_then(|d| d.read_raw::<i64>());

    let offsets = read_i64("arcs_offsets")
        .map_err(|_| "/arcs_offsets missing — not an STG skeleton store".to_string())?;
    let points = read_f64("arcs_points")
        .map_err(|_| "/arcs_points missing — not an STG skeleton store".to_string())?;
    let arcs = arcs_from_raw(&offsets, &points)?;

    // arc-unit stores date stretches of an arc; anchor-seg stores date whole edges.
    let vertex_t = if let Ok(runs) = read_f64("arc_time_runs") {
        vertex_t_from_runs(&arcs, &runs)?
    } else if let Ok(t_first) = read_i64("t_first") {
        if t_first.len() != arcs.n_edges() {
            return Err(format!(
                "/t_first has {} rows for {} edges",
                t_first.len(),
                arcs.n_edges()
            ));
        }
        arcs.offsets
            .windows(2)
            .zip(&t_first)
            .flat_map(|(e, &t)| std::iter::repeat_n(t as f32, e[1] - e[0]))
            .collect()
    } else {
        vec![0.0; arcs.points.len()]
    };

    let timepoints = read_i64("timepoints").unwrap_or_default();
    let quality = read_f64("timepoints_quality_weight")
        .unwrap_or_else(|_| vec![1.0; timepoints.len()]);
    if quality.len() != timepoints.len() {
        return Err("/timepoints_quality_weight and /timepoints differ in length".into());
    }

    let n_edges = arcs.n_edges();
    let check = |name: &str, len: usize| {
        if len == n_edges {
            Ok(())
        } else {
            Err(format!("/{name} has {len} rows for {n_edges} edges"))
        }
    };

    let nodes = read_f64("nodes").unwrap_or_default();
    let node_ids: Vec<i64> = nodes.chunks_exact(3).map(|n| n[0] as i64).collect();
    let node_pos = nodes.chunks_exact(3).map(|n| [n[1] as f32, n[2] as f32]).collect();
    let node_index: HashMap<i64, usize> = node_ids.iter().enumerate().map(|(i, &id)| (id, i)).collect();

    let edge_nodes: Vec<[i64; 2]> = read_i64("edges")
        .unwrap_or_default()
        .chunks_exact(2)
        .map(|e| [e[0], e[1]])
        .collect();
    if !edge_nodes.is_empty() {
        check("edges", edge_nodes.len())?;
    }

    let edge_length_um = match read_f64("edge_lengths_um") {
        Ok(l) => {
            check("edge_lengths_um", l.len())?;
            l.iter().map(|&x| x as f32).collect()
        }
        Err(_) => polyline_lengths(&arcs),
    };

    let edge_t: Vec<f32> = arcs
        .offsets
        .windows(2)
        .map(|e| vertex_t[e[0]..e[1]].iter().copied().fold(f32::INFINITY, f32::min))
        .collect();

    let hyperedge = match read_i64("hyperedges") {
        Ok(h) => {
            check("hyperedges", h.len())?;
            h
        }
        Err(_) => vec![-1; n_edges],
    };

    let mut width = vec![None; n_edges];
    if let Ok(segments) = file
        .dataset("edge_width_segments")
        .and_then(|d| d.read_raw::<WidthSegment>())
    {
        width = mean_widths(n_edges, &segments)?;
    }

    let mut node_t = vec![f32::INFINITY; node_ids.len()];
    for (e, ends) in edge_nodes.iter().enumerate() {
        for id in ends {
            if let Some(&n) = node_index.get(id) {
                node_t[n] = node_t[n].min(edge_t[e]);
            }
        }
    }
    for t in node_t.iter_mut().filter(|t| t.is_infinite()) {
        *t = f32::NEG_INFINITY;
    }

    // (longest edge, its length, earliest t) per hyperedge
    let mut by_h: HashMap<i64, (usize, f32, f32)> = HashMap::new();
    for (e, &h) in hyperedge.iter().enumerate().filter(|(_, h)| **h >= 0) {
        let entry = by_h.entry(h).or_insert((e, -1.0, f32::INFINITY));
        if edge_length_um[e] > entry.1 {
            (entry.0, entry.1) = (e, edge_length_um[e]);
        }
        entry.2 = entry.2.min(edge_t[e]);
    }
    let hyperedge_anchors = by_h
        .into_iter()
        .map(|(h, (e, _, t))| (h, arcs.points[(arcs.offsets[e] + arcs.offsets[e + 1]) / 2], t))
        .collect();

    let fingerprint = file
        .attr("backbone_fingerprint")
        .and_then(|a| a.read_scalar::<hdf5_metno::types::VarLenUnicode>())
        .map(|s| s.to_string())
        .unwrap_or_default();
    let root_nodes = read_i64("root_nodes").unwrap_or_default();

    Ok(Store {
        arcs,
        vertex_t,
        timepoints,
        quality,
        node_ids,
        node_pos,
        node_index,
        edge_nodes,
        edge_length_um,
        edge_t,
        hyperedge,
        width,
        node_t,
        hyperedge_anchors,
        fingerprint,
        root_nodes,
    })
}

fn polyline_lengths(arcs: &Arcs) -> Vec<f32> {
    arcs.offsets
        .windows(2)
        .map(|e| {
            arcs.points[e[0]..e[1]]
                .windows(2)
                .map(|w| ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt())
                .sum()
        })
        .collect()
}

/// Length-weighted mean width and standard error per edge; NaN widths are skipped.
fn mean_widths(n_edges: usize, segments: &[WidthSegment]) -> Result<Vec<Option<(f32, f32)>>, String> {
    let mut sums = vec![(0.0_f64, 0.0_f64, 0.0_f64); n_edges];
    for seg in segments {
        let sum = usize::try_from(seg.edge_id)
            .ok()
            .and_then(|e| sums.get_mut(e))
            .ok_or("/edge_width_segments names an edge out of range")?;
        let len = (seg.s_end_um - seg.s_start_um).max(0.0);
        if seg.width_um.is_finite() && len > 0.0 {
            *sum = (sum.0 + len * seg.width_um, sum.1 + len * seg.width_se_um, sum.2 + len);
        }
    }
    Ok(sums
        .into_iter()
        .map(|(w, se, len)| (len > 0.0).then(|| ((w / len) as f32, (se / len) as f32)))
        .collect())
}

impl Store {
    pub fn n_edges(&self) -> usize {
        self.arcs.n_edges()
    }

    /// Edges incident to node row `node`.
    pub fn incident_edges(&self, node: usize) -> Vec<usize> {
        let id = self.node_ids[node];
        (0..self.edge_nodes.len())
            .filter(|&e| self.edge_nodes[e].contains(&id))
            .collect()
    }

    /// Edges of hyperedge `h`.
    pub fn hyperedge_edges(&self, h: i64) -> Vec<usize> {
        (0..self.hyperedge.len()).filter(|&e| self.hyperedge[e] == h).collect()
    }

    /// Nearest node within `radius` µm that has appeared by timepoint `t`.
    pub fn pick_node(&self, at: [f32; 2], radius: f32, t: f32) -> Option<usize> {
        nearest(
            (0..self.node_pos.len()).filter(|&n| self.node_t[n] <= t),
            |n| dist2(at, self.node_pos[n]),
            radius,
        )
    }

    /// Nearest edge within `radius` µm, counting only arc segments visible at timepoint `t`.
    // ponytail: brute force over every segment (~1.4M, a few ms per click); grid index if it drags
    pub fn pick_edge(&self, at: [f32; 2], radius: f32, t: f32) -> Option<usize> {
        let (p, vt) = (&self.arcs.points, &self.vertex_t);
        nearest(
            0..self.n_edges(),
            |e| {
                let r = self.arcs.offsets[e]..self.arcs.offsets[e + 1];
                r.clone()
                    .zip(r.skip(1))
                    .filter(|&(a, b)| vt[a].min(vt[b]) <= t)
                    .map(|(a, b)| segment_dist2(at, p[a], p[b]))
                    .fold(f32::INFINITY, f32::min)
            },
            radius,
        )
    }
}

fn nearest(items: impl Iterator<Item = usize>, d2: impl Fn(usize) -> f32, radius: f32) -> Option<usize> {
    items
        .map(|i| (i, d2(i)))
        .filter(|&(_, d)| d <= radius * radius)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

fn dist2(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)
}

pub fn segment_dist2(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let len2 = ab[0] * ab[0] + ab[1] * ab[1];
    let s = if len2 > 0.0 {
        (((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    dist2(p, [a[0] + s * ab[0], a[1] + s * ab[1]])
}

/// `/arc_time_runs` rows are `(edge_id, s_start_um, t_index)`, sorted by edge then `s`;
/// each edge's runs tile `[0, length]`. A vertex takes the label of the last run starting
/// at or before its arc length.
fn vertex_t_from_runs(arcs: &Arcs, flat_runs: &[f64]) -> Result<Vec<f32>, String> {
    let mut vertex_t = vec![0.0; arcs.points.len()];
    let runs: Vec<&[f64]> = flat_runs.chunks_exact(3).collect();
    let mut r = 0;
    for (edge, range) in arcs.offsets.windows(2).enumerate() {
        let start = r;
        while r < runs.len() && runs[r][0] as usize == edge {
            r += 1;
        }
        let edge_runs = &runs[start..r];
        let mut s = 0.0_f32;
        let mut k = 0;
        for v in range[0]..range[1] {
            if v > range[0] {
                let (a, b) = (arcs.points[v - 1], arcs.points[v]);
                s += ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
            }
            while k + 1 < edge_runs.len() && edge_runs[k + 1][1] as f32 <= s {
                k += 1;
            }
            if let Some(run) = edge_runs.get(k) {
                vertex_t[v] = run[2] as f32;
            }
        }
    }
    if r != runs.len() {
        return Err("/arc_time_runs is not sorted by edge, or names an edge out of range".into());
    }
    Ok(vertex_t)
}

/// Validates the CSR pair: the file is outside data, a bad offset would index out of
/// bounds or silently draw the wrong polyline.
fn arcs_from_raw(offsets: &[i64], flat_points: &[f64]) -> Result<Arcs, String> {
    let n_points = flat_points.len() / 2;
    if offsets.first() != Some(&0)
        || offsets.last() != Some(&(n_points as i64))
        || offsets.windows(2).any(|w| w[1] < w[0])
    {
        return Err(format!(
            "/arcs_offsets is not a monotone CSR index over {n_points} points"
        ));
    }

    let points: Vec<[f32; 2]> = flat_points
        .chunks_exact(2)
        .map(|p| [p[0] as f32, p[1] as f32])
        .collect();
    let mut min = [f32::INFINITY; 2];
    let mut max = [f32::NEG_INFINITY; 2];
    for p in &points {
        for k in 0..2 {
            min[k] = min[k].min(p[k]);
            max[k] = max[k].max(p[k]);
        }
    }

    Ok(Arcs {
        offsets: offsets.iter().map(|&o| o as usize).collect(),
        points,
        min,
        max,
    })
}

#[cfg(test)]
mod tests {
    use super::{WidthSegment, arcs_from_raw, mean_widths, segment_dist2, vertex_t_from_runs};

    #[test]
    fn widths_and_segment_distance() {
        let seg = |edge_id, s_start_um, s_end_um, width_um| WidthSegment {
            edge_id,
            s_start_um,
            s_end_um,
            width_um,
            width_se_um: 1.0,
        };
        let w = mean_widths(3, &[seg(0, 0.0, 10.0, 4.0), seg(0, 10.0, 40.0, 8.0), seg(1, 0.0, 5.0, f64::NAN)]).unwrap();
        assert_eq!(w, [Some((7.0, 1.0)), None, None]);
        assert!(mean_widths(1, &[seg(4, 0.0, 1.0, 1.0)]).is_err());

        assert_eq!(segment_dist2([5.0, 3.0], [0.0, 0.0], [10.0, 0.0]), 9.0);
        assert_eq!(segment_dist2([-4.0, 3.0], [0.0, 0.0], [10.0, 0.0]), 25.0);
    }

    #[test]
    fn runs_label_vertices_by_arc_length() {
        // edge 0: x = 0, 4, 10 (s = 0, 4, 10); edge 1: two vertices
        let pts = [0.0, 0.0, 4.0, 0.0, 10.0, 0.0, 0.0, 5.0, 0.0, 6.0];
        let arcs = arcs_from_raw(&[0, 3, 5], &pts).unwrap();
        let runs = [0.0, 0.0, 3.0, 0.0, 5.0, 7.0, 1.0, 0.0, 9.0];
        assert_eq!(vertex_t_from_runs(&arcs, &runs).unwrap(), [3.0, 3.0, 7.0, 9.0, 9.0]);
        // an edge id past the last edge is rejected
        assert!(vertex_t_from_runs(&arcs, &[5.0, 0.0, 1.0]).is_err());
    }

    #[test]
    fn csr_validation() {
        let pts = [0.0, 0.0, 1.0, 2.0, 3.0, -1.0];
        let arcs = arcs_from_raw(&[0, 2, 3], &pts).unwrap();
        assert_eq!(arcs.n_edges(), 2);
        assert_eq!((arcs.min, arcs.max), ([0.0, -1.0], [3.0, 2.0]));

        assert!(arcs_from_raw(&[0, 2], &pts).is_err()); // doesn't cover all points
        assert!(arcs_from_raw(&[0, 3, 2, 3], &pts).is_err()); // not monotone
        assert!(arcs_from_raw(&[1, 3], &pts).is_err()); // doesn't start at 0
    }
}
