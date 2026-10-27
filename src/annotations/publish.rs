//! Publish to store (DESIGN.md §10, a temporary bridge): the one place GrAMF writes a store.

use super::file::{H5, vlu};
use super::*;
use std::collections::HashMap;

/// What Publish to store wrote, for the status line.
#[derive(Debug, PartialEq)]
pub struct Published {
    pub measurements: usize,
    /// Measurements on added edges: the store has no `edge_id` for them, so they stay in
    /// the annotation file only.
    pub on_added_edges: usize,
    pub trails: usize,
    pub roots: usize,
    /// Added edges written to `/added_edges`, so trails that cross them stay walkable.
    pub added_edges: usize,
    /// The store as it was, kept beside it.
    pub backup: std::path::PathBuf,
}

/// Publish to store (DESIGN.md §10, a temporary bridge): writes the flow layer, trails and
/// roots into `store` in STHype's format (`write_flow_mappings`, `write_root_nodes`),
/// replacing what is there. The one place GrAMF writes a store, so it refuses a store whose
/// backbone changed, and works on a copy renamed over the original only when complete.
pub fn publish(a: &Annotations, store: &Path, n_edges: usize, trail_paths: &BTreeMap<String, Vec<i64>>) -> Result<Published, String> {
    let stamp = chrono::Local::now().format("%Y-%m-%d-%H%M%S");
    publish_with_backup(a, store, n_edges, trail_paths, &format!("bak-before-gramf-publish-{stamp}"))
}

/// [`publish`], keeping the store as it was as `<store>.<suffix>`: the original is renamed
/// to the backup and the completed copy renamed into its place, so nothing is ever lost.
pub(super) fn publish_with_backup(a: &Annotations, store: &Path, n_edges: usize, trail_paths: &BTreeMap<String, Vec<i64>>, suffix: &str) -> Result<Published, String> {
    let flow = a.flow.as_ref().ok_or("no flow layer: verify a session and save first")?;
    let mut backup = store.as_os_str().to_owned();
    backup.push(format!(".{suffix}"));
    let backup = std::path::PathBuf::from(backup);
    if backup.exists() {
        return Err(format!("{} exists already", backup.display()));
    }
    let tmp = store.with_extension("h5.publishing");
    std::fs::copy(store, &tmp).map_err(|e| e.to_string())?;
    let written = match publish_into(a, flow, &tmp, n_edges, trail_paths) {
        Ok(Ok(p)) => p,
        Ok(Err(e)) => return Err(cleanup(&tmp, e)),
        Err(e) => return Err(cleanup(&tmp, e.to_string())),
    };
    std::fs::rename(store, &backup).map_err(|e| cleanup(&tmp, e.to_string()))?;
    if let Err(e) = std::fs::rename(&tmp, store) {
        let _ = std::fs::rename(&backup, store); // put the original back
        return Err(cleanup(&tmp, e.to_string()));
    }
    Ok(Published { backup, ..written })
}

fn cleanup(tmp: &Path, e: String) -> String {
    let _ = std::fs::remove_file(tmp);
    e
}

