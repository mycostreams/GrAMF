# Issue: Compute & analytics hooks (simulations, centrality)

Background
---
Users need to run graph computations such as hydraulic simulations and centrality metrics. These should be callable as jobs with inputs from STG/snapshots and outputs visible in the UI.

Proposed change
---
- Add a `src/graphs/compute.rs` module with a job API for running simulations and metrics asynchronously.
- Provide sample implementations: betweenness centrality and a small hydraulic solver stub (pluggable backends allowed).
- Jobs accept either a single `Snapshot` or a `GrowthSnapshot` as input and return node-indexed and/or edge-indexed arrays of computed scalar values.
  - Computed arrays are applied back onto the graph: each computed scalar becomes a property on the corresponding node or edge (e.g., `betweenness`, `flow`, `pressure`).
- Add UI to launch jobs and view results (plots, overlays).

Files to edit/add
---
- `src/graphs/compute.rs` (new)
- `src/graphs/snapshot.rs` (ensure `Snapshot` / `GrowthSnapshot` types are usable as job inputs)
- `src/graphs/mod.rs` (hooks to apply computed arrays as node/edge properties)
- `src/gramf_ui/ui_layout.rs` and `src/gramf_ui/ui_graph.rs` (launch UI + result display)
- tests (unit tests for algorithm correctness and property application)

Acceptance criteria
---
- API to run a compute job is available and returns results asynchronously.
- Job inputs can be a single `Snapshot` or `GrowthSnapshot`.
- A sample betweenness implementation is present and tested.
- Job results are stored back onto all affected nodes/edges as named properties and are visible in the UI overlays/plots.
- UI can run the job and display a simple result overlay.

Tasks
---
- [ ] Define job API and result types (accept `Snapshot` or `GrowthSnapshot`).
- [ ] Implement betweenness centrality and unit tests that verify results are written to node properties.
- [ ] Add hydraulic solver stub and tests that write edge-indexed outputs (flows/pressures) back to edge properties.
- [ ] Implement property-apply utilities to attach computed arrays to nodes/edges with provenance metadata.
- [ ] Add UI glue to launch jobs, show status, and visualize results (plots/overlays).
- [ ] Add integration tests verifying compute → property application → UI visibility.

Complexity: Medium

Notes
---
- Heavy compute should execute off the main thread and push results back into ECS safely.
- Include provenance metadata (job id, input snapshot/time, timestamp) when storing computed properties so results are auditable and reversible.