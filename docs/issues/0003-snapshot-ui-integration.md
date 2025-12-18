# Issue: Snapshot & UI integration (visualization & editing)

Background
---
Snapshots should be representable and editable in the UI. Currently, UI rendering uses static geometry and ignores time-varying width/active flags.

Proposed change
---
- Extend `SnapshotGraph` to contain visual properties (width, `state`/`progress`, and computed `real_length` from `base_length` where relevant). Edges should expose a time-invariant `base_length`.
- Add explicit source metadata to snapshots and UI views:
  - Snapshot source enum: `RawSnapshot(time_idx) | STGView(time_idx) | FullSTG`.
  - Snapshot metadata includes time index, origin file/id, and whether shown data is raw or includes user edits.
- Update UI systems (`src/gramf_ui/ui_graph.rs`, `bevy_utils/graph_entities.rs`) to render `width`, `state/progress`, and to display a clear indicator of what is being shown (raw snapshot vs STG view vs full STG).
- Add editing modes and a visible mode toggle: "Edit Snapshot (write changes to a snapshot export/geojson)" vs "Edit STG (apply user edits overlay to STG)". Edits must be tracked separately as `user_edits` overlay vs raw data.
- Add Snapshot import/export as GeoJSON (per-snapshot GeoJSON files) and hooks to load a GeoJSON snapshot into the viewport or into the STG as a user edit.
- Add basic edit-inspector UI: select edge → show source/time → edit numeric property → update `user_edits` overlay with provenance info. (Low-level input handling and interactive path selection are covered in separate issues.)

Files to edit
---
- `src/graphs/snapshot.rs` (define SnapshotGraph & metadata)
- `src/gramf_ui/ui_graph.rs` (render snapshot visuals)
- `bevy_utils/graph_entities.rs` (update-in-place entity updates)
- `src/io/geojson.rs` (snapshot import/export helpers)
- `src/gramf_ui/ui_layout.rs` (inspector panel)
- `src/graphs/mod.rs` (apply user edit overlays)

Acceptance criteria
---
- Snapshot-sourced visual properties (width, state/progress, real_length) appear correctly in the viewport.
- UI displays the current view source and time index and indicates whether edits are applied.
- Editing an edge property in the inspector updates the `user_edits` overlay with provenance and does not alter raw data unless explicitly exported/merged.
- User can export/import single snapshots as GeoJSON with preserved metadata (time_idx, origin, schema_version, provenance).
- Snapshot refresh and edit application are implemented with in-place updates where possible (avoid full respawn).
- Interactive input handlers and path-selection behaviors are implemented in companion issues (see `docs/issues/0006-path-selection-export.md`).

Tasks
---
- [ ] Add source metadata and provenance fields to `SnapshotGraph`.
- Compute `base_length` on demand from node positions and document `real_length = base_length(node_positions) * progress` (do not require storing `base_length` in input files).
- [ ] Display view source/time and edit mode toggle in inspector UI.
- [ ] Implement GeoJSON export/import for snapshots and add tests.
- [ ] Ensure edits are recorded as `user_edits` with source/time provenance and support applying/unapplying overlays (sidecar `.gramf-edits.json`).
- [ ] Update spawn/update systems to read visual props and apply in-place entity updates.
- [ ] Implement inspector UI to edit properties and observe provenance (non-interactive handlers only).
- [ ] Add integration tests or smoke tests for snapshot import/export, edit persistence, growth delta/rate computations, and in-place updates.
- [ ] Coordinate interactive input & path selection with `docs/issues/0006-path-selection-export.md`.

Notes
---
- Keep UI edits as small overlays (diffs) to simplify undo/redo and persistence.
- Snapshots exported as GeoJSON should include snapshot metadata (time_idx, origin, schema_version) to preserve provenance.
- Prefer explicit index-based snapshot import/export to avoid ambiguity in interpolation semantics.
- Consider a lightweight "what you see is what you edit" indicator to avoid accidental modification of raw data.
- Interactive path selection and its deterministic indexing policy are covered in `docs/issues/0006-path-selection-export.md`.
- Consider modifier key + click variants later (e.g., Shift to add/remove single edges).

Complexity: Medium