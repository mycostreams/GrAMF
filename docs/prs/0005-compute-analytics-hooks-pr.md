# PR: Compute & analytics hooks (jobs) (0005)

Branch: feat/compute-jobs

Summary
---
Add asynchronous compute job API and sample implementations (betweenness centrality + hydraulic solver stub). Jobs accept `Snapshot` or `GrowthSnapshot`, return node/edge-indexed arrays of computed scalars that are added back as properties with provenance.

Scope
---
- Add `src/graphs/compute.rs` job API and job queue/executor integration.
- Add sample algorithms and tests (betweenness centrality + hydraulic stub).
- Add hooks to attach computed arrays to nodes/edges as properties with provenance.

Files to add / edit
---
- Add: `src/graphs/compute.rs` (job API + job types)
- Edit: `src/graphs/mod.rs` (apply computed properties to graph)
- Add tests: `tests/test_betweenness.rs`, `tests/test_compute_property_apply.rs`
- Edit UI: `src/gramf_ui/ui_layout.rs` to add simple compute job launcher (optional)

Detailed checklist
---
- [ ] Create branch `feat/compute-jobs`.
- [ ] Design job API: `ComputeJobInput = Snapshot | GrowthSnapshot`, `ComputeJobResult = NodeArray<Option<f64>> | EdgeArray<Option<f64>>` with provenance metadata.
- [ ] Implement async job queue/executor (threadpool) and safe result application into ECS.
- [ ] Implement betweenness centrality (unit-tested) returning node-indexed scalars.
- [ ] Implement hydraulic solver stub returning edge-indexed flow values and tests that apply results as edge properties.
- [ ] Add tests verifying computed arrays are attached to nodes/edges with provenance.

Suggested tests
---
- `test_betweenness_small_graph()` — verify expected centrality values.
- `test_compute_result_apply()` — verify job results persisted as node/edge properties with provenance.

Commit messages
---
- "feat(compute): add job API and async executor"
- "feat(stats): add betweenness centrality implementation"
- "test: add compute job result application tests"

How to test locally
---
- Run `cargo test` and also run the compute job examples in a non-blocking thread to ensure results are applied correctly.

Notes
---
- Keep heavy compute off the main thread and apply via ECS events. Add cancellation / job status tracking if needed. Complexity: Medium
