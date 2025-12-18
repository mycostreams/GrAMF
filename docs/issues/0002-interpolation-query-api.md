# Issue: Add interpolation and query APIs (slice/diff/growth)

Background
---
Snapshots currently require exact timestamp matches. We need interpolation and a small query API to support fractional timestamps, slicing, diffs, and growth computation.

Proposed change
---
- Add `fn interpolate_edge_properties(&self, edge_id, time: f64) -> Option<EdgeProperties>`.
- Add `fn snapshot_at(&self, time: f64) -> SnapshotGraph` that uses interpolation.
- Add `fn slice(&self, t0: f64, t1: f64) -> SpatioTemporalGraph` & `fn diff(t0,t1)`.
- Add `fn growth_between(&self, t0: f64, t1: f64) -> GrowthSnapshot` with growth per-edge and source/target markings.

Files to edit
---
- `src/graphs/mod.rs` or `stg.rs` (methods on `SpatioTemporalGraph`)
- `src/graphs/snapshot.rs` (SnapshotGraph behavior)
- `tests/test_interpolation.rs` (new tests)

Acceptance criteria
---
- Interpolation returns expected numeric interpolations in unit tests.
- Fractional snapshot generation produces a `SnapshotGraph` with interpolated properties and activity flags.
- Growth computation matches `length(t1)-length(t0)` semantics and marks growth direction.

Tasks
---
- [ ] Implement interpolation helper (linear numeric interpolation policy).
- [ ] Decide and document boolean semantics for `active`.
- [ ] Write unit tests for interpolation, edge-case (before/after bounds), and growth.

Complexity: Medium → High

Notes
---
Document policy for booleans and out-of-range times (clamp vs None vs step).