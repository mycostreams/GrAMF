# Issue: Add interpolation and query APIs (slice/diff/growth)

Background
---
Snapshots currently require exact timestamp matches. We need interpolation and a small query API to support fractional timestamps, slicing, diffs, and growth computation.

Proposed change
---
- Add `fn interpolate_edge_properties(&self, edge_id, time: f64) -> Option<EdgeProperties>`.
  - When returning interpolated properties include computed `real_length = base_length * progress`.
- Add `fn snapshot_at(&self, time: f64) -> SnapshotGraph` that uses interpolation and that computes per-edge real lengths from `base_length` and interpolated progress.
- Add `fn slice(&self, t0: f64, t1: f64) -> SpatioTemporalGraph` & `fn diff(t0,t1)`.
- Add `fn growth_between(&self, t0: f64, t1: f64) -> GrowthSnapshot` with growth per-edge computed as:
    growth(edge) = base_length * (progress(t1) - progress(t0))
  and source/target markings.

Exact index API (no interpolation)
---
- Add `fn edge_properties_at_index(&self, edge_id, idx: usize) -> Option<EdgeProperties>` — returns the stored value at the exact time index or `None` if missing. Also expose `real_length_at_index = base_length * progress_at_index`.
- Add `fn snapshot_at_index(&self, idx: usize) -> SnapshotGraph` — builds a snapshot using the exact aligned `time_keys[idx]`.
- Add `fn slice_indices(&self, i0: usize, i1: usize) -> SpatioTemporalGraph` — index-aligned slice (inclusive/exclusive semantics documented).
- Add `fn growth_between_indices(&self, i0: usize, i1: usize) -> GrowthSnapshot` — growth computed from exact indices without interpolation using base_length * (progress1 - progress0).

Files to edit
---
- `src/graphs/mod.rs` or `stg.rs` (methods on `SpatioTemporalGraph`)
- `src/graphs/snapshot.rs` (SnapshotGraph behavior)
- `tests/test_interpolation.rs` (new tests)
- `tests/test_index_api.rs` (new tests for index-based API)

Acceptance criteria
---
- Interpolation returns expected numeric interpolations in unit tests, and `real_length` matches `base_length * progress`.
- Fractional snapshot generation produces a `SnapshotGraph` with interpolated properties, activity flags, and computed `real_length`.
- Growth computation matches `base_length * (progress1 - progress0)` semantics and marks growth direction.
- Index-based API returns exact stored values with no interpolation and handles out-of-range indices deterministically (returns `None` or an `Err` as documented).

Tasks
---
- [ ] Implement interpolation helper (linear numeric interpolation policy).
- [ ] Decide and document boolean semantics for `active`.
- [ ] Write unit tests for interpolation, edge-case (before/after bounds), and growth.
- [ ] Implement index-based API functions and add tests for exact-match semantics and out-of-range handling.
- [ ] Update docs to show both float-time and index-based usage examples.


Tasks / Tests to add
---
- [ ] Unit tests for TimeSeries conversion and alignment.
- [ ] Unit tests for interpolation behavior.
- [ ] Unit tests that verify `real_length = base_length * progress` across indices and fractional times.
- [ ] Tests for growth computation using `base_length * (progress1 - progress0)`.
- [ ] Fixtures: small STG examples where `base_length` is provided and time_data contains only state/progress/width; expected CSV exports should include computed real lengths.


Complexity: Medium → High

Notes
---
Document policy for booleans and out-of-range times (clamp vs None vs step). Document that index-based functions are recommended when callers require deterministic, non-interpolated reads.