# Issue: Edit-tracking and persistence layer

Background
---
Users must be able to modify topology and temporal variables and persist these changes separately from raw-data imports. Edits should be tracked with metadata to allow merging, reverting, and provenance.

Proposed change
---
- Add an `EditLayer` or `user_edits` overlay to STG representation (small diff objects: edge_id/node_id, field, old_value, new_value, metadata).
- Extend I/O to serialize both `raw` and `user_edits`, and to load & merge overlays on demand.
- Add CLI/UI 'Save' and 'Export merged' options.

Files to edit
---
- `src/graphs/mod.rs` (STG representation)
- `src/io/*` (serde and file format)
- `src/gramf_ui/ui_layout.rs` (save/export UI entries)

Acceptance criteria
---
- Edits are persisted in a separate section of saved files with `schema_version` and `author` metadata.
- Loading the dataset applies overlays on top of `raw` data by default with a configurable option to ignore overlays.
- A simple undo of the last edit works in the UI.

Tasks
---
- [ ] Define `Edit` schema and storage model
- [ ] Add serialization/deserialization support
- [ ] Add UI save/export actions
- [ ] Add tests for round-trip persist/restore of edits

Complexity: Low → Medium

Notes
---
Use small JSON-friendly diff objects to keep patch files easy to review and merge.