# Roadmap: STG & Snapshot Implementation 🚀

TL;DR
---
A prioritized, incremental plan to implement the STG & snapshot architecture: start with the data model and tests (MVP foundation), then add interpolation & query APIs, snapshot model & non-interactive UI, edit overlays/persistence, compute jobs, path selection/export, and (optionally) lazy-loading for very large datasets. Each step is a small, reviewable PR with tests and docs.

Milestones (ordered)
---
1) Foundation — Time model & TimeSeries (0001) ✅
   - Goal: `time_keys: Vec<i64>` (UNIX seconds) + `TimeSeries<T>` aligned arrays and migration helpers from legacy sparse maps.
   - PR: `feat/time-series` (docs/prs/0001-add-time-model-time-series-pr.md)
   - Acceptance: unit tests & fixtures; loader migration; `cargo test` passes.
   - Est.: 3–7 days (Medium)

2) Interpolation & Query APIs (0002) ✅
   - Goal: float-time interpolation preferring numeric `progress`, index-based exact APIs (no interpolation), growth deltas & rates.
   - PR: `feat/interpolation-query` (docs/prs/0002-interpolation-query-api-pr.md)
   - Acceptance: deterministic tests for interpolation, index APIs, growth rates.
   - Est.: 3–7 days (Medium → High)

3) Snapshot model & non-interactive UI (0003) ✅
   - Goal: `SnapshotGraph` type + metadata/provenance, GeoJSON snapshot import/export, inspector-level editing hooks, in-place entity updates.
   - PR: `feat/snapshot-ui-model` (docs/prs/0003-snapshot-ui-integration-pr.md)
   - Acceptance: snapshot round-trip tests; inspector creates sidecar overlays without mutating raw data; in-place update tests.
   - Est.: 4–10 days (Medium)

4) Edit overlays & persistence (0004) ✅
   - Goal: `UserEdit` schema, sidecar overlay persistence (`.gramf-edits.json`), apply/unapply and simple undo.
   - PR: `feat/edit-overlays` (docs/prs/0004-edit-tracking-persistence-pr.md)
   - Acceptance: overlay round-trip tests; UI inspector integration.
   - Est.: 2–5 days (Low → Medium)

5) Compute jobs & analytics (0005)
   - Goal: async compute job API; sample betweenness and hydraulic stub; apply computed arrays as node/edge properties with provenance.
   - PR: `feat/compute-jobs` (docs/prs/0005-compute-analytics-hooks-pr.md)
   - Acceptance: unit tests for compute correctness; result application tests; non-blocking execution.
   - Est.: 4–10 days (Medium)

6) Path selection & export (0006)
   - Goal: Ctrl+Left-Click shortest-path selection & extension, path inspector, CSV exporter with metadata (time_idx, provenance) and computed per-edge values (base_length, progress, real_length, width).
   - PR: `feat/path-selection-export` (docs/prs/0006-path-selection-export-pr.md)
   - Acceptance: selection + extension tests; CSV content & metadata tests.
   - Est.: 3–7 days (Low → Medium)

7) Lazy-loading & large datasets (0007) — optional for MVP
   - Goal: `TimeDataHandle` (InMemory | Lazy | Mmap), background loader, LRU cache, memory-mapped loader example.
   - PR: `feat/lazy-loading` (docs/prs/0007-lazy-loading-large-datasets-pr.md)
   - Acceptance: loader tests (mock), cache eviction tests, UI responsiveness improvements in a sample large fixture.
   - Est.: 2–4 weeks (High)

8) Tests, fixtures & docs (ongoing) (0008)
   - Goal: comprehensive fixtures, unit & integration tests, and docs & examples.
   - PR: `feat/tests-fixtures` (docs/prs/0008-tests-fixtures-pr.md)
   - Acceptance: CI runs tests and docs updated with examples.
   - Est.: Ongoing (Low → Medium)

MVP Definition ✅
---
A minimal, releaseable MVP includes: milestones 1–4 (TimeSeries + interpolation + snapshot model + edit overlays) + unit tests and fixtures. This allows UI inspection, deterministic snapshot export/import, edit persistence, and deterministic queries.

Dependencies & sequencing
---
- 0002 depends on 0001 (TimeSeries). 0003 depends on 0001+0002. 0004 depends on 0003. 0006 requires snapshot model from 0003. 0007 can begin after 0001 (or in parallel after 0003).
- Tests & fixtures (0008) should be added incrementally as each PR lands.

Gating & CI
---
- Each PR must:
  - include unit tests for new logic and fixtures for edge cases.
  - pass `cargo test`, `cargo fmt`, and `cargo clippy`.
  - include docs updates (short example JSON and API usage).
- Prefer small, focused PRs that are easy to review and merge (no big, monolithic changes).

Risk & mitigations ⚠️
---
- Backwards compatibility: include migration helpers and tests to convert legacy sparse maps to aligned `TimeSeries`.
- UI performance: implement update-in-place and test spawn/update performance vs full respawn.
- Large-file parsing: recommend memory-mapped binary layout + lazy-loading to avoid long startup times.

Owners & reviewers
---
- Recommend small team: data model + IO owner (you or maintainer), UI/Bevy owner, compute/algorithms owner. Request reviews from domain experts for schema and IO changes.

Milestones & releases
---
- Do 1–2 PRs per week depending on complexity; get 0001 & 0002 merged first, then 0003 & 0004 to reach MVP within ~4–8 weeks for a single-developer velocity.

Next immediate action ▶️
---
- Create branch `feat/time-series` and implement PR 0001 (TimeSeries + tests + loader migration). I can start that now and open a draft PR with incremental commits if you want.

---

If you want, I can also add this roadmap as a top-level issue and link the PRs to it, or create a small project board to track status and assign owners. Which would you prefer?