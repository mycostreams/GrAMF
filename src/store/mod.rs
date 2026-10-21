//! Read-only access to STHype's `stg_store.h5`. GrAMF never opens a store for writing
//! (DESIGN.md §2); `hdf5_metno::File::open` is read-only.

use std::collections::HashMap;
use std::path::Path;

mod detached;
mod grid;
mod route;
mod width;

pub use detached::{DETACHED_END_UM, DetachedEnd};
pub use grid::{SegmentGrid, segment_dist2};
use grid::{dist2, nearest};
pub use width::{WidthSegment, WidthTimestep, read_width_segments, read_width_timesteps};
use width::{mean_widths, width_table};

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
    /// Width per (timepoint, edge) from `/edge_width_timesteps`, flat `[t * n_edges + e]`:
    /// the segmenter estimate when valid, else the legacy one (the pipeline's own preference),
    /// carried forward from the last measured frame; NaN before the first. Empty when the
    /// store has no per-timestep widths.
    pub width_t: Vec<f32>,
    /// 5th, 50th and 95th percentile of the measured per-timestep widths.
    pub width_t_quantiles: Option<[f32; 3]>,
    /// Edge id per unordered node pair `(min, max)`; the first edge when pairs repeat.
    pub edge_by_nodes: HashMap<(i64, i64), usize>,
    /// Trails the old registrator wrote (`/flow_trails_*`), offered for import: name and
    /// routed node path.
    pub trails_in_store: Vec<(String, Vec<i64>)>,
    /// The registrator's flow layer in this store (`/flow_runs`, `/flow_measurements`), shown
    /// read-only until it is imported.
    pub flow_in_store: Option<FlowLayer>,
    /// Grid over arc segments for nearest-arc queries; built on first use.
    pub segment_grid: std::sync::OnceLock<SegmentGrid>,
    /// Arc ends more than [`DETACHED_END_UM`] from their node; built on first use.
    pub detached: std::sync::OnceLock<Vec<DetachedEnd>>,
}

pub struct FlowLayer {
    /// `/flow_runs/run_path`: `<position>/<timestamp>` relative to `polyflow_analysis`.
    pub run_paths: Vec<String>,
    pub n_measurements: usize,
    pub written_by: String,
    /// Per run (as `run_paths`), the STG edges its measurements are on, sorted.
    pub run_edges: Vec<Vec<usize>>,
}

impl FlowLayer {
    /// Groups `(run_index, edge_id)` pairs into [`FlowLayer::run_edges`], skipping edges
    /// outside the store (added edges).
    pub fn edges_per_run(n_runs: usize, n_edges: usize, pairs: impl Iterator<Item = (i64, i64)>) -> Vec<Vec<usize>> {
        let mut out = vec![vec![]; n_runs];
        for (run, edge) in pairs {
            if let (Some(list), Ok(e)) = (usize::try_from(run).ok().and_then(|r| out.get_mut(r)), usize::try_from(edge)) {
                if e < n_edges {
                    list.push(e);
                }
            }
        }
        for list in &mut out {
            list.sort_unstable();
            list.dedup();
        }
        out
    }
}

/// The `/flow_measurements` columns GrAMF reads.
#[derive(hdf5_metno::H5Type, Clone, Copy)]
#[repr(C)]
struct FlowMeasurementEdge {
    edge_id: i64,
    run_index: i64,
}

/// The `/flow_runs` column GrAMF reads (HDF5 converts compound members by name).
#[derive(hdf5_metno::H5Type, Clone)]
#[repr(C)]
struct FlowRunRow {
    run_path: hdf5_metno::types::VarLenUnicode,
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
    let mut edge_by_nodes = HashMap::new();
    for (e, &[u, v]) in edge_nodes.iter().enumerate() {
        edge_by_nodes.entry((u.min(v), u.max(v))).or_insert(e);
    }
    let trails_in_store = match (
        file.dataset("flow_trails_names").and_then(|d| d.read_raw::<hdf5_metno::types::VarLenUnicode>()),
        read_i64("flow_trails_offsets"),
        read_i64("flow_trails_nodes"),
    ) {
        (Ok(names), Ok(offsets), Ok(nodes)) if offsets.len() == names.len() + 1 => names
            .iter()
            .enumerate()
            .map(|(i, n)| (n.to_string(), nodes[offsets[i] as usize..offsets[i + 1] as usize].to_vec()))
            .collect(),
        _ => vec![],
    };
    let (width_t, width_t_quantiles) = file
        .dataset("edge_width_timesteps")
        .and_then(|d| d.read_raw::<WidthTimestep>())
        .map(|rows| width_table(&rows, timepoints.len(), n_edges))
        .unwrap_or_default();