fn publish_into(a: &Annotations, flow: &FlowOutput, path: &Path, n_edges: usize, trail_paths: &BTreeMap<String, Vec<i64>>) -> hdf5_metno::Result<Result<Published, String>> {
    let f = hdf5_metno::File::append(path)?;
    let fingerprint = f.attr("backbone_fingerprint")?.read_scalar::<VarLenUnicode>()?.to_string();
    if fingerprint != a.fingerprint {
        return Ok(Err("the store's backbone changed since these annotations were made".into()));
    }
    for name in ["flow_runs", "flow_measurements", "flow_trails_names", "flow_trails_nodes", "flow_trails_offsets", "root_nodes", "added_edges"] {
        if f.link_exists(name) {
            f.unlink(name)?;
        }
    }
    let attr_i = |d: &hdf5_metno::Dataset, name: &str, v: i64| -> H5 { d.new_attr::<i64>().shape(()).create(name)?.as_writer().write_scalar(&v) };
    let attr_s = |d: &hdf5_metno::Dataset, name: &str, v: &str| -> H5 { d.new_attr::<VarLenUnicode>().shape(()).create(name)?.as_writer().write_scalar(&vlu(v)) };

    let measurements: Vec<FlowMeasurementRow> = flow.measurements.iter().filter(|m| (m.edge_id as usize) < n_edges).copied().collect();
    let runs = f.new_dataset_builder().deflate(4).with_data(&flow.runs[..]).create("flow_runs")?;
    attr_i(&runs, "schema_version", 1)?;
    attr_s(&runs, "backbone_fingerprint", &fingerprint)?;
    let m = f.new_dataset_builder().deflate(4).with_data(&measurements[..]).create("flow_measurements")?;
    attr_i(&m, "schema_version", 1)?;
    attr_s(&m, "backbone_fingerprint", &fingerprint)?;
    m.new_attr::<f64>().shape(()).create("cutoff_um")?.as_writer().write_scalar(&(flow.cutoff_um as f64))?;
    attr_i(&m, "n_anchors", flow.n_anchors)?;
    attr_i(&m, "n_pins", flow.n_pins)?;
    m.new_attr::<f64>().shape(()).create("video_scale")?.as_writer().write_scalar(&(flow.video_scale as f64))?;
    attr_s(&m, "written_by", "GrAMF")?;

    let names: Vec<VarLenUnicode> = a.trails.keys().map(|n| vlu(n)).collect();
    let (mut offsets, mut nodes) = (vec![0i64], vec![]);
    for n in a.trails.keys() {
        nodes.extend(trail_paths.get(n).cloned().unwrap_or_default());
        offsets.push(nodes.len() as i64);
    }
    let d = f.new_dataset_builder().with_data(&names[..]).create("flow_trails_names")?;
    attr_i(&d, "schema_version", 1)?;
    f.new_dataset_builder().with_data(&nodes[..]).create("flow_trails_nodes")?;
    f.new_dataset_builder().with_data(&offsets[..]).create("flow_trails_offsets")?;

    // The user's added edges, so a trail drawn across one is a path of edges downstream too.
    // Written only when there are any: a store without them reads exactly as before.
    let added = added_edge_rows(&f, &a.added)?;
    if !added.is_empty() {
        let d = f.new_dataset_builder().with_data(&added[..]).create("added_edges")?;
        attr_i(&d, "schema_version", 1)?;
        attr_s(&d, "backbone_fingerprint", &fingerprint)?;
        attr_s(&d, "written_by", "GrAMF")?;
        attr_s(&d, "definition", ADDED_EDGES_DEFINITION)?;
    }

    let roots: Vec<i64> = a.roots.iter().copied().collect();
    let d = f.new_dataset_builder().deflate(4).with_data(&roots[..]).create("root_nodes")?;
    attr_i(&d, "schema_version", 1)?;
    attr_s(&d, "backbone_fingerprint", &fingerprint)?;
    Ok(Ok(Published {
        measurements: measurements.len(),
        on_added_edges: flow.measurements.len() - measurements.len(),
        trails: names.len(),
        roots: roots.len(),
        added_edges: added.len(),
        backup: Default::default(),
    }))
}

const ADDED_EDGES_DEFINITION: &str = "Edges a user added in GrAMF where the traced network has a gap. Not part of /edges: \
    the backbone and its fingerprint are unchanged. u, v: store node ids; length_um: straight-line distance between \
    them (STG um). t_first: the minimum /t_first over the store edges sharing either endpoint. width_um: the mean over \
    those edges of their length-weighted /edge_width_segments width_um (NaN when none has one). No flow is measured on \
    them. Written by GrAMF Publish to store.";

/// One `/added_edges` row.
#[derive(hdf5_metno::H5Type, Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct AddedEdgeRow {
    pub u: i64,
    pub v: i64,
    pub length_um: f64,
    pub t_first: i64,
    pub width_um: f64,
}

/// The `/edge_width_segments` columns read here (HDF5 converts compound members by name).
#[derive(hdf5_metno::H5Type, Clone, Copy)]
#[repr(C)]
struct WidthSeg {
    edge_id: i64,
    s_start_um: f64,
    s_end_um: f64,
    width_um: f64,
}

