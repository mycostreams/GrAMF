//! Reading and writing `annotations.<author>.h5`: atomic save, and load of every optional group.

use super::*;

pub(super) type H5 = hdf5_metno::Result<()>;

pub(super) fn vlu(s: &str) -> VarLenUnicode {
    // ponytail: interior NULs are the only failure; strip them rather than error
    VarLenUnicode::from_str(&s.replace('\0', "")).unwrap_or_default()
}

/// Writes `a` to `path` atomically: a `.tmp` sibling, then a rename over the target, so a
/// crash or a Dropbox sync never sees a half-written HDF5 file. `trail_paths` holds each
/// trail's routed node path, written for readers that cannot route themselves.
pub fn save(a: &Annotations, path: &Path, trail_paths: &BTreeMap<String, Vec<i64>>) -> Result<(), String> {
    let tmp = path.with_extension("h5.tmp");
    write_file(a, &tmp, trail_paths).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

fn write_file(a: &Annotations, path: &Path, trail_paths: &BTreeMap<String, Vec<i64>>) -> H5 {
    let f = hdf5_metno::File::create(path)?;
    let attr_str = |name: &str, value: &str| -> H5 {
        f.new_attr::<VarLenUnicode>().shape(()).create(name)?.as_writer().write_scalar(&vlu(value))
    };
    attr_str("author", &a.author)?;
    attr_str("backbone_fingerprint", &a.fingerprint)?;
    attr_str("gramf_version", env!("CARGO_PKG_VERSION"))?;
    f.new_attr::<i64>().shape(()).create("annotation_schema_version")?.as_writer().write_scalar(&SCHEMA_VERSION)?;

    let ints = |g: &hdf5_metno::Group, name: &str, v: &[i64]| -> H5 {
        g.new_dataset_builder().with_data(v).create(name).map(drop)
    };
    let strs = |g: &hdf5_metno::Group, name: &str, v: &[String]| -> H5 {
        let v: Vec<VarLenUnicode> = v.iter().map(|s| vlu(s)).collect();
        g.new_dataset_builder().with_data(&v[..]).create(name).map(drop)
    };
    let as_i64 = |it: &mut dyn Iterator<Item = usize>| -> Vec<i64> { it.map(|x| x as i64).collect() };

    let g = f.create_group("excluded")?;
    ints(&g, "edge_id", &as_i64(&mut a.excluded.iter().copied()))?;

    let g = f.create_group("duplicate_of")?;
    ints(&g, "edge_id", &as_i64(&mut a.duplicate_of.keys().copied()))?;
    ints(&g, "of_edge_id", &as_i64(&mut a.duplicate_of.values().copied()))?;

    let g = f.create_group("hyperedge_overrides")?;
    ints(&g, "edge_id", &as_i64(&mut a.hyperedge.keys().copied()))?;
    ints(&g, "hyperedge", &a.hyperedge.values().copied().collect::<Vec<_>>())?;

    let g = f.create_group("roots")?;
    ints(&g, "node_id", &a.roots.iter().copied().collect::<Vec<_>>())?;

    let g = f.create_group("added_edges")?;
    ints(&g, "id", &a.added.iter().map(|e| e.id).collect::<Vec<_>>())?;
    ints(&g, "u", &a.added.iter().map(|e| e.u).collect::<Vec<_>>())?;
    ints(&g, "v", &a.added.iter().map(|e| e.v).collect::<Vec<_>>())?;

    let g = f.create_group("notes")?;
    let (kind, id): (Vec<String>, Vec<i64>) = a
        .notes
        .keys()
        .map(|t| match t {
            Target::Edge(e) => ("edge".to_string(), *e as i64),
            Target::Node(n) => ("node".to_string(), *n),
        })
        .unzip();
    strs(&g, "kind", &kind)?;
    ints(&g, "id", &id)?;
    strs(&g, "text", &a.notes.values().cloned().collect::<Vec<_>>())?;

    // Trails: names, waypoints and routed node paths, both CSR (offsets has n + 1 entries).
    let g = f.create_group("trails")?;
    let csr = |lists: &mut dyn Iterator<Item = Vec<i64>>| -> (Vec<i64>, Vec<i64>) {
        let (mut offsets, mut values) = (vec![0], vec![]);
        for l in lists {
            values.extend(l);
            offsets.push(values.len() as i64);
        }
        (offsets, values)
    };
    strs(&g, "name", &a.trails.keys().cloned().collect::<Vec<_>>())?;
    let (wo, wv) = csr(&mut a.trails.values().cloned());
    ints(&g, "waypoint_offsets", &wo)?;
    ints(&g, "waypoints", &wv)?;
    let (no, nv) = csr(&mut a.trails.keys().map(|n| trail_paths.get(n).cloned().unwrap_or_default()));
    ints(&g, "node_offsets", &no)?;
    ints(&g, "nodes", &nv)?;

    write_registration(&f, &a.registration, &ints, &strs)?;
    if let Some(flow) = &a.flow {
        f.new_dataset_builder().with_data(&flow.runs[..]).create("flow_runs")?;
        let m = f.new_dataset_builder().with_data(&flow.measurements[..]).create("flow_measurements")?;
        let attr_i = |name: &str, v: i64| -> H5 { m.new_attr::<i64>().shape(()).create(name)?.as_writer().write_scalar(&v) };
        attr_i("schema_version", 1)?;
        attr_i("n_anchors", flow.n_anchors)?;
        attr_i("n_pins", flow.n_pins)?;
        m.new_attr::<f64>().shape(()).create("cutoff_um")?.as_writer().write_scalar(&(flow.cutoff_um as f64))?;
        m.new_attr::<f64>().shape(()).create("video_scale")?.as_writer().write_scalar(&(flow.video_scale as f64))?;
        for (name, value) in [("backbone_fingerprint", a.fingerprint.as_str()), ("written_by", "GrAMF")] {
            m.new_attr::<VarLenUnicode>().shape(()).create(name)?.as_writer().write_scalar(&vlu(value))?;
        }
    }

    let g = f.create_group("history")?;
    ints(&g, "utc", &a.history.iter().map(|h| h.utc).collect::<Vec<_>>())?;
    strs(&g, "author", &a.history.iter().map(|h| h.author.clone()).collect::<Vec<_>>())?;
    strs(&g, "op", &a.history.iter().map(|h| h.op.clone()).collect::<Vec<_>>())?;
    Ok(())
}

type Ints<'a> = dyn Fn(&hdf5_metno::Group, &str, &[i64]) -> H5 + 'a;

type Strs<'a> = dyn Fn(&hdf5_metno::Group, &str, &[String]) -> H5 + 'a;

fn write_registration(f: &hdf5_metno::File, r: &RegistrationLayer, ints: &Ints, strs: &Strs) -> H5 {
    let g = f.create_group("registration")?;
    let floats = |g: &hdf5_metno::Group, name: &str, v: &[f64]| -> H5 { g.new_dataset_builder().with_data(v).create(name).map(drop) };
    let attr_f = |name: &str, v: f64| -> H5 { g.new_attr::<f64>().shape(()).create(name)?.as_writer().write_scalar(&v) };
    attr_f("camera", r.camera.map_or(-1.0, f64::from))?;
    attr_f("threshold_um", r.threshold_um.map_or(f64::NAN, f64::from))?;
    attr_f("snap", r.snap.map_or(-1.0, |s| s as u8 as f64))?;
    attr_f("video_scale", r.video_scale.map_or(f64::NAN, f64::from))?;

    let a = g.create_group("anchors")?;
    strs(&a, "position", &r.anchors.keys().cloned().collect::<Vec<_>>())?;
    floats(&a, "x_um", &r.anchors.values().map(|p| p[0]).collect::<Vec<_>>())?;
    floats(&a, "y_um", &r.anchors.values().map(|p| p[1]).collect::<Vec<_>>())?;

    let v = g.create_group("verified")?;
    strs(&v, "session", &r.verified.iter().cloned().collect::<Vec<_>>())?;

    let n = g.create_group("nudges")?;
    strs(&n, "run_path", &r.nudges.keys().cloned().collect::<Vec<_>>())?;
    floats(&n, "dx_um", &r.nudges.values().map(|p| p[0] as f64).collect::<Vec<_>>())?;
    floats(&n, "dy_um", &r.nudges.values().map(|p| p[1] as f64).collect::<Vec<_>>())?;

    let run_edges = |g: &hdf5_metno::Group, keys: &mut dyn Iterator<Item = &RunEdge>| -> H5 {
        let keys: Vec<&RunEdge> = keys.collect();
        strs(g, "run_path", &keys.iter().map(|k| k.0.clone()).collect::<Vec<_>>())?;
        for (i, name) in ["u", "v", "k"].iter().enumerate() {
            ints(g, name, &keys.iter().map(|k| k.1[i]).collect::<Vec<_>>())?;
        }
        Ok(())
    };
    let s = g.create_group("assignments")?;
    run_edges(&s, &mut r.assignments.keys())?;
    let (mut offsets, mut edge, mut added, mut dir) = (vec![0i64], vec![], vec![], vec![]);
    for chain in r.assignments.values() {
        for (e, d) in chain {
            let (id, is_added) = match e {
                EdgeRef::Pipeline(e) => (*e as i64, 0),
                EdgeRef::Added(id) => (*id, 1),
            };
            edge.push(id);
            added.push(is_added);
            dir.push(*d as i64);
        }
        offsets.push(edge.len() as i64);
    }
    ints(&s, "chain_offsets", &offsets)?;
    ints(&s, "chain_edge", &edge)?;
    ints(&s, "chain_added", &added)?;
    ints(&s, "chain_direction", &dir)?;

    let d = g.create_group("disabled")?;
    run_edges(&d, &mut r.disabled.iter())
}

fn read_registration(g: &hdf5_metno::Group) -> hdf5_metno::Result<RegistrationLayer> {
    let ints = |name: &str| g.dataset(name).and_then(|d| d.read_raw::<i64>());
    let floats = |name: &str| g.dataset(name).and_then(|d| d.read_raw::<f64>());
    let strs = |name: &str| -> hdf5_metno::Result<Vec<String>> {
        Ok(g.dataset(name)?.read_raw::<VarLenUnicode>()?.iter().map(|s| s.to_string()).collect())
    };
    let attr = |name: &str| g.attr(name).and_then(|a| a.read_scalar::<f64>()).unwrap_or(f64::NAN);
    let mut r = RegistrationLayer {
        camera: Some(attr("camera")).filter(|c| *c >= 0.0).map(|c| c as u8),
        threshold_um: Some(attr("threshold_um")).filter(|t| t.is_finite()).map(|t| t as f32),
        snap: Some(attr("snap")).filter(|s| *s >= 0.0).map(|s| s != 0.0),
        // absent in files from before the calibration existed: NaN -> None
        video_scale: Some(attr("video_scale")).filter(|v| v.is_finite() && *v > 0.0).map(|v| v as f32),
        ..Default::default()
    };
    let (names, xs, ys) = (strs("anchors/position")?, floats("anchors/x_um")?, floats("anchors/y_um")?);
    r.anchors = (0..names.len()).map(|i| (names[i].clone(), [xs[i], ys[i]])).collect();
    r.verified = strs("verified/session")?.into_iter().collect();
    let (runs, dx, dy) = (strs("nudges/run_path")?, floats("nudges/dx_um")?, floats("nudges/dy_um")?);
    r.nudges = (0..runs.len()).map(|i| (runs[i].clone(), [dx[i] as f32, dy[i] as f32])).collect();
    let run_edges = |p: &str| -> hdf5_metno::Result<Vec<RunEdge>> {
        let (runs, u, v, k) = (strs(&format!("{p}/run_path"))?, ints(&format!("{p}/u"))?, ints(&format!("{p}/v"))?, ints(&format!("{p}/k"))?);
        Ok((0..runs.len()).map(|i| (runs[i].clone(), [u[i], v[i], k[i]])).collect())
    };
    let keys = run_edges("assignments")?;
    let (offsets, edge, added, dir) =
        (ints("assignments/chain_offsets")?, ints("assignments/chain_edge")?, ints("assignments/chain_added")?, ints("assignments/chain_direction")?);
    r.assignments = keys
        .into_iter()
        .enumerate()
        .map(|(i, key)| {
            let chain = (offsets[i] as usize..offsets[i + 1] as usize)
                .map(|j| {
                    let e = if added[j] != 0 { EdgeRef::Added(edge[j]) } else { EdgeRef::Pipeline(edge[j] as usize) };
                    (e, if dir[j] >= 0 { 1 } else { -1 })
                })
                .collect();
            (key, chain)
        })
        .collect();
    r.disabled = run_edges("disabled")?.into_iter().collect();
    Ok(r)
}

pub fn load(path: &Path) -> Result<Annotations, String> {
    read_file(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn read_file(path: &Path) -> hdf5_metno::Result<Annotations> {
    let f = hdf5_metno::File::open(path)?;
    let attr_str = |name: &str| -> String {
        f.attr(name)
            .and_then(|a| a.read_scalar::<VarLenUnicode>())
            .map(|s| s.to_string())
            .unwrap_or_default()
    };
    let ints = |name: &str| f.dataset(name).and_then(|d| d.read_raw::<i64>());
    let strs = |name: &str| -> hdf5_metno::Result<Vec<String>> {
        Ok(f.dataset(name)?.read_raw::<VarLenUnicode>()?.iter().map(|s| s.to_string()).collect())
    };
    let idx = |v: i64| v as usize;

    let mut a = Annotations::new(&attr_str("author"), &attr_str("backbone_fingerprint"));
    a.excluded = ints("excluded/edge_id")?.into_iter().map(idx).collect();
    a.duplicate_of = ints("duplicate_of/edge_id")?
        .into_iter()
        .zip(ints("duplicate_of/of_edge_id")?)
        .map(|(e, k)| (idx(e), idx(k)))
        .collect();
    a.hyperedge = ints("hyperedge_overrides/edge_id")?
        .into_iter()
        .map(idx)
        .zip(ints("hyperedge_overrides/hyperedge")?)
        .collect();
    a.roots = ints("roots/node_id")?.into_iter().collect();
    let (ids, us, vs) = (ints("added_edges/id")?, ints("added_edges/u")?, ints("added_edges/v")?);
    a.added = (0..ids.len()).map(|i| AddedEdge { id: ids[i], u: us[i], v: vs[i] }).collect();
    a.notes = strs("notes/kind")?
        .into_iter()
        .zip(ints("notes/id")?)
        .zip(strs("notes/text")?)
        .map(|((kind, id), text)| {
            let target = if kind == "node" { Target::Node(id) } else { Target::Edge(idx(id)) };
            (target, text)
        })
        .collect();
    if let Ok(g) = f.group("registration") {
        a.registration = read_registration(&g)?;
    }
    if let (Ok(runs), Ok(m)) = (f.dataset("flow_runs"), f.dataset("flow_measurements")) {
        let attr_i = |n: &str| m.attr(n).and_then(|x| x.read_scalar::<i64>()).unwrap_or(0);
        a.flow = Some(FlowOutput {
            runs: runs.read_raw::<FlowRunRow>()?,
            measurements: read_measurements(&m)?,
            cutoff_um: m.attr("cutoff_um").and_then(|x| x.read_scalar::<f64>()).unwrap_or(f64::NAN) as f32,
            n_anchors: attr_i("n_anchors"),
            n_pins: attr_i("n_pins"),
            // absent in layers written before the factor was recorded
            video_scale: m.attr("video_scale").and_then(|x| x.read_scalar::<f64>()).unwrap_or(f64::NAN) as f32,
        });
    }
    // Optional: files written before trails existed have no /trails.
    if let (Ok(names), Ok(offsets), Ok(points)) =
        (strs("trails/name"), ints("trails/waypoint_offsets"), ints("trails/waypoints"))
    {
        a.trails = names
            .into_iter()
            .enumerate()
            .map(|(i, name)| (name, points[offsets[i] as usize..offsets[i + 1] as usize].to_vec()))
            .collect();
    }
    let (utc, authors, ops) = (ints("history/utc")?, strs("history/author")?, strs("history/op")?);
    a.history = (0..utc.len())
        .map(|i| HistoryRow { utc: utc[i], author: authors[i].clone(), op: ops[i].clone() })
        .collect();
    Ok(a)
}

/// `/flow_measurements` as written before the along-arc span fields existed.
#[derive(hdf5_metno::H5Type, Clone, Copy)]
#[repr(C)]
struct FlowMeasurementRowV1 {
    edge_id: i64,
    direction: i8,
    run_index: i64,
    src_u: i64,
    src_v: i64,
    src_k: i64,
    median: f64,
    perc_25: f64,
    perc_75: f64,
    flux_ratio: f64,
    coverage: f64,
    pixel_count: f64,
    flux: f64,
}

/// Reads `/flow_measurements`; files from before `s_start_um` / `s_end_um` get NaN spans.
fn read_measurements(m: &hdf5_metno::Dataset) -> hdf5_metno::Result<Vec<FlowMeasurementRow>> {
    let has_spans = match m.dtype()?.to_descriptor()? {
        hdf5_metno::types::TypeDescriptor::Compound(c) => c.fields.iter().any(|f| f.name == "s_start_um"),
        _ => false,
    };
    if has_spans {
        return m.read_raw::<FlowMeasurementRow>();
    }
    Ok(m.read_raw::<FlowMeasurementRowV1>()?
        .into_iter()
        .map(|r| FlowMeasurementRow {
            edge_id: r.edge_id,
            direction: r.direction,
            run_index: r.run_index,
            src_u: r.src_u,
            src_v: r.src_v,
            src_k: r.src_k,
            median: r.median,
            perc_25: r.perc_25,
            perc_75: r.perc_75,
            flux_ratio: r.flux_ratio,
            coverage: r.coverage,
            pixel_count: r.pixel_count,
            flux: r.flux,
            s_start_um: f64::NAN,
            s_end_um: f64::NAN,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_flow_tables_written_before_spans() {
        let dir = std::env::temp_dir().join(format!("gramf-v1-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("old.h5");
        let f = hdf5_metno::File::create(&path).unwrap();
        let row = FlowMeasurementRowV1 {
            edge_id: 4, direction: 1, run_index: 0, src_u: 0, src_v: 1, src_k: 0,
            median: 1.0, perc_25: 0.5, perc_75: 1.5, flux_ratio: 0.2, coverage: 1.0, pixel_count: 9.0, flux: 2.0,
        };
        let m = f.new_dataset_builder().with_data(&[row]).create("flow_measurements").unwrap();
        let rows = read_measurements(&m).unwrap();
        assert_eq!((rows[0].edge_id, rows[0].flux), (4, 2.0));
        assert!(rows[0].s_start_um.is_nan() && rows[0].s_end_um.is_nan());
        drop((m, f));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
