# Issue: Add aligned time model and TimeSeries types

Background
---
We need a graph-level `time_keys` ordered list and per-edge aligned `TimeSeries<T>` to enable efficient interpolation, slicing, and compact storage. Current code stores per-edge timestamp -> props as sparse maps which complicates interpolation and performance.

Proposed change
---
- Add `time_keys: Vec<i64>` to `SpatioTemporalGraph`.
- Introduce `TimeSeries<T>` (dense `Vec<Option<T>>` aligned to `time_keys`) or a `SparseSeries<T>` wrapper type.
- Add migration helpers from existing edge `HashMap<timestamp, props>` to aligned arrays.
- `base_length` is derived from node positions (computed on demand) rather than required to be stored. Real length at any time is computed as `base_length(node_positions) * progress(t)` where `progress(t)` is derived from the per-index `EdgeState`/progress.

Files to edit
---
- `src/graphs/mod.rs` / `src/graphs/stg.rs` (or the current STG struct file)
- `src/graphs/types.rs` (new file for `TimeSeries` type)
- Any serde / I/O helpers in `src/io/*` for persistence

Acceptance criteria
---
- `SpatioTemporalGraph` exposes `time_keys` and edges hold `TimeSeries<EdgeProperties>` for time-varying fields.
- `base_length` is computed from node positions on demand; code provides a helper to compute and optionally cache `base_length` per-edge.
- Unit tests convert sparse maps to `TimeSeries` and back with no data loss and verify `real_length` is computed as `base_length(node_positions) * progress`.
- `time_keys` are stored as i64 seconds (UNIX epoch seconds).

Tasks
---
- [ ] Define `TimeSeries<T>` API (get, set, iterate, slice).
- [ ] Add `time_keys` to STG struct and constructors; add helper(s) to compute per-edge `base_length` from node positions (cache optional).
- [ ] Implement migration utilities for loading old sparse maps and for extracting progress-based length from old length values.
- [ ] Add unit tests for `TimeSeries` behavior and `real_length` computation (including growth delta and rate tests).
- [ ] Ensure serialization/deserialization uses i64 seconds for `time_keys`.

Complexity: Medium

Notes
---
Store time_keys as i64 seconds (UNIX epoch). Use `f64` only if sub-second fractional timestamps are required; document units and epoch explicitly in the schema.