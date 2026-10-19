# GrAMF

A Rust / wgpu desktop application to view, annotate and export the spatio-temporal graphs
(STGs) of AMF (arbuscular mycorrhizal fungi) networks that [STHype](../STHype) writes as
`stg_store.h5`. It is the visualisation corner of the ecosystem (see
[../docs/Overview.md](../docs/Overview.md)); [DESIGN.md](DESIGN.md) records the decisions.

## Status

v1 features are in (M1–M6 of [DESIGN.md](DESIGN.md) §6):

- opens `stg_store.h5` **read-only** (anchor-seg and arc-unit stores) and draws every arc;
- time slider with dated growth, in frames, UTC, hours since inoculation or since crossing
  (plate dates from the lab registry `AMF-PlateList`);
- click to inspect edges, nodes and hyperedges; colour by t_first, width or hyperedge;
  node and hyperedge id labels;
- per-author annotation files `annotations.<author>.h5` (exclude, merge duplicates, roots,
  added edges, notes, hyperedge overrides) with undo/redo — the store is never written;
- CSV / GeoJSON export with a README of provenance;
- a developer view (View > Developer) with the store's datasets, health checks, raw rows
  and the matcher evidence behind the dating.

Next: video registration (replacing `registrator`), then per-frame spatial graphs.

## Install (lab colleagues)

Download the latest build from the
[Releases page](https://github.com/mycostreams/GrAMF/releases/latest) — nothing else needs
installing. GrAMF tells you in its top bar when a newer release exists.

- **macOS (Apple Silicon)**: `GrAMF-macos-arm64.zip`, unzip, move `GrAMF.app` to
  Applications. The app is not notarized, so the first time **right-click → Open → Open**
  (a plain double-click says it "cannot be opened").
- **Windows**: `GrAMF-windows-x64.zip`, unzip, run `GrAMF.exe`. If SmartScreen warns,
  choose **More info → Run anyway**.

Plate dates are read from `~/AMOLF-SHIMIZU Dropbox/AMF-PlateList`; set `GRAMF_REGISTRY`
to point elsewhere.

## Using it

| | |
|---|---|
| File > Open store… | pick a `stg_store.h5` |
| drag / scroll / Home | pan / zoom at the cursor / fit the plate |
| ← → / Space | previous / next timepoint / play |
| V X M R E | tools: select, exclude, merge duplicate, root, add edge |
| Esc | cancel a tool step, else clear the selection |
| Cmd/Ctrl+S, +Z, +Shift+Z | save annotations, undo, redo |
| File > Export tables… | CSVs (+ README) for the plate, the view, or the selection |

Your annotations are saved as `annotations.<your name>.h5` next to the store; colleagues'
files there show as extra layers. The name comes from `~/.gramf/author` (else your login)
and can be switched in the Annotations panel.

## Build from source

Rust ≥ 1.85 (edition 2024) and **CMake** (HDF5 is compiled from source and linked
statically): `brew install cmake` on macOS; on Windows install CMake and the MSVC build tools.

```bash
cargo run --release -- path/to/stg_store.h5   # the path is optional
cargo test --lib
```

## Releasing

1. Bump `version` in `Cargo.toml` and commit.
2. Tag and push: `git tag v0.2.0 && git push origin v0.2.0`.

`.github/workflows/release.yml` then tests on macOS and Windows, packages
`GrAMF-macos-arm64.zip` (via `scripts/package-macos.sh`) and `GrAMF-windows-x64.zip`, and
attaches both to the GitHub Release. The tag must equal the `Cargo.toml` version. Pull
requests run the same build and tests without releasing.

## Layout

```
src/
  main.rs, lib.rs   entry point
  store.rs          read-only stg_store.h5 loader, picking
  annotations.rs    annotations.<author>.h5 model, undo, atomic save
  export.rs         CSV / GeoJSON / README export
  devtools.rs       dataset listing, health checks, raw rows, matcher evidence
  registry.rs       plate lookup in the AMF-PlateList xlsx files
  time.rs           time axis and units      colour.rs  colour-by modes
  update.rs         newer-release check
  app/              winit/egui app: ui, annotate, export_dialog, dev_panel
  renderer/         wgpu setup, line-strip graph renderer, 2D camera
```
