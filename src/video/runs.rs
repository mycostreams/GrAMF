//! Flow-video runs of a plate: `polyflow_analysis/<position>/<YYYYMMDD_HHMMSS>/` folders
//! (DESIGN.md §10, Inputs). Read-only; this module only lists and groups them.

use std::path::{Path, PathBuf};

/// The microscope that filmed the videos, from the camera serial. Mirrors
/// `pipeline-steps/pipeline_steps/optics.py` (the source of truth; keep in step).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Setup {
    Aretha,
    Morrison,
    Unknown(String),
}

impl Setup {
    pub fn from_camera(model: &str) -> Self {
        match model.split("SN:").nth(1).map(str::trim) {
            Some("25054443") => Setup::Aretha,   // Basler acA2040-90um, ARETHA videos
            Some("40018720") => Setup::Morrison, // Basler acA4112-30um, MORRISON videos
            _ => Setup::Unknown(model.to_string()),
        }
    }

    pub fn name(&self) -> String {
        match self {
            Setup::Aretha => "ARETHA".into(),
            Setup::Morrison => "MORRISON".into(),
            Setup::Unknown(model) if model.is_empty() => "unknown camera".into(),
            Setup::Unknown(model) => format!("unknown camera ({model})"),
        }
    }
}

/// One video run (one folder with metadata and a flow analysis).
#[derive(Clone, Debug)]
pub struct Run {
    /// `<position>/<timestamp>`, relative to `polyflow_analysis` (how the store names runs).
    pub rel_path: String,
    /// Acquisition time, epoch seconds.
    pub utc: i64,
    pub mode: String,
    pub run_nr: Option<i64>,
    /// Video scale, µm per pixel.
    pub delta_x_um: f32,
    pub frame_size: [u32; 2],
    pub camera: String,
    /// Stage position recorded in the metadata (mm).
    pub stage_mm: Option<[f64; 2]>,
}

/// A stage position and its runs, oldest first.
#[derive(Clone, Debug)]
pub struct Position {
    pub name: String,
    /// Stage position in mm, from the run metadata (the folder name as fallback).
    pub stage_mm: [f64; 2],
    pub runs: Vec<Run>,
}

pub struct Videos {
    pub root: PathBuf,
    pub positions: Vec<Position>,
    /// Majority setup over all runs.
    pub setup: Setup,
    /// Folders that looked like runs but could not be read, with the reason.
    pub skipped: Vec<String>,
}

/// Runs of the same session were filmed without moving the plate.
#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    /// Indices into `Videos::positions`, by first-run time.
    pub positions: Vec<usize>,
    pub start: i64,
    pub end: i64,
}

/// A run's extracted video graph: each edge's pixel trail in µm, relative to the frame centre,
/// in the camera's own axes (`placement::camera_orient` turns them into stage axes).
pub struct VideoGraph {
    pub edges: Vec<VideoEdge>,
}

pub struct VideoEdge {
    /// `(u, v, k)` in the video graph, as the flow analysis keys it.
    pub key: [i64; 3],
    pub points: Vec<[f32; 2]>,
}

/// Loads `extraction_graph.json` of `run` under `root`.
pub fn load_graph(root: &Path, run: &Run) -> Result<VideoGraph, String> {
    let dir = root.join(&run.rel_path);
    let file = std::fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.file_name().is_some_and(|n| n.to_string_lossy().ends_with("extraction_graph.json")))
        .ok_or("no extraction_graph.json")?;
    let g: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let centre = [run.frame_size[1] as f32 / 2.0, run.frame_size[0] as f32 / 2.0];
    let edges = g["edges"]
        .as_array()
        .ok_or("no edges")?
        .iter()
        .filter_map(|e| {
            let key = e["edge"].as_array()?;
            let points: Vec<[f32; 2]> = e["pixel_list"]
                .as_array()?
                .iter()
                .filter_map(|p| {
                    let (a, b) = (p[0].as_f64()? as f32, p[1].as_f64()? as f32);
                    Some([(a - centre[0]) * run.delta_x_um, (b - centre[1]) * run.delta_x_um])
                })
                .collect();
            let key = [key.first()?.as_i64()?, key.get(1)?.as_i64()?, key.get(2).and_then(|k| k.as_i64()).unwrap_or(0)];
            (points.len() >= 2).then_some(VideoEdge { key, points })
        })
        .collect();
    Ok(VideoGraph { edges })
}

