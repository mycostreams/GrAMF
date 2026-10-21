//! Publish to store (DESIGN.md §10, a temporary bridge): the one place GrAMF writes a store.

use super::file::{H5, vlu};
use super::*;

/// What Publish to store wrote, for the status line.
#[derive(Debug, PartialEq)]
pub struct Published {
    pub measurements: usize,
    /// Measurements on added edges: the store has no `edge_id` for them, so they stay in
    /// the annotation file only.
    pub on_added_edges: usize,
    pub trails: usize,
    pub roots: usize,
}

/// Publish to store (DESIGN.md §10, a temporary bridge): writes the flow layer, trails and
/// roots into `store` in STHype's format (`write_flow_mappings`, `write_root_nodes`),
/// replacing what is there. The one place GrAMF writes a store, so it refuses a store whose
/// backbone changed, and works on a copy renamed over the original only when complete.
pub fn publish(a: &Annotations, store: &Path, n_edges: usize, trail_paths: &BTreeMap<String, Vec<i64>>) -> Result<Published, String> {
    let flow = a.flow.as_ref().ok_or("no flow layer: verify a session and save first")?;
    let tmp = store.with_extension("h5.publishing");
    std::fs::copy(store, &tmp).map_err(|e| e.to_string())?;
    let result = publish_into(a, flow, &tmp, n_edges, trail_paths).map_err(|e| e.to_string());
    match result {
        Ok(Ok(p)) => std::fs::rename(&tmp, store).map(|_| p).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            e.to_string()
        }),
        Ok(Err(e)) | Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

fn publish_into(a: &Annotations, flow: &FlowOutput, path: &Path, n_edges: usize, trail_paths: &BTreeMap<String, Vec<i64>>) -> hdf5_metno::Result<Result<Published, String>> {
    let f = hdf5_metno::File::append(path)?;
    let fingerprint = f.attr("backbone_fingerprint")?.read_scalar::<VarLenUnicode>()?.to_string();
    if fingerprint != a.fingerprint {
        return Ok(Err("the store's backbone changed since these annotations were made".into()));
    }
    for name in ["flow_runs", "flow_measurements", "flow_trails_names", "flow_trails_nodes", "flow_trails_offsets", "root_nodes"] {
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

    let roots: Vec<i64> = a.roots.iter().copied().collect();
    let d = f.new_dataset_builder().deflate(4).with_data(&roots[..]).create("root_nodes")?;
    attr_i(&d, "schema_version", 1)?;
    attr_s(&d, "backbone_fingerprint", &fingerprint)?;
    Ok(Ok(Published {
        measurements: measurements.len(),
        on_added_edges: flow.measurements.len() - measurements.len(),
        trails: names.len(),
        roots: roots.len(),
    }))
}
