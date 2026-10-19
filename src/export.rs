//! Raw-table export (DESIGN.md §5): CSVs that join on `edge_id`, an optional GeoJSON of the
//! geometry, and a README saying where it all came from.

use crate::annotations::{Annotations, Target};
use crate::store::Store;
use crate::time::TimeAxis;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct Tables {
    pub edges: bool,
    pub nodes: bool,
    pub widths_t: bool,
    pub timepoints: bool,
    pub hyperedges: bool,
    pub geometry: bool,
}

impl Default for Tables {
    fn default() -> Self {
        Self {
            edges: true,
            nodes: true,
            widths_t: true,
            timepoints: true,
            hyperedges: true,
            geometry: false,
        }
    }
}

/// What to export and how.
pub struct Request<'a> {
    pub store: &'a Store,
    pub store_path: &'a Path,
    pub plate: &'a str,
    pub axis: Option<&'a TimeAxis>,
    /// Applied when present: flags, overrides and added edges come from this author.
    pub annotations: Option<&'a Annotations>,
    /// Pipeline edges in scope, and how the scope was chosen (for the README).
    pub edges: Vec<usize>,
    pub scope: String,
    /// Keep excluded / duplicate edges with a flag (true), or drop them.
    pub keep_excluded: bool,
    pub tables: Tables,
}

/// `,`-separated row; fields with `,` `"` or newlines are quoted.
fn row(fields: &[String]) -> String {
    let quote = |f: &String| {
        if f.contains([',', '"', '\n', '\r']) {
            format!("\"{}\"", f.replace('"', "\"\""))
        } else {
            f.clone()
        }
    };
    let mut line = fields.iter().map(quote).collect::<Vec<_>>().join(",");
    line.push('\n');
    line
}

fn num(x: f64) -> String {
    if x.is_finite() {
        format!("{x:.3}")
    } else {
        String::new()
    }
}

/// Writes the requested files into a new folder under `parent`; returns that folder.
pub fn export(parent: &Path, req: &Request) -> Result<PathBuf, String> {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let dir = parent.join(format!("gramf_export_{}_{stamp}", req.plate));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let write = |name: &str, body: String| {
        std::fs::write(dir.join(name), body).map_err(|e| format!("{name}: {e}"))
    };

    let files = build(req)?;
    for (name, body) in files {
        write(name, body)?;
    }
    Ok(dir)
}