    let flow_in_store = file.dataset("flow_runs").and_then(|d| d.read_raw::<FlowRunRow>()).ok().map(|rows| {
        let measurements = file.dataset("flow_measurements").ok();
        let edges = measurements.as_ref().and_then(|d| d.read_raw::<FlowMeasurementEdge>().ok()).unwrap_or_default();
        FlowLayer {
            run_paths: rows.iter().map(|r| r.run_path.to_string()).collect(),
            run_edges: FlowLayer::edges_per_run(rows.len(), n_edges, edges.iter().map(|m| (m.run_index, m.edge_id))),
            n_measurements: measurements.as_ref().map_or(0, |d| d.shape().first().copied().unwrap_or(0)),
            written_by: measurements
                .and_then(|d| d.attr("written_by").and_then(|a| a.read_scalar::<hdf5_metno::types::VarLenUnicode>()).ok())
                .map_or_else(String::new, |s| s.to_string()),
        }
    });

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
        width_t,
        width_t_quantiles,
        edge_by_nodes,
        trails_in_store,
        flow_in_store,
        segment_grid: Default::default(),
        detached: Default::default(),
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

    /// True when edge `e`'s arc runs node2 -> node1 of `/edges` (a few percent of edges).
    pub fn arc_reversed(&self, e: usize) -> bool {
        let (Some(&[n1, n2]), Some(first)) = (self.edge_nodes.get(e), self.arcs.points.get(self.arcs.offsets[e])) else {
            return false;
        };
        let d = |id: &i64| self.node_index.get(id).map_or(f32::INFINITY, |&n| {
            let p = self.node_pos[n];
            (p[0] - first[0]).hypot(p[1] - first[1])
        });
        d(&n2) < d(&n1)
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
    use super::route::dijkstra;
    use super::{WidthSegment, WidthTimestep, arcs_from_raw, mean_widths, segment_dist2, vertex_t_from_runs, width_table};

    #[test]
    fn grid_finds_the_nearest_segment() {
        let pts = [0.0, 0.0, 100.0, 0.0, 0.0, 200.0, 0.0, 300.0];
        let arcs = arcs_from_raw(&[0, 2, 4], &pts).unwrap();
        let grid = super::SegmentGrid::build(&arcs);
        let (d, v) = grid.nearest(&arcs, [50.0, 3.0], 10.0).unwrap();
        assert_eq!((d, v), (3.0, 0));
        assert_eq!(grid.nearest(&arcs, [4.0, 250.0], 10.0).map(|x| x.1), Some(2));
        assert!(grid.nearest(&arcs, [50.0, 60.0], 10.0).is_none());
    }

    #[test]
    fn dijkstra_takes_the_shorter_way() {
        let mut adj: std::collections::HashMap<i64, Vec<(i64, f32)>> = Default::default();
        for (u, v, l) in [(1, 2, 1.0), (2, 3, 1.0), (1, 3, 5.0), (3, 4, 1.0)] {
            adj.entry(u).or_default().push((v, l));
            adj.entry(v).or_default().push((u, l));
        }
        assert_eq!(dijkstra(&adj, 1, 4), Some(vec![1, 2, 3, 4]));
        assert_eq!(dijkstra(&adj, 4, 1), Some(vec![4, 3, 2, 1]));
        assert_eq!(dijkstra(&adj, 1, 99), None);
    }

    #[test]
    fn width_table_prefers_seg_and_carries_forward() {
        let row = |edge_id, timepoint_index, width_um, width_seg_um| WidthTimestep { edge_id, timepoint_index, width_um, width_seg_um };
        let rows = [row(0, 1, 5.0, 7.0), row(0, 3, 6.0, f64::NAN), row(1, 0, 0.0, f64::NAN)];
        let (t, q) = width_table(&rows, 4, 2);
        // edge 0: NaN, 7 (seg), 7 (carried), 6 (legacy); edge 1: never valid
        assert!(t[0].is_nan() && t[2] == 7.0 && t[4] == 7.0 && t[6] == 6.0);
        assert!(t[1].is_nan() && t[7].is_nan());
        assert_eq!(q, Some([6.0, 7.0, 7.0]));
    }

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
