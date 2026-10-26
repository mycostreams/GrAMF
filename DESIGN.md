# GrAMF design — users, data, feature map, UI

Decisions from the 2026-10-19 design session. Each section states what was decided; open
points are collected at the end. Change this file when a decision changes.

## 1. Users

About 20 lab colleagues on a mix of macOS and Windows, with mixed coding competence, all
working with plates and their extracted networks. Three uses:

- **View and inspect** a plate's spatio-temporal graph (STG) over time.
- **Annotate** it — corrections live in a per-author layer, never in pipeline data.
- **Export** raw tables (CSV) for their own analysis.

Pipeline developers additionally get a developer view (store inspector, evidence overlay).
GrAMF also replaces the `registrator` repo (v2).

## 2. Data sources

| source | access | notes |
|---|---|---|
| `stg_store.h5` (STHype) | **read-only** | opened via File > Open; written only by the explicit, temporary Publish to store bridge (§10) |
| `annotations.<author>.h5` annotation files | read/write (own author only) | beside the store; several can be loaded at once |
| plate registry `AMF-PlateList/{NOW,OLD}_Experiments/*.xlsx` | read-only | path configurable; read with `calamine` |
| per-frame spatial graphs (GeoJSON, later `graph.h5`) | read-only | separate workflow (v3), STG integration later |

### `stg_store.h5` as found on disk (not as the ADR's table says)

Flat datasets, per-dataset `schema_version`; step-owned layers carry `backbone_fingerprint`.

