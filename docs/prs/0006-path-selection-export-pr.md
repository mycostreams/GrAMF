# PR: Path selection & export (CSV, path UI) (0006)

Branch: feat/path-selection-export

Summary
---
Add deterministic shortest-path selection interactions, path highlight state, path inspector view (table & simple plot), and CSV export that includes metadata and computed per-edge values (base_length, progress, real_length, width).

Scope
---
- Input handling: Ctrl+Left-Click shortest path selection & extension (maintain `PathSelectionState`).
- Path inspector UI for listing per-edge variables and plotting simple trends.
- CSV export utility with header metadata including snapshot/index and provenance.

Files to add / edit
---
- Edit: `src/gramf_ui/ui_input.rs` (handle Ctrl+Left-Click & PathSelectionState)
- Edit: `src/gramf_ui/ui_graph.rs` (highlighting path edges)
- Add: `src/graphs/shortest_path.rs` (Dijkstra/A* helper)
- Edit: `src/gramf_ui/ui_layout.rs` (path inspector & export button)
- Add: `tools/export.rs` or similar CSV writer + tests
- Add tests: `tests/test_shortest_path_selection.rs`, `tests/test_path_csv_export.rs`

Detailed checklist
---
- [ ] Create branch `feat/path-selection-export`.
- [ ] Implement `PathSelectionState` in UI input system; track last selected and current path.
- [ ] Implement shortest-path routine on `SnapshotGraph` (index-based snapshot inputs recommended for determinism).
- [ ] Implement Ctrl+Left-Click behavior: compute shortest path and set/extend `current_path`.
- [ ] Add highlighting system for path edges and inspector UI listing per-edge values.
- [ ] Implement CSV export with metadata header and tests verifying format/content.
- [ ] Add unit/integration tests for selection behaviors (single path, extension) and export correctness.

Suggested tests
---
- `test_shortest_path_basic()` — verifies shortest path between two nodes/edges.
- `test_path_extension()` — verify path extended by subsequent Ctrl+Left-Clicks.
- `test_csv_export_contains_metadata_and_rows()` — export correctness.

Commit messages
---
- "feat(ui): add path selection input and PathSelectionState"
- "feat(path): add shortest-path routine and CSV export"
- "test: add path selection and export tests"

How to test locally
---
- Run `cargo test` and manually test in UI by selecting nodes/edges with Ctrl+Left-Click.

Notes
---
- Use index-based snapshots to avoid interpolation ambiguity for deterministic path calculations. Complexity: Low → Medium
