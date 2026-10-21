//! Edge widths: the per-segment and per-timestep tables, and the carried-forward width table.

use super::Store;
use std::path::Path;

/// The `/edge_width_segments` columns GrAMF uses (HDF5 converts compound members by name).
#[derive(hdf5_metno::H5Type, Clone, Copy)]
#[repr(C)]
pub struct WidthSegment {
    pub edge_id: i64,
    pub s_start_um: f64,
    pub s_end_um: f64,
    pub width_um: f64,
    pub width_se_um: f64,
}

pub fn read_width_segments(path: &Path) -> Result<Vec<WidthSegment>, String> {
    let file = hdf5_metno::File::open(path).map_err(|e| e.to_string())?;
    file.dataset("edge_width_segments")
        .and_then(|d| d.read_raw::<WidthSegment>())
        .map_err(|e| format!("/edge_width_segments: {e}"))
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

/// Builds [`Store::width_t`] and its quantiles; see there.
pub(super) fn width_table(rows: &[WidthTimestep], n_frames: usize, n_edges: usize) -> (Vec<f32>, Option<[f32; 3]>) {
    if n_frames == 0 {
        return (vec![], None);
    }
    let valid = |w: f64| (w.is_finite() && w > 0.0).then_some(w as f32);
    let mut table = vec![f32::NAN; n_frames * n_edges];
    let mut measured = vec![];
    for r in rows {
        let (Ok(t), Ok(e)) = (usize::try_from(r.timepoint_index), usize::try_from(r.edge_id)) else {
            continue;
        };
        if let (true, Some(w)) = (t < n_frames && e < n_edges, valid(r.width_seg_um).or(valid(r.width_um))) {
            table[t * n_edges + e] = w;
            measured.push(w);
        }
    }
    for t in 1..n_frames {
        for e in 0..n_edges {
            if table[t * n_edges + e].is_nan() {
                table[t * n_edges + e] = table[(t - 1) * n_edges + e];
            }
        }
    }
    measured.sort_by(f32::total_cmp);
    let q = |p: f32| measured[((measured.len() - 1) as f32 * p).round() as usize];
    let quantiles = (!measured.is_empty()).then(|| [q(0.05), q(0.5), q(0.95)]);
    (table, quantiles)
}

/// Length-weighted mean width and standard error per edge; NaN widths are skipped.
pub(super) fn mean_widths(n_edges: usize, segments: &[WidthSegment]) -> Result<Vec<Option<(f32, f32)>>, String> {
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
    /// Width of edge `e` at timepoint `t` (per-timestep table), else its pooled width.
    pub fn width_at(&self, e: usize, t: Option<usize>) -> Option<f32> {
        match t.filter(|_| !self.width_t.is_empty()) {
            Some(t) => {
                let t = t.min(self.timepoints.len() - 1);
                Some(self.width_t[t * self.n_edges() + e]).filter(|w| w.is_finite())
            }
            None => self.width[e].map(|w| w.0),
        }
    }
}
