# Issue: Path selection & export (CSV)

Background
---
Users should be able to select and highlight a series of edges (a path), view variables along that path, and export per-edge data to CSV for external analysis. Shortest-path selection should be accessible via a simple modifier+click interaction.

Proposed change
---
- Add UI path selection (click-to-add edges or select a precomputed path between nodes).
- Add interaction: Ctrl+Left-Click computes the shortest path between the last-selected element and the clicked element and highlights it. Consecutive Ctrl+Left-Clicks extend the current path from its current end to the newly clicked element.
- Provide a view showing variables along the path (table and simple plot). Variables should include computed values such as:
  - base_length, progress, real_length (= base_length * progress), width, edge id, source/target.
- Add CSV export utility and an export UI action. CSVs must include metadata (time_keys / time_idx used, units, schema_version, export provenance).

Files to edit/add
---
- `src/gramf_ui/ui_graph.rs` (selection interactions / highlight)
- `src/gramf_ui/ui_input.rs` (input handling: Ctrl+Left-Click, selection state)
- `src/gramf_ui/ui_layout.rs` (path inspector: table/plot/export button)
- `src/graphs/snapshot.rs` (helpers for extracting per-edge variables for a chosen time/index)
- `tools/export.rs` (CSV writer utility)
- `src/graphs/shortest_path.rs` (Dijkstra/A* helper for SnapshotGraph)

Acceptance criteria
---
- Ctrl+Left-Click computes the shortest path on the current SnapshotGraph between the last-selected element and the clicked element.
- The computed path is highlighted immediately and can be extended by subsequent Ctrl+Left-Clicks.
- The inspector shows a table/plot of per-edge variables along the selected path.
- `Export CSV` writes a self-describing CSV with per-edge rows and metadata header (time_idx/time_keys used, units, schema_version, provenance).
- Deterministic path calculations use index-based snapshots (no interpolation ambiguity) and the UI exposes which time/index was used for the path/export.

Tasks
---
- [ ] Add input handler for Ctrl+Left-Click and maintain a PathSelectionState (last_selected, current_path).
- [ ] Implement/plug a shortest-path routine (Dijkstra/A*) on SnapshotGraph for edge-weighted/edge-count metrics.
- [ ] Support path extension via consecutive Ctrl+Left-Clicks and edge-by-edge selection toggles.
- [ ] Implement path inspector UI (table + simple plot) that lists per-edge variables including computed real_length.
- [ ] Implement CSV export with metadata and unit tests verifying format and contents.
- [ ] Add unit/integration tests for selection, extension behavior, variable extraction, and export correctness.

Notes
---
- Prefer index-based snapshots for deterministic shortest-path queries. If fractional-time queries are used, document interpolation policy and include the used fractional time in CSV metadata.
- CSV exports should include provenance (which snapshot/time/index, whether values include user edits) so analyses remain reproducible.
- Include `real_length = base_length * progress` in exports and plots to make growth/flow analyses straightforward.
Complexity: Low → Medium