- `/nodes` `(n,3)` f64 `(id, x_um, y_um)` — **x = column** (opposite of amftrack's `row_um`).
- `/edges` `(m,2)` i64 node ids; **row index = `edge_id`**, the key every other layer uses.
- `/arcs_points` `(N,2)` f64 + `/arcs_offsets` `(m+1,)` — true polyline per edge (N+1 CSR).
- `/points` `(m,2)` — anchor-frame centre per edge (matcher query point, not geometry).
- `/edge_lengths_um`, `/t_first`, `/hyperedges` (`-1` = none) — `(m,)`, row-aligned.
- Time evidence, depends on root attr `structure`:
  - `anchor-seg`: `/edge_time_runs` `(hits,3)` `(edge_id, frame_index, score)` sparse,
    `+ /edge_time_runs_closest_nodes`; `score_semantic` attr says the direction.
  - `arc-unit`: `/arc_time_runs` `(runs,3)` `(edge_id, s_start_um, t_index)` run-length.
- `/edge_width_segments`, `/edge_width_timesteps` — structured arrays, see their attrs.
- `/timepoints` (epoch s), `/timepoints_quality_weight` (0 = gated frame).
- `/spores` (uuid), `/spores_position`, `/spores_activation`, `/spores_radii`.
- Optional human layers written by the old registrator: `/flow_*`, `/root_nodes`.

Scale of a real plate: ~60k edges, ~1.4M arc points, ~38 timepoints, 70 MB.

Unknown datasets or newer schema versions are shown in the inspector, never refused.

## 3. Editing semantics

- **Annotation-only over an immutable backbone.** Edits are keyed on `edge_id` / `node_id`;
  nothing renumbers. Deletion is a flag (`excluded`), not a removal.
- Edit vocabulary (v1): `excluded` flag, `duplicate_of` (parallel-edge merge: consumers drop
  the duplicate), hyperedge override, root nodes, spore corrections, free-text note, and
  **added edges** — node-to-node only, own id range, drawn polyline (straight by default),
  no time/width data unless supplied.
- Not now: edge splitting / mid-edge attachment (the 34.5 µm anchor-seg segmentation makes
  node-to-node reach almost anywhere), width re-estimation (stretch goal).

### Annotation file `annotations.<author>.h5`

- One file per author (author name set once in preferences). Different authors may hold
  different interpretations; downstream analysis names the annotation file it used. No automatic
  merge; a compare/copy view can move records between annotation files (e.g. into `consensus`).
- Contents (schema v1, one group per layer, 1-D columns so any HDF5 reader can open it):

  | group | columns |
  |---|---|
  | `/excluded` | `edge_id` |
  | `/duplicate_of` | `edge_id`, `of_edge_id` (consumers drop `edge_id`) |
  | `/hyperedge_overrides` | `edge_id`, `hyperedge` |
  | `/roots` | `node_id` |
  | `/added_edges` | `id` (from 1 000 000), `u`, `v` (node ids; straight, no time/width) |
  | `/notes` | `kind` (`edge`/`node`), `id`, `text` |
  | `/history` | `utc`, `author`, `op` — append-only, every applied edit including undo/redo |

  Later layers (`/flow_*` from video registration) are added as new groups.
- Root attrs: `backbone_fingerprint` of the store it was authored against, `author`,
  `annotation_schema_version`, GrAMF version. Fingerprint mismatch = stale, shown loudly.
- **Explicit save** (Cmd/Ctrl+S, dirty marker, prompt on close), in-memory undo/redo.
  Writes are atomic: write `*.tmp`, then rename.
- A store carrying `/root_nodes` (old registrator) offers a one-time import into the user's
  file; `/flow_*` import comes with the video-registration workflow (v2).
- Author name, registry folder and session gap: Settings window, stored in `~/.gramf/settings`
  (`key = value`; the older `~/.gramf/author` is read once). `GRAMF_REGISTRY` still overrides.
- `/trails`: `name`, `waypoint_offsets` + `waypoints` (what the user clicked), `node_offsets` +
  `nodes` (the routed path, written at save for readers that cannot route).

## 4. Time model

- Slider over `/timepoints`; gated frames (`quality_weight == 0`) are marked.
- Axis labels switchable: frame index, UTC, **hours since inoculation**, **hours since crossing**.
  - Inoculation = registry column `Start`. (`Infection date` is a different event.)
  - Crossing = registry `Crossed` if filled, else the first imaging day of the plate, else
    the store's first timepoint labelled `≈`.
- Display modes: **dated** (default — edge shown when `t_first ≤ t`; arc-unit stores trim the
  polyline at the run start), **evidence** (dev: matcher hits at frame t, coloured by score;
  dated-but-unseen edges faded).

### Registry join

Plate key = `UI`, with `old UI` as alias. A store resolves to a plate by: store `plate_id`
attr (if the pipeline ever writes one) → folder name as UI → `NNN_CODE` → `CODENNN`
(e.g. `127_HHE2601A` → `HHE2601A127`) → user picks once, remembered in the annotation file.
Columns are matched by header name, not position. `NOW_` beats `OLD_`; files named
`*conflicted copy*` are ignored with a warning.

## 5. Exports (v1: raw tables only)

One Export dialog (File > Export tables…): tables as checkboxes, scope = whole plate / in view
and appeared by the current t / current selection (edge, node or hyperedge; a lasso comes later),
and which author's annotation file to apply (excluded edges dropped, or kept with `excluded=1` —
default keep-and-flag). Every export folder gets a `README.txt` (source path, fingerprint,
annotation author, units, GrAMF version). Output goes to a new `gramf_export_<plate>_<time>`
folder inside the folder the user picks, so nothing is overwritten.

- `edges.csv` — `edge_id, u, v, length_um, t_first, t_first_h_inoc, t_first_h_cross,
  width_um, width_se_um, hyperedge, x_mid_um, y_mid_um, excluded, duplicate_of, note`
- `nodes.csv` — `node_id, x_um, y_um, degree, is_root, is_tip`
- `edge_widths_t.csv` — long format from `/edge_width_timesteps`
- `timepoints.csv` — `index, utc, h_inoc, h_cross, quality_weight`
- `hyperedges.csv` — `id, n_edges, total_length_um, t_first_min`
- optional geometry (GeoJSON) for QGIS/shapely users

Computed properties (growth rates, branch angles, network summaries) are future items,
decided one by one.

## 6. Feature map

### v1 — view, annotate, export

