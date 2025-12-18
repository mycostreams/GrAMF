# Issue: Snapshot & UI integration (visualization & editing)

Background
---
Snapshots should be representable and editable in the UI. Currently, UI rendering uses static geometry and ignores time-varying width/active flags.

Proposed change
---
- Extend `SnapshotGraph` to contain visual properties (width, active, growth values when relevant).
- Add explicit source metadata to snapshots and UI views:
  - Snapshot source enum: `RawSnapshot(time_idx) | STGView(time_idx) | FullSTG`.
  - Snapshot metadata includes time index, origin file/id, and whether shown data is raw or includes user edits.
- Update UI systems (`src/gramf_ui/ui_graph.rs`, `bevy_utils/graph_entities.rs`) to render `width`, `state/progress`, and to display a clear indicator of what is being shown (raw snapshot vs STG view vs full STG).
- Add editing modes and a visible mode toggle: "Edit Snapshot (write changes to a snapshot export/geojson)" vs "Edit STG (apply user edits overlay to STG)". Edits must be tracked separately as `user_edits` overlay vs raw data.
- Add Snapshot import/export as GeoJSON (per-snapshot GeoJSON files) and hooks to load a GeoJSON snapshot into the viewport or into the STG as a user edit.
- Add basic edit-inspector UI: select edge → show source/time → edit numeric property → update `user_edits` overlay with provenance info.
- Add interaction: Ctrl+Left-Click selects the shortest path between the last selected node/edge and the clicked node/edge. The computed path is highlighted. Consecutive Ctrl+Left-Clicks extend the path from the current end to the newly clicked element.

Files to edit
---
- `src/gramf_ui/ui_graph.rs`
- `src/gramf_ui/ui_input.rs` (new: input handling & selection state)
- `bevy_utils/graph_entities.rs`
- `src/io/geojson.rs` (snapshot import/export helpers)
- `src/gramf_ui/ui_layout.rs` (inspector panel)
- `src/graphs/snapshot.rs` (new/extend SnapshotGraph metadata)

Acceptance criteria
---
- Snapshot-sourced visual properties appear correctly in the viewport.
- UI displays the current view source and time index (raw snapshot vs STG view vs full STG).
- Editing an edge property updates the graph resource and is tracked as a user edit with provenance (which view/time it came from).
- User can toggle edit target between snapshot export (GeoJSON) and STG overlay; the chosen target affects persistence semantics.
- Snapshot export/import as GeoJSON works end-to-end for a single time snapshot.
- Snapshot refresh is implemented without full entity respawn when possible (update-in-place).
- Ctrl+Left-Click computes shortest path on the current SnapshotGraph between the last-selected element and the clicked element.
- Highlighted path is visible immediately and can be extended by subsequent Ctrl+Left-Clicks.
- Path selection state is exposed to export (CSV/GeoJSON) and the inspector.
- Unit/integration tests exercise single-step selection, extension, and exporting the selected path.

Tasks
---
- [ ] Add source metadata and provenance fields to `SnapshotGraph`.
- [ ] Display view source/time and edit mode toggle in inspector UI.
- [ ] Implement GeoJSON export/import for snapshots and add tests.
- [ ] Ensure edits are recorded as `user_edits` with source/time provenance.
- [ ] Update spawn/update systems to read visual props and apply to entities.
- [ ] Implement a simple inspector UI to edit properties and observe provenance.
- [ ] Add integration tests or smoke tests for UI update and export/import behaviour.
- [ ] Add input handler for Ctrl+Left-Click and maintain a PathSelectionState (last_selected, current_path).
- [ ] Implement/plug a shortest-path routine (Dijkstra/A*) on SnapshotGraph for edge-weighted/edge-count metrics.
- [ ] Highlight path edges and support extension via consecutive Ctrl+Left-Clicks.
- [ ] Add tests for selection and extension behavior and for export of path data.

Notes
---
- Keep UI edits as small overlays (diffs) to simplify undo/redo and persistence.
- Snapshots exported as GeoJSON should include snapshot metadata (time_idx, origin, schema_version) to preserve provenance.
- Prefer explicit index-based snapshot import/export to avoid ambiguity in interpolation semantics.
- Consider a lightweight "what you see is what you edit" indicator to avoid accidental modification of raw data.
- Use index-based snapshots for deterministic shortest-path queries (avoid interpolation ambiguity).
- Consider modifier key + click variants later (e.g., Shift to add/remove single edges).

Complexity: Medium