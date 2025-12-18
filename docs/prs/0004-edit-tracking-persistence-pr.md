# PR: Edit-tracking & persistence (sidecar overlays) (0004)

Branch: feat/edit-overlays

Summary
---
Add sidecar edit overlay support: `UserEdit` schema, overlay application logic, serialization (sidecar `.gramf-edits.json`), undo/redo basics and tests.

Scope
---
- Introduce `UserEdit` type and `EditOverlay` storage.
- IO: read/write sidecar overlays and options to apply/ignore overlays on load.
- UI: inspector hooks to create overlays and simple undo.

Files to add / edit
---
- Edit: `src/graphs/mod.rs` (store & apply overlays)
- Add: `src/io/persistence.rs` (read/write sidecar overlays)
- Edit: `src/gramf_ui/ui_inspector.rs` or layout to call overlay APIs
- Add tests: `tests/test_edit_overlay.rs`
- Update docs: `docs/issues/0004-edit-tracking-persistence.md`

Detailed checklist
---
- [ ] Create branch `feat/edit-overlays`.
- [ ] Define `UserEdit` schema with provenance fields (author, timestamp, source_snapshot/time).
- [ ] Implement overlay storage on `SpatioTemporalGraph` and apply/unapply logic.
- [ ] Implement sidecar serialization/deserialization helpers and add CLI flags to load / ignore overlays.
- [ ] Wire inspector UI to emit `UserEdit` entries and add a simple undo stack for last edit.
- [ ] Add tests for overlay round-trip, apply/unapply semantics, and undo behavior.

Suggested tests
---
- `test_persist_overlay_roundtrip()` — save + load sidecar and ensure no data loss in edit diffs.
- `test_apply_unapply_overlay()` — applying overlays changes view state but does not mutate raw data; unapplying restores raw view.

Commit messages
---
- "feat(edit): add UserEdit overlay schema and persistence"
- "test: add overlay apply/unapply tests"

How to test locally
---
- `cargo test` for unit tests
- Manual test: create a snapshot, edit via inspector, save sidecar, reload with/without overlay applied

Notes
---
- Choose sidecar naming convention (e.g., `<dataset>.gramf-edits.json`).
- Complexity: Low → Medium