/// `x_n10_38_y_4_484` -> (-10.38, 4.484): `n` is a minus sign, `_` the decimal point.
pub fn parse_position_name(name: &str) -> Option<[f64; 2]> {
    let rest = name.strip_prefix("x_")?;
    let (x, y) = rest.split_once("_y_")?;
    let num = |s: &str| -> Option<f64> {
        let (neg, s) = s.strip_prefix('n').map_or((false, s), |s| (true, s));
        let v: f64 = s.replacen('_', ".", 1).parse().ok()?;
        Some(if neg { -v } else { v })
    };
    Some([num(x)?, num(y)?])
}

/// Every run under `root` (a `polyflow_analysis` folder). A run folder needs a
/// `*metadata.json` and a `*flow_analysis.parquet`.
pub fn scan(root: &Path) -> Result<Videos, String> {
    let entries = std::fs::read_dir(root).map_err(|e| format!("{}: {e}", root.display()))?;
    let mut positions = vec![];
    let mut skipped = vec![];
    for entry in entries.flatten().filter(|e| e.path().is_dir()) {
        let name = entry.file_name().to_string_lossy().into_owned();
        let mut runs = vec![];
        for run_dir in std::fs::read_dir(entry.path()).into_iter().flatten().flatten().map(|e| e.path()) {
            let Some(files) = run_files(&run_dir) else {
                continue;
            };
            let rel = format!("{name}/{}", run_dir.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned()));
            match read_run(&files, rel.clone()) {
                Ok(run) => runs.push(run),
                Err(e) => skipped.push(format!("{rel}: {e}")),
            }
        }
        if runs.is_empty() {
            continue;
        }
        runs.sort_by_key(|r| r.utc);
        let stage_mm = runs[0].stage_mm.or_else(|| parse_position_name(&name));
        let Some(stage_mm) = stage_mm else {
            skipped.push(format!("{name}: no stage position"));
            continue;
        };
        positions.push(Position { name, stage_mm, runs });
    }
    positions.sort_by_key(|p| p.runs[0].utc);
    let setup = majority_setup(&positions);
    Ok(Videos { root: root.to_path_buf(), positions, setup, skipped })
}

/// `(metadata, parquet)` paths when `dir` is a run folder.
fn run_files(dir: &Path) -> Option<PathBuf> {
    let mut metadata = None;
    let mut parquet = false;
    for f in std::fs::read_dir(dir).ok()?.flatten() {
        let name = f.file_name().to_string_lossy().into_owned();
        if name.ends_with("metadata.json") {
            metadata = Some(f.path());
        }
        parquet |= name.ends_with("flow_analysis.parquet");
    }
    metadata.filter(|_| parquet)
}

fn read_run(metadata: &Path, rel_path: String) -> Result<Run, String> {
    let text = std::fs::read_to_string(metadata).map_err(|e| e.to_string())?;
    if text.is_empty() {
        return Err("empty metadata (online-only in Dropbox?)".into());
    }
    let m: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let date = m["date_time"].as_str().ok_or("no date_time")?;
    let utc = chrono::DateTime::parse_from_rfc3339(date)
        .or_else(|_| chrono::DateTime::parse_from_rfc3339(&format!("{date}Z")))
        .map_err(|e| format!("date_time {date:?}: {e}"))?
        .timestamp();
    let frame = &m["camera"]["frame_size"];
    Ok(Run {
        rel_path,
        utc,
        mode: m["mode"].as_str().unwrap_or("").to_string(),
        run_nr: m["run_nr"].as_i64(),
        delta_x_um: m["deltas"]["delta_x"].as_f64().ok_or("no deltas.delta_x")? as f32,
        frame_size: [frame[0].as_u64().unwrap_or(0) as u32, frame[1].as_u64().unwrap_or(0) as u32],
        camera: m["camera"]["model"].as_str().unwrap_or("").to_string(),
        stage_mm: m["position"]["x"].as_f64().zip(m["position"]["y"].as_f64()).map(|(x, y)| [x, y]),
    })
}

