//! Flow values of a video run, from its `flow_analysis.parquet` (DESIGN.md §10, Inputs).

use parquet::file::reader::{FileReader, SerializedFileReader};
use parquet::record::RowAccessor;
use std::collections::BTreeMap;
use std::path::Path;

/// The seven per-edge, per-direction values, in STHype's `_MEASUREMENT_VALUES` order.
pub const VALUE_NAMES: [&str; 7] = ["median", "perc_25", "perc_75", "flux_ratio", "coverage", "pixel_count", "flux"];

/// Flow along one direction of one video edge.
#[derive(Clone, Debug, PartialEq)]
pub struct DirectionFlow {
    /// The direction as `(u, v, k)`: `== edge` means along the edge's own `(u, v)`.
    pub dir: [i64; 3],
    /// `VALUE_NAMES` order; NaN when missing.
    pub values: [f64; 7],
}

/// Video edge `(u, v, k)` -> its directions.
pub type RunFlow = BTreeMap<[i64; 3], Vec<DirectionFlow>>;

/// Reads `*flow_analysis.parquet` in `run_dir`. The `edge_*` columns are constant per edge and
/// direction, so the first row of each is taken. Pre-fix files are repaired as the registrator
/// does: inverted quartiles are swapped, and a NaN `edge_flux_ratio` becomes the mean of the
/// per-frame `flux_ratio`.
pub fn read(run_dir: &Path) -> Result<RunFlow, String> {
    let path = std::fs::read_dir(run_dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.file_name().is_some_and(|n| n.to_string_lossy().ends_with("flow_analysis.parquet")))
        .ok_or("no flow_analysis.parquet")?;
    let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
    let reader = SerializedFileReader::new(file).map_err(|e| e.to_string())?;
    let schema = reader.metadata().file_metadata().schema_descr();
    let col = |name: &str| (0..schema.num_columns()).find(|&i| schema.column(i).name() == name).ok_or(format!("no column {name}"));
    let ints = ["edge_u", "edge_v", "edge_k", "dir_u", "dir_v", "dir_k"].map(col);
    let ints: Vec<usize> = ints.into_iter().collect::<Result<_, _>>()?;
    let vals: Vec<usize> = VALUE_NAMES.map(|n| col(&format!("edge_{n}"))).into_iter().collect::<Result<_, _>>()?;
    let per_frame_ratio = col("flux_ratio").ok();

    let mut out: RunFlow = BTreeMap::new();
    // per (edge, dir): running mean of per-frame flux_ratio, for the repair
    let mut ratio_sums: BTreeMap<([i64; 3], [i64; 3]), (f64, usize)> = BTreeMap::new();
    for row in reader.get_row_iter(None).map_err(|e| e.to_string())? {
        let row = row.map_err(|e| e.to_string())?;
        let int = |i: usize| row.get_int(ints[i]).map(i64::from).unwrap_or(-1);
        let edge = [int(0), int(1), int(2)];
        let dir = [int(3), int(4), int(5)];
        if let Some(c) = per_frame_ratio {
            if let Ok(r) = row.get_float(c) {
                if r.is_finite() {
                    let e = ratio_sums.entry((edge, dir)).or_default();
                    *e = (e.0 + r as f64, e.1 + 1);
                }
            }
        }
        let dirs = out.entry(edge).or_default();
        if dirs.iter().any(|d| d.dir == dir) {
            continue;
        }
        let mut values = [f64::NAN; 7];
        for (k, &c) in vals.iter().enumerate() {
            values[k] = row.get_float(c).map_or(f64::NAN, f64::from);
        }
        dirs.push(DirectionFlow { dir, values });
    }
    for (edge, dirs) in &mut out {
        for d in dirs.iter_mut() {
            if d.values[1] > d.values[2] {
                d.values.swap(1, 2);
            }
            if d.values[3].is_nan() {
                if let Some((sum, n)) = ratio_sums.get(&(*edge, d.dir)).filter(|x| x.1 > 0) {
                    d.values[3] = sum / *n as f64;
                }
            }
        }
    }
    Ok(out)
}
