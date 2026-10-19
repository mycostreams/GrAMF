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

pub fn read_arcs(path: &Path) -> Result<Arcs, String> {
    let file = hdf5_metno::File::open(path).map_err(|e| e.to_string())?;
    let read = |name: &str| {
        file.dataset(name)
            .map_err(|_| format!("{name} missing — not an STG skeleton store"))
    };
    let offsets = read("arcs_offsets")?
        .read_raw::<i64>()
        .map_err(|e| e.to_string())?;
    let points = read("arcs_points")?
        .read_raw::<f64>()
        .map_err(|e| e.to_string())?;
    arcs_from_raw(&offsets, &points)
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
    use super::arcs_from_raw;

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
