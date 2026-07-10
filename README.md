# GrAMF

A standalone **Rust / wgpu** desktop application intended to become a GPU-accelerated viewer
for AMF (arbuscular mycorrhizal fungi) networks — in particular the spatio-temporal graphs
(STGs) produced by [STHype](../STHype). It is the visualisation corner of the ecosystem (see
[../docs/Overview.md](../docs/Overview.md)).

## Status: early prototype (in development, not yet wired in)

GrAMF is under active development but early. **Today it:**

- opens a window (titled "GrAMF") and renders a single **hardcoded quad** (`src/vertex/data.rs`
  `EDGE_VERTS`) through the wgpu pipeline, with an `egui` overlay and a camera;
- has **stub** File/Edit menu actions;
- **does not yet ingest real STG / graph data** — there is no graph loader or serialization
  path yet.

The next step toward usefulness is a real graph loader (a GeoJSON/`serde` path) so it can read
and display an STG instead of the placeholder geometry — see the "Smaller improvements" item in
[../docs/Roadmap.md](../docs/Roadmap.md) §6. The canonical on-disk graph format it should target
is documented in [STHype's `read_write.py`](../STHype/sthype/utils/read_write.py) (schema-v2 µm
GeoJSON).

## Tech stack

Rust (edition 2024), [`wgpu`](https://wgpu.rs) for GPU rendering, [`egui`](https://github.com/emilk/egui)
+ [`winit`](https://github.com/rust-windowing/winit) for the window/UI, and `cgmath` /
`nalgebra-glm` for math.

## Build & run

Requires a recent Rust toolchain (edition 2024 → Rust ≥ 1.85):

```bash
cargo run            # debug build + launch
cargo run --release  # optimised build
cargo build          # compile only
```

## Layout

```
src/
  main.rs            ← entry point: builds the winit event loop, runs App
  lib.rs             ← crate root (exports App)
  app/               ← application state + egui UI (menus are currently stubs)
  renderer/          ← wgpu setup (gpu.rs), graph renderer, camera
  vertex/            ← Vertex type + placeholder geometry (data.rs: the hardcoded quad)
```