/// `/added_edges` rows for `added`, from the store's own `/nodes`, `/edges`, `/t_first` and
/// `/edge_width_segments`. An added edge whose end is not a store node is left out.
fn added_edge_rows(f: &hdf5_metno::File, added: &[AddedEdge]) -> hdf5_metno::Result<Vec<AddedEdgeRow>> {
    if added.is_empty() {
        return Ok(vec![]);
    }
    let nodes = f.dataset("nodes")?.read_raw::<f64>()?;
    let pos: HashMap<i64, [f64; 2]> = nodes.chunks_exact(3).map(|n| (n[0] as i64, [n[1], n[2]])).collect();
    let edges: Vec<[i64; 2]> = f.dataset("edges")?.read_raw::<i64>()?.chunks_exact(2).map(|e| [e[0], e[1]]).collect();
    let t_first = f.dataset("t_first").and_then(|d| d.read_raw::<i64>()).unwrap_or_default();
    let segs = f.dataset("edge_width_segments").and_then(|d| d.read_raw::<WidthSeg>()).unwrap_or_default();
    Ok(added_rows(added, &pos, &edges, &t_first, &segs.iter().map(|s| (s.edge_id, s.s_start_um, s.s_end_um, s.width_um)).collect::<Vec<_>>()))
}

/// The rule behind [`added_edge_rows`] (Sajitha, 27 Oct 2026): an added edge takes the earliest
/// `t_first` and the mean width of the store edges that share either of its ends.
fn added_rows(added: &[AddedEdge], pos: &HashMap<i64, [f64; 2]>, edges: &[[i64; 2]], t_first: &[i64], segs: &[(i64, f64, f64, f64)]) -> Vec<AddedEdgeRow> {
    let mut acc: HashMap<i64, (f64, f64)> = HashMap::new(); // edge -> (sum width*len, sum len)
    for &(e, s0, s1, w) in segs {
        let l = s1 - s0;
        if l > 0.0 && w.is_finite() && w > 0.0 {
            let a = acc.entry(e).or_default();
            a.0 += w * l;
            a.1 += l;
        }
    }
    let width = |e: usize| acc.get(&(e as i64)).filter(|a| a.1 > 0.0).map(|a| a.0 / a.1);
    added
        .iter()
        .filter_map(|ae| {
            let (pu, pv) = (pos.get(&ae.u)?, pos.get(&ae.v)?);
            let adjacent: Vec<usize> =
                (0..edges.len()).filter(|&i| [ae.u, ae.v].iter().any(|n| edges[i].contains(n))).collect();
            let t = adjacent.iter().filter_map(|&i| t_first.get(i).copied()).min().unwrap_or(0);
            let ws: Vec<f64> = adjacent.iter().filter_map(|&i| width(i)).collect();
            Some(AddedEdgeRow {
                u: ae.u,
                v: ae.v,
                length_um: (pu[0] - pv[0]).hypot(pu[1] - pv[1]),
                t_first: t,
                width_um: if ws.is_empty() { f64::NAN } else { ws.iter().sum::<f64>() / ws.len() as f64 },
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn added_edge_takes_earliest_neighbour_date_and_mean_width() {
        // nodes 1-2-3 in a row and 4 off to the side; edges 0: 1-2, 1: 2-3, 2: 3-4; added 1-4
        let pos = HashMap::from([(1, [0.0, 0.0]), (2, [10.0, 0.0]), (3, [20.0, 0.0]), (4, [3.0, 4.0])]);
        let edges = [[1, 2], [2, 3], [3, 4]];
        let t_first = [7, 3, 5];
        // edge 0: 8 um over its whole length; edge 2: 4 um over 1 um and 10 um over 3 um -> 8.5
        let segs = [(0, 0.0, 10.0, 8.0), (2, 0.0, 1.0, 4.0), (2, 1.0, 4.0, 10.0)];
        let rows = added_rows(&[AddedEdge { id: 1_000_000, u: 1, v: 4 }], &pos, &edges, &t_first, &segs);
        // adjacent to node 1: edge 0; to node 4: edge 2 -> t_first min(7, 5), width mean(8, 8.5)
        assert_eq!(rows, vec![AddedEdgeRow { u: 1, v: 4, length_um: 5.0, t_first: 5, width_um: 8.25 }]);
        // an end that is not a store node is left out
        assert!(added_rows(&[AddedEdge { id: 1, u: 1, v: 99 }], &pos, &edges, &t_first, &segs).is_empty());
    }
}
