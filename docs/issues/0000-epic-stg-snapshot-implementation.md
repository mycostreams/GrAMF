# Epic: STG & Snapshot Implementation

Summary
---
This epic tracks the work to implement the STG and snapshot features from the Architecture doc: explicit time-series support, interpolation/slicing APIs, snapshot/UI integration, edit-tracking and persistence, compute hooks, path export, tests, and optional large-dataset lazy-loading.

Related Issues
---
- 0001 - Add aligned time model and TimeSeries types
- 0002 - Add interpolation and query APIs (slice/diff/growth)
- 0003 - Snapshot & UI integration (visualization & editing)
- 0004 - Edit-tracking and persistence layer
- 0005 - Compute & analytics hooks (simulations, centrality)
- 0006 - Path selection & export (CSV)
- 0007 - Lazy-loading and large dataset support
- 0008 - Tests, fixtures & docs

Acceptance criteria
---
- All sub-issues are created and actionable (checklists defined).
- A minimal integration branch can run a full `cargo build` and unit tests for new functionality (incremental work accepted).
- A short README/documentation page is produced describing the schema and APIs.

Tasks
---
- [ ] Create and triage sub-issues
- [ ] Prioritize and assign initial implementation (data model + API first)
- [ ] Start PRs for step 1 and step 2

Complexity: High
