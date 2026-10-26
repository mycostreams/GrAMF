# GrAMF

A Rust / wgpu desktop app to view and annotate the spatio-temporal graphs (STGs) of AMF
networks that [STHype](https://github.com/mycostreams/STHype) writes as `stg_store.h5`, and
to register flow videos onto them (it replaces `registrator`). Colleagues run it on their own
laptops. [DESIGN.md](DESIGN.md) records the decisions.

In the pipeline: it reads the store and the plate's flow folders, and writes annotations and,
on Publish, the store's flow layer. See
[rhizosphere/repos/GrAMF.md](https://github.com/Staalduine/rhizosphere/blob/main/repos/GrAMF.md).

## What it does

- opens `stg_store.h5` read-only (anchor-seg and arc-unit stores) and draws every arc;
- time slider with dated growth, in frames, UTC, hours since inoculation or since crossing
  (plate dates from the lab registry `AMF-PlateList`); colour by t_first, width or hyperedge;
- click to inspect edges, nodes and hyperedges;
- per-author annotation files `annotations.<author>.h5` (exclude, merge duplicates, roots,
  added edges, trails, notes, hyperedge overrides) with undo/redo;
- video registration: places the plate's flow runs on the STG and matches them to its edges;
- **Publish to store** writes the flow layer, trails and roots into the store, after keeping
  the original as `stg_store.h5.bak-before-gramf-publish-<date>`. This is the only time GrAMF
  writes a store;
- CSV / GeoJSON export with a provenance README;
- a developer view (View > Developer) with the store's datasets, health checks, raw rows
  and the matcher evidence behind the dating.

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
| File > Open store… | pick a `stg_store.h5` (on macOS File / Edit / View are in the screen's menu bar) |
| right-drag, Space + drag, or scroll (Shift: sideways); also left-drag with Select | pan, with any tool |
| Option or Cmd/Ctrl + scroll, trackpad pinch | zoom at the cursor |
| Cmd/Ctrl + = / − / 0 / 2 (View menu); Home | zoom in / out / fit the plate / zoom to the selection; fit |
| ← → / Space tap | previous / next timepoint (Video registration: run) / play |
| V B X M R E T | tools: select, box select, exclude, merge duplicate, root, add edge, trail |
| Esc | cancel a tool step, else clear the selection |
| Cmd/Ctrl+S, +Z, +Shift+Z | save annotations, undo, redo |
| File > Export tables… | CSVs (+ README) for the plate, the view, or the selection (incl. a box) |
| Layers: lines by width | line thickness follows each edge's width at the current timepoint |
| « / » | hide or show the side panels |

Your annotations are saved as `annotations.<your name>.h5` next to the store; colleagues'
files there show as extra layers. Trails (T) follow the shortest path through the waypoints
you click and are saved with your annotations.

**Video registration**: the second workflow button lists the plate's video runs from the folder you
choose (`<position>/<timestamp>/` run folders; GrAMF never guesses one), grouped
into sessions, with a stage scatter and a run slider that moves the STG to the run's time.
Place a session, nudge runs onto the graph, then press **Publish to store…** in the registration panel.
When the network's pixel size is off (video graphs too small or large against the STG), set the
session's **video scale** (STG µm per video µm), e.g. with **Set from fit scale** once 2+ anchors
are placed; **Save as <setup> default** keeps it for the next plate.

Settings (⌘, on macOS, File > Settings… elsewhere) hold your author name, the plate registry
folder and the video-registration session gap; they live in `~/.gramf/settings`.

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
  main.rs, lib.rs     entry point
  store/              read-only stg_store.h5 loader, picking; grid (nearest arc), width,
                      route (trails), detached (arc ends far from their node)
  annotations/        annotations.<author>.h5 model and undo; file (atomic save, load),
                      publish (Publish to store)
  video/              flow videos: runs (folders, sessions), placement, matching, flow
  export.rs           CSV / GeoJSON / README export
  devtools.rs         dataset listing, health checks, raw rows, matcher evidence
  registry.rs         plate lookup in the AMF-PlateList xlsx files
  settings.rs         ~/.gramf/settings      time.rs  time axis    colour.rs  colour-by
  update.rs           newer-release check
  app/                winit/egui app: actions (files), picking, overlays, window (event loop),
                      annotate, export_dialog, dev_panel, settings_window, native_menu
    ui/               panels: timeline, inspector, legend
    registration/     Video registration workspace: panels, map
  renderer/           wgpu setup, segment renderer and its vertex layout, 2D camera
```