| # | milestone | done when |
|---|---|---|
| M1 | Load `stg_store.h5` (read-only) and draw all arcs; 2D ortho pan / zoom-to-cursor / fit | a real 60k-edge plate renders at interactive frame rate |
| M2 | Time slider + dated mode + axis units; registry lookup | scrubbing is a uniform update, no buffer re-upload |
| M3 | Picking + inspector: edge / node / hyperedge; labels for node ids and hyperedge ids; colour by t_first / width / hyperedge | clicking an edge shows all its fields |
| M4 | Annotations: edit tools, undo/redo, save, multi-author layers, stale-fingerprint warning | round-trip test: edit → save → reopen → same state |
| M5 | Export dialog + README | CSVs open in Excel/pandas and join on `edge_id` |
| M6 | Developer view: store inspector, health checks, raw row view, evidence overlay, timings | — |
| M7 | CI releases: macOS + Windows binaries on tag, static HDF5, update notice | colleague installs from Releases without a toolchain |

### v2 — Video registration workflow (replaces `registrator`)

Designed 2026-10-20 (§10 has the full design). Milestones, each usable on its own:

| # | milestone | done when |
|---|---|---|
| R1 | Settings panel (author, registry path, session gap); Trail tool (T) in Visualize & clean up | trails saved, reloaded, drawn |
| R2 | Open a plate's `polyflow_analysis/`: runs, sessions, setup from camera serial, stage scatter, run slider; show an existing store `/flow_*` read-only | FLW2605A003 loads 6 positions x 35 runs in one session |
| R3 | Placement: setup default transform, anchors with live fit and residuals, camera orientation, overlays on the map, Verify session | videos land on hyphae on FLW2605A003 |
| R4 | Snap + cover: per-run rigid snap, coverage chains with direction, gaps drawn, flow values per video edge | every video edge shows its STG chain or a gap |
| R5 | Per-run corrections (assign / remove / flip, disable artifact, nudge, copy from previous run); save registration + flow layer to the annotation file | round trip: correct, save, reopen, same mapping |
| R6 | Publish to store (bridge); STHype reader accepts annotation files; transport_paper reads them | transport_paper runs on an annotation file |

### v3 — per-frame spatial graphs

Separate workflow: open a frame's GeoJSON (later `graph.h5`; note `row_um/col_um`) on its own.
STG integration (overlay a frame on the backbone for matching debug) later.

### Future

- 2D+t globally registered volumes (after the volume → STG pipeline exists).
- 3D networks (~1–1.5 y): keep 3D vertex positions and the depth buffer; ortho camera now.
- Compute & plot workflow: computed properties (decided per item) and plots.
- Width re-estimation; edge splitting / mid-edge attachment.

## 7. UI layout

### Workflows (Affinity-style persona buttons)

Big buttons top-left switch the whole workspace: toolbar, side panels and the inspector's
contents change; the map view, timeline and loaded store stay put.

| workflow | toolbar | left panel | right panel | lands in |
|---|---|---|---|---|
| **Visualize & clean up** | Select, Lasso, Add edge, Exclude, Merge, Root; colour-by | Layers, Annotations | Inspector + annotation fields | v1 |
| **Video registration** | Select, Anchor, Assign, Disable edge, Nudge; threshold; Publish to store | Sessions + stage scatter | Position, run, video edges, chain, corrections | v2 (§10) |
| **Compute & plot** | choose metric / plot | Metric list | Plot area + export | future, per item |

Only implemented workflows get a button; v1 ships with one, so the switcher appears in v2.

```
┌───────────────────────────────────────────────────────────────────────────────┐
│ [◆ Visualize & clean up][▶ Video registration][∑ Compute & plot]              │
│ File  Edit  View  Help                     HHE2601A127 · Rachael  [● unsaved] │
├───────────────────────────────────────────────────────────────────────────────┤
│ [Select V][Lasso L][Add edge E][Exclude X][Merge M][Root R]  Colour: t_first ▾ │
├──────────────┬──────────────────────────────────────────────┬─────────────────┤
│ LAYERS       │                                              │ INSPECTOR       │
│ ☑ Network    │                                              │ Plate           │
│ ☑ Spores     │                                              │  strain, treat. │
│ ☑ Roots      │               map view                       │  inoc. / crossed│
│ ☐ Node ids   │        (pan: drag, zoom: wheel,              │ ─────────────── │
│ ☐ Hyperedge  │         fit: Home)                           │ Edge 812        │
│   ids        │                                              │  u→v, length    │
│              │                                              │  t_first, width │
│ ▾ ANNOTATIONS│                                              │  hyperedge 41   │
│ ◉ simon  ☑   │                                              │ Annotation      │
│ ○ rachael ☑  │                                     ⟝ 1 mm ⟞ │  ☐ excluded     │
│ + load…      │                                              │  note: ____     │
├──────────────┴──────────────────────────────────────────────┴─────────────────┤
│ ◀ ▶ ⏵  ├──┼──┼──╳──┼──┼──●──┼──┼──┤  t = 412 h since inoc. ▾   (╳ = gated)  │
├───────────────────────────────────────────────────────────────────────────────┤
│ stg_store.h5 · fingerprint ✓ · 60 841 edges · x 12 340 µm  y 8 120 µm         │
└───────────────────────────────────────────────────────────────────────────────┘
```