/// The export as `(file name, contents)`, without touching the disk.
pub fn build(req: &Request) -> Result<Vec<(&'static str, String)>, String> {
    let s = req.store;
    let a = req.annotations;
    let empty = Annotations::default();
    let ann = a.unwrap_or(&empty);
    let flagged = |e: usize| ann.excluded.contains(&e) || ann.duplicate_of.contains_key(&e);
    let edges: Vec<usize> = req
        .edges
        .iter()
        .copied()
        .filter(|&e| req.keep_excluded || !flagged(e))
        .collect();
    let hyperedge = |e: usize| ann.hyperedge.get(&e).copied().unwrap_or(s.hyperedge[e]);

    // Added edges travel with the scope when both their nodes are endpoints of exported edges
    // (always, for the whole plate).
    let endpoints: BTreeSet<i64> = edges
        .iter()
        .filter_map(|&e| s.edge_nodes.get(e))
        .flatten()
        .copied()
        .collect();
    let whole = req.edges.len() == s.n_edges();
    let added: Vec<_> = ann
        .added
        .iter()
        .filter(|x| whole || (endpoints.contains(&x.u) && endpoints.contains(&x.v)))
        .copied()
        .collect();
    let node_pos = |id: i64| s.node_index.get(&id).map(|&n| s.node_pos[n]);

    let hours = |t: f32, since: Option<i64>| -> String {
        match (req.axis, since) {
            (Some(axis), Some(since)) if t.is_finite() && (t as usize) < axis.len() => {
                num((axis.timepoints[t as usize] - since) as f64 / 3600.0)
            }
            _ => String::new(),
        }
    };
    let inoc = req.axis.and_then(|x| x.inoculation);
    let cross = req.axis.map(|x| x.crossing.0);
    let mut files = vec![];

    if req.tables.edges {
        let mut out = row(&[
            "edge_id",
            "source",
            "u",
            "v",
            "length_um",
            "t_first",
            "t_first_h_inoc",
            "t_first_h_cross",
            "width_um",
            "width_se_um",
            "hyperedge",
            "x_mid_um",
            "y_mid_um",
            "excluded",
            "duplicate_of",
            "note",
        ]
        .map(String::from));
        for &e in &edges {
            let [u, v] = s
                .edge_nodes
                .get(e)
                .map_or([String::new(), String::new()], |n| n.map(|x| x.to_string()));
            let mid = s.arcs.points[(s.arcs.offsets[e] + s.arcs.offsets[e + 1]) / 2];
            let t = s.edge_t[e];
            let (w, se) = s.width[e].map_or((f64::NAN, f64::NAN), |(w, se)| (w as f64, se as f64));
            out += &row(&[
                e.to_string(),
                "pipeline".into(),
                u,
                v,
                num(s.edge_length_um[e] as f64),
                if t.is_finite() {
                    (t as i64).to_string()
                } else {
                    String::new()
                },
                hours(t, inoc),
                hours(t, cross),
                num(w),
                num(se),
                Some(hyperedge(e))
                    .filter(|h| *h >= 0)
                    .map_or(String::new(), |h| h.to_string()),
                num(mid[0] as f64),
                num(mid[1] as f64),
                (ann.excluded.contains(&e) as u8).to_string(),
                ann.duplicate_of
                    .get(&e)
                    .map_or(String::new(), usize::to_string),
                ann.notes.get(&Target::Edge(e)).cloned().unwrap_or_default(),
            ]);
        }
        for x in &added {
            let (pu, pv) = (node_pos(x.u), node_pos(x.v));
            let (len, mid) = match (pu, pv) {
                (Some(p), Some(q)) => (
                    (((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2)) as f64).sqrt(),
                    [
                        num(((p[0] + q[0]) / 2.0) as f64),
                        num(((p[1] + q[1]) / 2.0) as f64),
                    ],
                ),
                _ => (f64::NAN, [String::new(), String::new()]),
            };
            let [xm, ym] = mid;
            out += &row(&[
                x.id.to_string(),
                "added".into(),
                x.u.to_string(),
                x.v.to_string(),
                num(len),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                xm,
                ym,
                "0".into(),
                String::new(),
                String::new(),
            ]);
        }
        files.push(("edges.csv", out));
    }

    if req.tables.nodes {
        let mut degree: BTreeMap<i64, usize> = BTreeMap::new();
        for ends in edges
            .iter()
            .filter_map(|&e| s.edge_nodes.get(e))
            .chain(added.iter().map(|x| [x.u, x.v]).collect::<Vec<_>>().iter())
        {
            for id in ends {
                *degree.entry(*id).or_default() += 1;
            }
        }
        let mut out = row(&[
            "node_id", "x_um", "y_um", "degree", "is_root", "is_tip", "t_first", "note",
        ]
        .map(String::from));
        for (id, d) in &degree {
            let n = s.node_index.get(id);
            let [x, y] = n.map_or([f32::NAN; 2], |&n| s.node_pos[n]);
            let t = n.map_or(f32::NAN, |&n| s.node_t[n]);
            out += &row(&[
                id.to_string(),
                num(x as f64),
                num(y as f64),
                d.to_string(),
                (ann.roots.contains(id) as u8).to_string(),
                ((*d == 1) as u8).to_string(),
                if t.is_finite() {
                    (t as i64).to_string()
                } else {
                    String::new()
                },
                ann.notes
                    .get(&Target::Node(*id))
                    .cloned()
                    .unwrap_or_default(),
            ]);
        }
        files.push(("nodes.csv", out));
    }

    let mut skipped = vec![];
    if req.tables.widths_t {
        match crate::store::read_width_timesteps(req.store_path) {
            Err(e) => skipped.push(format!("edge_widths_t.csv ({e})")),
            Ok(rows) => {
                let keep: BTreeSet<usize> = edges.iter().copied().collect();
                let mut out =
                    row(&["edge_id", "timepoint_index", "width_um", "width_seg_um"]
                        .map(String::from));
                for r in rows
                    .iter()
                    .filter(|r| usize::try_from(r.edge_id).is_ok_and(|e| keep.contains(&e)))
                {
                    out += &row(&[
                        r.edge_id.to_string(),
                        r.timepoint_index.to_string(),
                        num(r.width_um),
                        num(r.width_seg_um),
                    ]);
                }
                files.push(("edge_widths_t.csv", out));
            }
        }
    }

    if req.tables.timepoints && req.axis.is_none() {
        skipped.push("timepoints.csv (the store has no /timepoints)".into());
    }
    if let (true, Some(axis)) = (req.tables.timepoints, req.axis) {
        let mut out =
            row(&["index", "utc", "h_inoc", "h_cross", "quality_weight"].map(String::from));
        for i in 0..axis.len() {
            out += &row(&[
                i.to_string(),
                axis.label(i, crate::time::TimeUnit::Utc),
                hours(i as f32, inoc),
                hours(i as f32, cross),
                num(axis.quality[i]),
            ]);
        }
        files.push(("timepoints.csv", out));
    }

    if req.tables.hyperedges {
        let mut by_h: BTreeMap<i64, (usize, f64, f32)> = BTreeMap::new();
        for &e in edges.iter().filter(|&&e| hyperedge(e) >= 0) {
            let entry = by_h.entry(hyperedge(e)).or_insert((0, 0.0, f32::INFINITY));
            *entry = (
                entry.0 + 1,
                entry.1 + s.edge_length_um[e] as f64,
                entry.2.min(s.edge_t[e]),
            );
        }
        let mut out =
            row(&["hyperedge", "n_edges", "total_length_um", "t_first_min"].map(String::from));
        for (h, (n, len, t)) in by_h {
            let t = if t.is_finite() {
                (t as i64).to_string()
            } else {
                String::new()
            };
            out += &row(&[h.to_string(), n.to_string(), num(len), t]);
        }
        files.push(("hyperedges.csv", out));
    }

    if req.tables.geometry {
        let mut out = String::from("{\"type\":\"FeatureCollection\",\"features\":[\n");
        let mut first = true;
        let mut feature = |id: i64, source: &str, pts: &mut dyn Iterator<Item = [f32; 2]>| {
            let coords: Vec<String> = pts.map(|p| format!("[{:.2},{:.2}]", p[0], p[1])).collect();
            let _ = write!(
                out,
                "{}{{\"type\":\"Feature\",\"properties\":{{\"edge_id\":{id},\"source\":\"{source}\"}},\"geometry\":{{\"type\":\"LineString\",\"coordinates\":[{}]}}}}",
                if first { "" } else { ",\n" },
                coords.join(",")
            );
            first = false;
        };
        for &e in &edges {
            feature(
                e as i64,
                "pipeline",
                &mut s.arcs.points[s.arcs.offsets[e]..s.arcs.offsets[e + 1]]
                    .iter()
                    .copied(),
            );
        }
        for x in &added {
            if let (Some(p), Some(q)) = (node_pos(x.u), node_pos(x.v)) {
                feature(x.id, "added", &mut [p, q].into_iter());
            }
        }
        out.push_str("\n]}\n");
        files.push(("geometry.geojson", out));
    }

    files.push((
        "README.txt",
        readme(req, edges.len(), added.len(), &files, &skipped),
    ));
    Ok(files)
}

