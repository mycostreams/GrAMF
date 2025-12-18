# Issue: Tests, fixtures & docs

Background
---
Robust tests and clear fixtures are required to maintain correctness (interpolation, growth, slice/diff, persistence, lazy-loading, UI updates).

Proposed change
---
- Add fixtures in `tests/fixtures/sample_stg.json` with sparse and dense time-data cases and growth scenarios.
- Add unit tests for `TimeSeries`, interpolation, growth, slice/diff, edit persistence, and CSV export.
- Add an examples directory with a small STG and scripts/docs describing the new schema and usage.

Files to edit/add
---
- `tests/fixtures/sample_stg.json`
- `tests/test_time_series.rs`
- `tests/test_interpolation.rs`
- `tests/test_growth.rs`
- `docs/architecture.md` (update with new schema examples)

Acceptance criteria
---
- CI runs unit tests for new modules and fixtures.
- Tests for interpolation handle edge cases and pass reliably.
- Documentation includes minimal example JSON and guide for using the new APIs.

Tasks
---
- [ ] Create sample fixtures
- [ ] Implement unit tests as above
- [ ] Add documentation pages and example usage

Complexity: Low → Medium

Notes
---
Focus tests first around the data model and interpolation to make later UI and IO changes easier to validate.