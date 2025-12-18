# Issue: Path selection & export (CSV)

Background
---
Users should be able to select and highlight a series of edges (a path), view variables along that path, and export per-edge data to CSV for external analysis.

Proposed change
---
- Add UI path selection (click-to-add edges or select a precomputed path between nodes).
- Provide a view showing variables along the path (table/plot).
- Add CSV export utility and an export UI action.

Files to edit/add
---
- `src/gramf_ui/ui_graph.rs` (selection interactions)
- `src/gramf_ui/ui_layout.rs` (path inspector)
- `tools/export.rs` (CSV writer utility)

Acceptance criteria
---
- User can select/clear a path and see an overlay highlight.
- The inspector shows a table with per-edge variables and a `Export CSV` button that writes a file with timestamped values.
- Unit tests for path-to-csv writer ensure correct CSV format.

Tasks
---
- [ ] Implement path selection interactions
- [ ] Implement path inspector and plotting UI (basic)
- [ ] Implement CSV export and tests

Complexity: Low

Notes
---
Include metadata in the export (time_keys used, units) to keep CSVs self-describing.