fn readme(
    req: &Request,
    n_edges: usize,
    n_added: usize,
    files: &[(&str, String)],
    skipped: &[String],
) -> String {
    let a = req.annotations;
    let author = match a {
        Some(a) if a.dirty => format!(
            "{} (including edits not yet saved to annotations.{}.h5)",
            a.author, a.author
        ),
        Some(a) => format!("{} (annotations.{}.h5)", a.author, a.author),
        None => "none".into(),
    };
    let names: Vec<&str> = files.iter().map(|f| f.0).collect();
    format!(
        "GrAMF export\n\
         ============\n\
         exported      {now}\n\
         GrAMF         {version}\n\
         store         {store}\n\
         plate         {plate}\n\
         fingerprint   {fp}\n\
         annotations   {author}\n\
         scope         {scope}\n\
         flagged edges {flagged}\n\
         rows          {n_edges} pipeline edges, {n_added} added edges\n\
         files         {names}\n\
         skipped       {skipped}\n\
         \n\
         Units: lengths and coordinates in micrometres; x = image column, y = image row.\n\
         Times: t_first is a timepoint index into timepoints.csv; *_h_inoc / *_h_cross are\n\
         hours since inoculation (registry `Start`) / since crossing (registry `Crossed`, else\n\
         the store's first timepoint). Empty cells mean unknown or not measured.\n\
         \n\
         edges.csv joins every other table on edge_id. source = pipeline | added (drawn in GrAMF,\n\
         ids from 1000000, no time or width). excluded / duplicate_of / note and hyperedge\n\
         overrides come from the annotations named above; consumers should drop rows with\n\
         excluded = 1 or a duplicate_of value.\n\
         nodes.csv: degree counts the exported edges; is_tip = degree 1.\n\
         edge_widths_t.csv: per (edge, timepoint) widths from /edge_width_timesteps.\n",
        now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S %Z"),
        version = env!("CARGO_PKG_VERSION"),
        store = req.store_path.display(),
        plate = req.plate,
        fp = req.store.fingerprint,
        scope = req.scope,
        flagged = if req.keep_excluded {
            "kept, flagged"
        } else {
            "dropped"
        },
        names = names.join(", "),
        skipped = if skipped.is_empty() {
            "none".into()
        } else {
            skipped.join("; ")
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotations::{AddedEdge, Op};
    use crate::store::Arcs;

    fn tiny_store() -> Store {
        // two edges: 0 = nodes 10-11, 1 = nodes 11-12
        let mut s = Store {
            arcs: Arcs {
                offsets: vec![0, 2, 4],
                points: vec![[0.0, 0.0], [3.0, 4.0], [3.0, 4.0], [6.0, 8.0]],
                min: [0.0; 2],
                max: [6.0, 8.0],
            },
            vertex_t: vec![1.0, 1.0, 2.0, 2.0],
            node_ids: vec![10, 11, 12],
            node_pos: vec![[0.0, 0.0], [3.0, 4.0], [6.0, 8.0]],
            edge_nodes: vec![[10, 11], [11, 12]],
            edge_length_um: vec![5.0, 5.0],
            edge_t: vec![1.0, 2.0],
            hyperedge: vec![7, -1],
            width: vec![Some((2.0, 0.1)), None],
            node_t: vec![1.0, 1.0, 2.0],
            ..Default::default()
        };
        s.node_index = s
            .node_ids
            .iter()
            .enumerate()
            .map(|(i, &id)| (id, i))
            .collect();
        s
    }

    #[test]
    fn tables_apply_annotations() {
        let store = tiny_store();
        let mut a = Annotations::new("simon", "");
        a.edit(Op::SetExcluded(1, true));
        a.edit(Op::SetHyperedge(1, Some(9)));
        a.edit(Op::AddEdge(AddedEdge {
            id: 1_000_000,
            u: 10,
            v: 12,
        }));
        a.edit(Op::SetNote(
            Target::Edge(0),
            Some("a, \"quoted\" note".into()),
        ));
        let tables = Tables {
            widths_t: false,
            timepoints: false,
            geometry: true,
            ..Default::default()
        };
        let req = |keep| Request {
            store: &store,
            store_path: Path::new("/x/stg_store.h5"),
            plate: "TST0000A001",
            axis: None,
            annotations: Some(&a),
            edges: vec![0, 1],
            scope: "whole plate".into(),
            keep_excluded: keep,
            tables: tables.clone(),
        };

        let files = build(&req(true)).unwrap();
        let get =
            |files: &[(&str, String)], name| files.iter().find(|f| f.0 == name).unwrap().1.clone();
        let edges = get(&files, "edges.csv");
        let lines: Vec<&str> = edges.lines().collect();
        assert_eq!(lines.len(), 4); // header, 2 pipeline, 1 added
        assert_eq!(
            lines[1],
            "0,pipeline,10,11,5.000,1,,,2.000,0.100,7,3.000,4.000,0,,\"a, \"\"quoted\"\" note\""
        );
        assert!(lines[2].starts_with("1,pipeline,11,12,5.000,2,,,,,9,"));
        assert!(lines[2].contains(",1,,")); // excluded flag kept
        assert!(lines[3].starts_with("1000000,added,10,12,10.000,"));
        // node 11 has degree 2 (edges 0 and 1); 10 and 12 also touch the added edge
        assert!(get(&files, "nodes.csv").contains("\n11,3.000,4.000,2,0,0,1,\n"));
        assert!(get(&files, "geometry.geojson").contains("\"edge_id\":1000000"));

        let dropped = get(&build(&req(false)).unwrap(), "edges.csv");
        assert!(!dropped.lines().any(|l| l.starts_with("1,pipeline")));
        let hyper = get(&build(&req(false)).unwrap(), "hyperedges.csv");
        assert_eq!(hyper.lines().nth(1), Some("7,1,5.000,1"));
    }
}