fn majority_setup(positions: &[Position]) -> Setup {
    let mut counts: Vec<(Setup, usize)> = vec![];
    for run in positions.iter().flat_map(|p| &p.runs) {
        let setup = Setup::from_camera(&run.camera);
        match counts.iter_mut().find(|(s, _)| *s == setup) {
            Some((_, n)) => *n += 1,
            None => counts.push((setup, 1)),
        }
    }
    counts.into_iter().max_by_key(|(_, n)| *n).map_or(Setup::Unknown(String::new()), |(s, _)| s)
}

/// Positions grouped into sessions: a new session starts when a position's first run is more
/// than `gap_minutes` after the previous one's. `positions` must be sorted by first run.
pub fn sessions(positions: &[Position], gap_minutes: f32) -> Vec<Session> {
    let gap = (gap_minutes * 60.0) as i64;
    let mut out: Vec<Session> = vec![];
    let mut previous_first: Option<i64> = None;
    for (i, p) in positions.iter().enumerate() {
        let first = p.runs[0].utc;
        let last = p.runs.last().map_or(first, |r| r.utc);
        match out.last_mut() {
            Some(s) if previous_first.is_some_and(|prev| first - prev <= gap) => {
                s.positions.push(i);
                s.end = s.end.max(last);
            }
            _ => out.push(Session { positions: vec![i], start: first, end: last }),
        }
        previous_first = Some(first);
    }
    out
}

/// The STG timepoint a run belongs to: the first at or after it, clamped to the last.
pub fn stg_frame(utc: i64, timepoints: &[i64]) -> Option<usize> {
    let last = timepoints.len().checked_sub(1)?;
    Some(timepoints.partition_point(|&t| t < utc).min(last))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn position(first: i64) -> Position {
        let run = Run {
            rel_path: String::new(),
            utc: first,
            mode: String::new(),
            run_nr: None,
            delta_x_um: 0.22,
            frame_size: [1024, 1024],
            camera: "Basler acA2040-90um SN:25054443".into(),
            stage_mm: None,
        };
        Position { name: String::new(), stage_mm: [0.0; 2], runs: vec![run] }
    }

    #[test]
    fn names_setups_frames() {
        assert_eq!(parse_position_name("x_n10_38_y_4_484"), Some([-10.38, 4.484]));
        assert_eq!(parse_position_name("x_3_y_n0_5"), Some([3.0, -0.5]));
        assert_eq!(parse_position_name("Img"), None);
        assert_eq!(Setup::from_camera("Basler acA2040-90um SN:25054443"), Setup::Aretha);
        assert_eq!(Setup::from_camera("Basler acA4112-30um SN:40018720"), Setup::Morrison);
        assert!(matches!(Setup::from_camera("Basler x"), Setup::Unknown(_)));
        assert_eq!(stg_frame(5, &[0, 10, 20]), Some(1));
        assert_eq!(stg_frame(10, &[0, 10, 20]), Some(1));
        assert_eq!(stg_frame(99, &[0, 10, 20]), Some(2));
        assert_eq!(stg_frame(1, &[]), None);
    }

    #[test]
    fn sessions_split_on_gaps() {
        // minutes 0, 5, 9 together; 60 starts a new session (15 min gap)
        let ps: Vec<Position> = [0, 300, 540, 3600].iter().map(|&t| position(t)).collect();
        let s = sessions(&ps, 15.0);
        assert_eq!(s.iter().map(|s| s.positions.clone()).collect::<Vec<_>>(), [vec![0, 1, 2], vec![3]]);
        assert_eq!(sessions(&ps, 120.0).len(), 1);
    }
}
