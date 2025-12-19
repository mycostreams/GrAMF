# PR: Add interpolation & query APIs (slice / diff / growth) (0002)

Branch: feat/interpolation-query

Summary
---
Add interpolation and query APIs to SpatioTemporalGraph: float-time interpolation, index-based exact reads, slice/diff, and growth computation (raw deltas + per-time-unit rates).

Scope
---
- Implement float-time interpolation helpers (prefer numeric progress interpolation).
- Add index-based exact APIs (no interpolation).
- Add growth snapshot computation (deltas + rates) and unit tests.

Files to add / edit
---
- Edit: `src/graphs/mod.rs` / `src/graphs/stg.rs` (add interpolation & index APIs)
- Edit: `src/graphs/snapshot.rs` (Snapshot construction from float time or index)
- Add: `src/graphs/growth.rs` (GrowthSnapshot type & helpers)
- Add tests: `tests/test_interpolation.rs`, `tests/test_growth.rs`
- Update docs: `docs/issues/0002-interpolation-query-api.md`

Detailed checklist
---
- [ ] Create branch `feat/interpolation-query`.
- [ ] Implement `fn interpolate_edge_properties(&self, edge_id: EdgeId, time: f64) -> Option<EdgeProperties>`:
  - Use linear interpolation on numeric fields and prefer `progress` when present; fallback semantics documented.
  - Return computed `real_length = base_length(node_positions) * progress`.
- [ ] Implement index-based APIs: `edge_properties_at_index`, `snapshot_at_index`, `slice_indices`, `growth_between_indices`.
- [ ] Implement float-time snapshot constructor `snapshot_at(time: f64)` using interpolation.
- [ ] Add `GrowthSnapshot` type and helpers computing raw deltas and rates.
- [ ] Add unit tests for interpolation edge cases (exact keys, fractional, before/after bounds), growth deltas & rates, and index-based behavior.
- [ ] Add example fixtures for tests (`tests/fixtures/interp_fixture.json`).
- [ ] Run `cargo test`, fix issues, and add doc examples.

Suggested tests
---
- `test_interpolate_numeric_progress()` — verify linear interpolation of progress and real_length.
- `test_index_api_exact_match()` — ensure index API returns stored values.
- `test_growth_deltas_and_rates()` — check correct deltas and rates for length and width.

Commit messages
---
- "feat(graphs): add interpolation APIs and index-based snapshots"
- "feat(graphs): add GrowthSnapshot and rate helpers"
- "test: add interpolation & growth unit tests"

How to test locally
---
- `cargo test --lib` and run the new unit tests.
- Load sample fixture in REPL or examples to verify snapshot outputs.

Notes
---
- Keep interpolation behavior deterministic and documented; index-based APIs recommended when determinism required.
- Complexity: Medium → High