- **Annotations** panel is collapsible (▾/▸ header, state remembered); radio = the file being
  edited (always the user's own), checkbox = visible.
- Node and hyperedge ids are always available (layer toggles + inspector), not dev-only.
- View > Developer adds a bottom dock: store inspector, health checks, raw rows, timings.
- Box select (B): drag a box to select edges (scope for export).
- Right-drag always pans, whatever the tool or workflow; left-drag belongs to the tool (and pans only with Select).
- Navigation follows Adobe: scroll pans (Shift: sideways), Space + drag pans with any tool (a Space tap plays),
  ⌥ or ⌘ + scroll and pinch zoom at the cursor; ⌘= / ⌘− / ⌘0 / ⌘2 zoom in, out, fit, to the selection
  (the selected video frame in Video registration).
  The stage scatter takes the same controls while the pointer is over it (Home / ⌘0 fit the scatter,
  ⌘= / ⌘− zoom it, and the map stays put).
- Node tools (Root, Add edge) draw nodes, ring the one under the cursor (10 px snap) and, while
  adding an edge, show a rubber band from the first node.
- Lines are screen-space quads: thickness in px, optionally following each edge's width at the
  current timepoint (`/edge_width_timesteps`: seg estimate, else legacy, carried forward).
  Colour by width also follows the current timepoint; the scale is the 5-95 % range of all
  per-timestep widths, so colours compare across frames.
- Both side panels collapse (« / »); raw rows for an edge are a section of the inspector.
- Frames with quality weight 0 are labelled "faulty frame".
- Keys: ←/→ step frame, Space play, V/B/X/M/R/E tools, Esc cancels a tool step or clears the
  selection, Home fit, Cmd/Ctrl+S save, Cmd/Ctrl+Z / Shift+Z,
  Cmd/Ctrl+1/2/3 switch workflow.

## 8. Technical direction

- Rust + wgpu + egui (existing). Edges drawn as instanced thick-line segments with per-vertex
  `edge_id`, `t_first`, `s_um`; time filtering and colouring happen in the shader from a
  uniform, so scrubbing never re-uploads geometry.
- `hdf5-metno` (static feature) for HDF5, `calamine` for xlsx, `csv` for export.
- Store is loaded fully into memory on open (70 MB plate is fine).

## 9. Open points

- How downstream readers (STHype `store.py`, transport_paper) locate the chosen annotation file.
- Exact annotation file column schemas — settle each when its writer lands (same rule as the store ADR).
- Root/tip definition for `nodes.csv` (`is_tip` = degree 1 excluding added edges?).

## 10. Video registration design (v2)

Decided in the 2026-10-20 design session; the `registrator` survey that fed it found ~1,300
lines of real logic and ~1,700 of UI. GrAMF does **not** port its workflow one-to-one.

### Inputs

- A plate's `<runs folder>/<position>/<YYYYMMDD_HHMMSS>/` run folders, each with
  `run_metadata.json`, `extraction_graph.json` and `flow_analysis.parquet` (a run needs the
  first and the last). From the metadata: `date_time`, `mode`, `run_nr` (may be null),
  `deltas.delta_x` (µm/px), `camera.frame_size`, `camera.model` (incl. `SN:`) and
  `position.x/y` (stage mm; the folder name `x_n10_38_y_4_484` is only the fallback).
  The runs folder (`polyflow_analysis` on Snellius, `polyflow_data` or `polyflow_data_walls_v4` in
  Dropbox sessions) is always chosen by the user. It is never looked up beside the store, because
  a plate can carry several run sets and a guessed one would be published unnoticed.
- Video graph: `extraction_graph.json` edges `[u, v, k]` with `pixel_list`; node positions in µm
  relative to the frame centre (`(pixel - frame_size/2) * delta_x`). Flow values per edge and
  direction from the parquet: `median, perc_25, perc_75, flux_ratio, coverage, pixel_count,
  flux` (per-edge constants; take the first row per direction). Repair pre-fix parquets as
  the registrator does (swap inverted quartiles; NaN `flux_ratio` = mean of per-frame values).
- Each run snaps to the first STG timepoint at or after its `date_time` (clamped to the last).
- Test plate: `registrator/temp/FLW/FLW2605A003` (store + 6 positions x 35 runs, ARETHA);
  `FLW2605A015` as a second.

### Setups and sessions

- **Setup from the video camera serial**, copied from `pipeline-steps/pipeline_steps/optics.py`
  (the source of truth; keep in step): `25054443` = ARETHA videos (acA2040-90um),
  `40018720` = MORRISON videos (acA4112-30um). Unknown serial: scale only.
- **Default transform per setup.** ARETHA: full stage-to-STG default (scale, rotation, mirror,
  offset), because plate imaging and videos share one machine. MORRISON: default scale only;
  rotation and mirror from anchors, because the plate is imaged elsewhere (PRINCE).
  Stage scale is ~1000 µm/mm; the video's own scale is `deltas.delta_x`.
- **Video scale** (STG µm per video µm, default 1) is part of the setup default and is saved with
  each registration (`/registration` attr `video_scale`; older files read as unset, which means
  the setup default, else 1). It scales every video graph and frame about the frame centre before
  placement. It exists because a store is in whatever µm its network pixel size made: PRINCE
  stores are built at the nominal 1.725 µm/px (3.45 µm / 2x), and EPY2412A088 fits at
  ~1146 µm/mm, i.e. the STG reads ~1.15x long against MORRISON stage mm and 0.138 µm/px videos.
  "Set from fit scale" sets it to the session's fit scale / 1000 (2+ anchors). The session's own
  scale still only spaces positions; this factor is the only thing that resizes videos.
- **Camera orientation** (video image axes vs stage axes) is part of the setup default, found
  once by trying the 8 rotation/mirror options and keeping the best overlap with the STG.
- **Sessions**: runs grouped by position's first-run time, new session when the gap exceeds
  15 minutes (Settings). One transform per session, starting from the previous session's
  (ARETHA's first from the default). Never trusted silently: a session placed with 0 anchors
  is **unverified** until the user confirms it or adds an anchor. Each session shows a fit
  score (how closely its snapped video graphs lie on the STG).
- **Anchors** refine, live, as they are added: 1 fixes offset, 2 rotation and scale, 3+ a
  least-squares fit with per-anchor residuals so a bad click stands out.
- As built (R3): an anchor ties a position's video centre to a clicked STG point (snapped to
  a node within 10 px). The fit score is the mean distance of the placed video-graph samples
  to the nearest arc, capped at 50 µm. GrAMF ships an ARETHA default fitted from
  FLW2605A003's six registrator anchors (scale 1008 µm/mm, 0.86°, mirrored, camera option 3;
  score 6.1 µm with no anchors); "Save as <setup> default" replaces it in `~/.gramf/settings`.
  Anchors, verified flags and the camera choice are kept in memory until R5 saves them.

