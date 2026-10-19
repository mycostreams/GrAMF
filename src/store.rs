//! Read-only access to STHype's `stg_store.h5`. GrAMF never opens a store for writing
//! (DESIGN.md §2); `hdf5_metno::File::open` is read-only.

use std::path::Path;

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
pub struct Store {
    pub arcs: Arcs,
    /// Timepoint index at which each arc vertex first appears, row-aligned with
    /// `arcs.points`. All zero when the store carries no dating.
    pub vertex_t: Vec<f32>,
    /// `/timepoints` (epoch seconds) and `/timepoints_quality_weight` (0 = gated frame);
    /// empty when absent.
    pub timepoints: Vec<i64>,
    pub quality: Vec<f64>,
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

    Ok(Store {
        arcs,
        vertex_t,
        timepoints,
        quality,
    })
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
    use super::{arcs_from_raw, vertex_t_from_runs};

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
