# PR: Add aligned time model and TimeSeries types (0001)

Branch: feat/time-series

Summary
---
Implement the aligned time model for STGs: add `time_keys: Vec<i64>` stored as UNIX seconds, introduce a `TimeSeries<T>` type for per-edge aligned arrays, provide migration helpers from the current sparse representation, and add unit tests and docs.

Motivation
---
This enables fast, deterministic interpolation, efficient storage, and simpler APIs for snapshots and growth computation.

Scope (what this PR will change)
---
- Add `TimeSeries<T>` type and helpers.
- Add `time_keys: Vec<i64>` to `SpatioTemporalGraph` and constructors.
- Add helpers to compute `base_length` on-demand from node positions (cache optional).
- Add migration utilities to convert current per-edge sparse maps into aligned `TimeSeries` arrays during load.
- Add unit tests and fixtures.
- Update docs and issue references.

Files to add / edit (concrete)
---
- Add: `src/graphs/types.rs` (new: `TimeSeries<T>` implementation)
- Edit: `src/graphs/mod.rs` or `src/graphs/stg.rs` (add `time_keys`, update edge struct to hold `TimeSeries<EdgeProperties>`)
- Edit: `src/io/loader.rs` (or loader entry) to support migration from sparse maps
- Add: `tests/test_time_series.rs` (unit tests for TimeSeries)
- Add: `tests/fixtures/sample_stg_timeseries.json` (small fixture demonstrating aligned arrays)
- Edit: `docs/issues/0001-add-time-model-time-series.md` (mark PR checklist / update acceptance criteria)

Detailed checklist (PR tasks)
---
- [ ] Create branch `feat/time-series` and add initial commit
- [ ] Implement `TimeSeries<T>` in `src/graphs/types.rs` with API:
  - `fn new(len: usize) -> Self`
  - `fn get(&self, idx: usize) -> Option<&T>`
  - `fn set(&mut self, idx: usize, value: Option<T>)`
  - `fn iter(&self) -> impl Iterator<Item = Option<&T>>`
  - `fn slice(&self, i0: usize, i1: usize) -> TimeSeries<T>`
  - `fn from_sparse_map(map: &HashMap<i64, T>, time_keys: &Vec<i64>) -> Self` (migration helper)
- [ ] Add `time_keys: Vec<i64>` to `SpatioTemporalGraph` and update constructors and `load_from_geojson`/loader code to populate it from input files (parse `time_keys` if present; fall back to collected timestamps but prefer explicit `time_keys` when present).
- [ ] Replace edge property storage to use `TimeSeries<EdgeProperties>` for time-varying fields; keep backward-compatible loader that accepts old `HashMap<i64, EdgeProperties>` and converts it (migration helper).
- [ ] Add helper `fn base_length(&self, edge: &Edge) -> f64` on STG that computes distance between node positions (do not persist base_length in files).
- [ ] Add unit-tests (`tests/test_time_series.rs`): coverage for creation, get/set, slice, conversion from sparse map with mismatched time keys, and edge cases (out-of-range).
- [ ] Add integration loader test that loads legacy sparse-format fixture and verifies migration produces aligned `TimeSeries` and correct `time_keys`.
- [ ] Add docs and examples: update `docs/issues/0001-add-time-model-time-series.md` and `docs/Architecture.md` referencing the implementation and showing example JSON fixture.
- [ ] Run `cargo test`, `cargo fmt`, `cargo clippy` and fix issues.

Suggested tests to add (file & content)
---
- `tests/test_time_series.rs`
  - test_new_and_get_set()
  - test_slice()
  - test_from_sparse_map_exact_keys()
  - test_from_sparse_map_missing_keys_fills_none()
- `tests/test_loader_migration.rs`
  - test_load_legacy_sparse_geojson()` loads fixture and asserts `time_keys` and `TimeSeries` alignment

Suggested commit messages
---
- "feat(graphs): add TimeSeries<T> type and aligned time_keys storage"
- "fix(loader): migrate legacy per-edge sparse maps to aligned TimeSeries during load"
- "test: add TimeSeries and loader migration tests"

Review checklist
---
- [ ] Code compiles and tests pass locally
- [ ] API shapes (`TimeSeries`) are ergonomic and documented
- [ ] Backward compatibility for existing datasets is covered by tests
- [ ] Docs updated and sample fixtures included

How to test locally
---
- Run `cargo test` to execute unit tests
- `cargo fmt` and `cargo clippy` for style & lint
- Load the sample fixture in the app (`cargo run --example load_fixture` or run unit tests for loader)

Notes & open questions
---
- We compute `base_length` from node positions on demand; caching can be added later if profiling indicates need.
- Time keys are `i64` seconds (UNIX epoch). If sub-second precision is required later, we can migrate to `f64`/i128 ns.

Estimated complexity: Medium

Request review from: @you, @repo-maintainer, or domain experts for data model and IO handling.

---

If this looks good I can prepare a draft branch and open a draft PR with the changes and tests in small commits.
