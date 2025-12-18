# Issue: Snapshot & UI integration (visualization & editing)

Background
---
Snapshots should be representable and editable in the UI. Currently, UI rendering uses static geometry and ignores time-varying width/active flags.

Proposed change
---
- Extend `SnapshotGraph` to contain visual properties (width, active, growth values when relevant).
- Update UI systems (`src/gramf_ui/ui_graph.rs`, `bevy_utils/graph_entities.rs`) to render `width` and `active` and to update entities when snapshots change.
- Add basic edit-inspector UI: select edge → edit numeric property → update `user_edits` overlay.

Files to edit
---
- `src/gramf_ui/ui_graph.rs`
- `bevy_utils/graph_entities.rs`
- `src/gramf_ui/ui_layout.rs` (inspector panel)

Acceptance criteria
---
- Snapshot-sourced visual properties appear correctly in the viewport.
- Editing an edge property updates the graph resource and is tracked as a user edit.
- Snapshot refresh is implemented without full entity respawn when possible (update-in-place).

Tasks
---
- [ ] Extend `SnapshotGraph` type to include visuals
- [ ] Update spawn/update systems to read visual props and apply to entities
- [ ] Implement a simple inspector UI to edit properties
- [ ] Add integration tests or smoke tests for UI update behaviour

Complexity: Medium

Notes
---
Keep UI edits as small overlays (diffs) to simplify undo/redo and persistence.