### Matching: overlay, snap, cover (replaces the line-graph assignment and pins)

1. **Overlay**: the session transform and camera orientation place the whole video graph
   (not just its centre) in STG µm.
2. **Snap**: per run, a small rigid fit (translation + small rotation) of the video graph onto
   nearby STG arcs; the user can **nudge** it by dragging.
3. **Cover**: sample each video edge; the STG edges (pipeline or added) its samples lie on,
   within a threshold (default 10 µm), form its **chain**, each with a direction from the
   shared tangent. Stretches on no edge are **gaps**, drawn dashed amber. Poorly covered edges
   are flagged, never forced onto something.
4. As built (R4): snap is ICP (rotation about the centroid + translation, 20 iterations,
   30 then 12 µm pairing radius) and is refused beyond 50 µm or 5°; cover samples every 2 µm
   and drops chain members covered for less than 6 µm (unless that is all an edge has); gaps
   are drawn from 5 µm. Directions are relative to `/edges` (node1 -> node2), checked per edge
   because ~3 % of arcs are stored node2 -> node1. On FLW2605A003 the snap moves runs 3-9 µm
   and most video edges are 100 % covered. Flow values come from `flow_analysis.parquet`
   (`edge_*` columns, first row per edge and direction; registrator's repairs applied).

### Corrections (per run, by decision)

- Assign: with a video edge selected, click STG edges to add / remove them from its chain;
  F flips direction. Disable: mark a video edge as an artifact (contributes no flow).
- Corrections are stored per (run, video edge) and override the automatic result; nothing
  else re-solves. "Copy corrections from previous run" is an explicit button and lists what
  it copied.
- Gaps are bridged by hand with the Add edge tool (no suggested or automatic bridges); the
  next cover pass includes the added edge, so flow can map onto added edges (ids >= 1 000 000).

### Workspace (Video registration workflow)

- Left: session list (status, anchors, fit score; Verify session) and the **stage scatter**
  of the selected session (positions coloured mapped / gaps / unplaced; click to select).
  No list view.
- Map: all positions of the session faint, the selected run bright with per-edge colours,
  gaps dashed, the snap window outlined.
- Right: position and run (and the STG frame it snaps to), its video edges with coverage %
  and chain length, the selected edge's chain and flow values, the correction controls.
- Bottom: a run slider for the selected position; the STG time follows the run.
- Tools: Select (S), Anchor (A), Assign (G), Disable edge (D), Nudge (N); threshold control;
  Publish to store….
- Not now: video frames under the overlay (not reachable); flow display in the Visualize
  workflow (its own design round later).

### Trails (Visualize & clean up)

Trail tool (T): name a trail, click waypoints; it follows the geometric shortest path along
the STG. Saved in the annotation file next to roots and notes.

### Storage

All in the author's `annotations.<author>.h5` (new optional groups; no schema bump):

| group | content |
|---|---|
| `/registration/sessions` | start / end utc, 2x3 transform, verified |
| `/registration/anchors` | session, position (run folder relative to `polyflow_analysis`), node_id |
| `/registration/nudges` | run_path, dx, dy, d_rotation |
| `/registration/assignments` | run_path, video u/v/k, chain as CSR of (edge_id, direction) |
| `/registration/disabled` | run_path, video u/v/k |
| `/flow_runs`, `/flow_measurements` | the result, in STHype's existing compound schema (edge_id may be an added edge) |
| `/trails` | names, node path CSR, waypoint CSR |

Anchors, nudges and corrections are saved, so a registration can be reopened and corrected
(the registrator only stored their counts). As built (R5): every registration change is an annotation edit
(undo/redo, unsaved marker); the flow layer is recomputed on save from **verified sessions
only**, so unchecked placements never reach downstream analysis (FLW2605A003: 210 runs,
6,462 measurements in 1.6 s). STHype's `read_flow_mappings` reads the result unchanged
when its tables are placed in a store. On open, a store's existing `/flow_*`, trails and
`/root_nodes` are offered for import, like `/root_nodes` today.

**Publish to store** (temporary bridge): an explicit, confirmed action that writes the active
author's `/flow_*`, trails and `/root_nodes` into the store, in STHype's existing format, for
consumers that do not read annotation files yet. It is the one place GrAMF writes a store and
is removed once STHype's `read_flow_mappings` accepts an annotation file and transport_paper
names the file it uses. As built (R6): a toolbar button in Video registration; it saves first, asks with a
native dialog, refuses a store whose `backbone_fingerprint` differs from the annotations',
and writes a copy (`stg_store.h5.publishing`) renamed over the store only when complete; the
store as it was is kept beside it as `stg_store.h5.bak-before-gramf-publish-<date-time>`. Before
asking, it names any trails or roots in the store that the annotations lack (they would be
dropped) and offers to import them first; the confirm dialog names the flow layer it replaces.
Measurements on added edges have no store `edge_id`, so they stay in the annotation file
(the status line counts them). STHype's `read_flow_mappings` and `read_root_nodes` read
the result.
