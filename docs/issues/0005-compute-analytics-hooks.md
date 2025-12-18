# Issue: Compute & analytics hooks (simulations, centrality)

Background
---
Users need to run graph computations such as hydraulic simulations and centrality metrics. These should be callable as jobs with inputs from STG/snapshots and outputs visible in the UI.

Proposed change
---
- Add a `src/graphs/compute.rs` module with a job API for running simulations and metrics asynchronously.
- Provide sample implementations: betweenness centrality and a small hydraulic solver stub (pluggable backends allowed).
- Add UI to launch jobs and view results (plots, overlays).

Files to edit/add
---
- `src/graphs/compute.rs` (new)
- `src/gramf_ui/ui_layout.rs` and `src/gramf_ui/ui_graph.rs` (launch UI + result display)
- tests (unit tests for algorithm correctness)

Acceptance criteria
---
- API to run a compute job is available and returns results asynchronously.
- A sample betweenness implementation is present and tested.
- UI can run the job and display a simple result overlay.

Tasks
---
- [ ] Define job API and result types
- [ ] Implement betweenness centrality and unit tests
- [ ] Add a simple hydraulic solver stub and tests
- [ ] Add UI glue to launch jobs and present results

Complexity: Medium

Notes
---
Heavy compute should execute off the main thread and push results back into ECS safely.