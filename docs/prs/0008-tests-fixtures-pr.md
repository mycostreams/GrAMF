# PR: Tests, fixtures & docs (0008)

Branch: feat/tests-fixtures

Summary
---
Add comprehensive fixtures, unit and integration tests for TimeSeries, interpolation, growth, persistence, lazy-loading (mock), GeoJSON snapshot import/export, and UI inspector flows.

Scope
---
- Add fixtures under `tests/fixtures/` including legacy sparse format and aligned time-series JSON.
- Add unit tests for `TimeSeries`, interpolation, growth, snapshot export/import, edit-overlay persistence.
- Add integration tests for loader migration and background load behavior (mocked).

Files to add / edit
---
- Add: `tests/fixtures/sample_stg_sparse.json`, `tests/fixtures/sample_stg_timeseries.json`
- Add tests: `tests/test_time_series.rs`, `tests/test_interpolation.rs`, `tests/test_growth.rs`, `tests/test_loader_migration.rs`, `tests/test_snapshot_export.rs`
- Update docs: `docs/issues/0008-tests-fixtures.md`

Detailed checklist
---
- [ ] Create branch `feat/tests-fixtures`.
- [ ] Add fixtures covering sparse and aligned time series and growth cases.
- [ ] Add unit tests for TimeSeries API and edge cases.
- [ ] Add tests for interpolation and index-api behaviors.
- [ ] Add snapshot GeoJSON export/import round-trip tests.
- [ ] Add loader migration test that converts legacy sparse fixture to aligned TimeSeries.
- [ ] Run CI locally and ensure tests pass.

Suggested tests (high-priority)
---
- `test_time_series_basic()` — creation / get / set / slice.
- `test_interpolation_progress_preferred()` — ensure progress-preferred interpolation avoids jumps.
- `test_growth_delta_and_rate()` — validate raw deltas and rates.
- `test_snapshot_geojson_roundtrip()` — preserves metadata and values.

Commit messages
---
- "test: add fixtures and unit tests for TimeSeries & interpolation"
- "test: add snapshot export/import and loader migration tests"

How to test locally
---
- `cargo test` runs full test suite including fixtures
- `cargo test -- --nocapture` for debug printing when needed

Notes
---
- Ensure CI runs these new tests and fixtures; update project CI config if test paths need inclusion. Complexity: Low → Medium
