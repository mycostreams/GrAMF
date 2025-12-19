# PR: Snapshot model & non-interactive UI integration (0003)

Branch: feat/snapshot-ui-model

Summary
---
Implement `SnapshotGraph` data model & metadata, inspector-only UI hooks (non-interactive), GeoJSON snapshot import/export, and in-place snapshot update mechanisms (no low-level input handlers in this PR).

Scope
---
- Define `SnapshotGraph` type and `SnapshotMetadata`.
- Implement GeoJSON snapshot importer/exporter (metadata-preserving).
- Implement inspector-level edit application (creates `user_edits` overlay entries).
- Add in-place entity update helpers to apply snapshot changes without full respawn.

Files to add / edit
---
- Add/Edit: `src/graphs/snapshot.rs` (SnapshotGraph & Metadata)
- Edit: `src/io/geojson.rs` (snapshot import/export)
- Edit: `bevy_utils/graph_entities.rs` (in-place update helpers)
- Edit: `src/gramf_ui/ui_layout.rs` (inspector panel glue)
- Add tests: `tests/test_snapshot_export.rs`, `tests/test_inspector_overlay.rs`
- Update docs: `docs/issues/0003-snapshot-ui-integration.md`

Detailed checklist
---
- [ ] Create branch `feat/snapshot-ui-model`.
- [ ] Implement `SnapshotMetadata` (source enum, origin_id, schema_version, provenance, edit_overlay_applied).
- [ ] Implement `SnapshotGraph` shape; constructors from STG (index & float time) with resolved visual props (`width`, `state`, `progress`, `real_length` computed on demand).
- [ ] Add GeoJSON exporter/importer preserving `SnapshotMetadata` and snapshot properties; support loading as viewport snapshot or registering as a user edit overlay on request.
- [ ] Add inspector API to view snapshot metadata and to create `UserEdit` entries (sidecar overlay entries).
- [ ] Add in-place entity update helpers and unit tests that verify update-in-place vs respawn.
- [ ] Add tests for GeoJSON import/export round-trip and inspector overlay application.

Suggested tests
---
- `test_snapshot_geojson_roundtrip()` — preserves metadata and properties.
- `test_inspector_creates_user_edit()` — editing via inspector creates a sidecar overlay entry and does not mutate raw data.
- `test_update_in_place_behaviour()` — applying snapshot updates components without respawn count increase.

Commit messages
---
- "feat(snapshot): add SnapshotGraph and SnapshotMetadata"
- "feat(io): add GeoJSON snapshot import/export with metadata"
- "test: add snapshot import/export and inspector tests"

How to test locally
---
- `cargo test --lib` for new tests
- Try export/import example via provided utility (if present)

Notes
---
- Interaction-level input handling (shortcuts, path-selection) will be implemented in a separate PR (see `0006`).
- Complexity: Medium
