# Issue: Lazy-loading and large dataset support

Background
---
Large STG datasets require memory- and IO-efficient strategies. Lazy-loading (on-demand), memory mapping, or chunked IO will help scale to multi-GB datasets.

Proposed change
---
- Introduce `TimeDataHandle` enum: `InMemory`, `Lazy(FileRef)`, `Mmap(Arc<Mmap>)`.
- Add background loader API to fetch `TimeSeries` chunks on-demand and integrate with ECS via futures or thread-pool.
- Define a simple compact on-disk layout (index + per-edge blocks) or recommend memory-mapped NDJSON/binary layout.

Files to edit/add
---
- `src/io/loader.rs` (new or extend loader)
- `src/graphs/mod.rs` (support `TimeDataHandle` in `Edge` definitions)
- `tests/test_lazy_load.rs` (mock loader tests)

Acceptance criteria
---
- STG topology loads quickly while time-series data can be loaded lazily.
- Background loading does not stall the Bevy main thread.
- Mocked lazy-loader tests ensure API correctness and correct resolution of data on-demand.

Tasks
---
- [ ] Define `TimeDataHandle` type and APIs
- [ ] Implement a mock loader for tests
- [ ] Implement a memory-mapped loader option (optional)
- [ ] Add integration tests for background load and UI update behavior

Complexity: High

Notes
---
This is optional for MVP; prioritize after core APIs and UI editing are stable.