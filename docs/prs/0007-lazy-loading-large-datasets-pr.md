# PR: Lazy-loading & large dataset support (0007)

Branch: feat/lazy-loading

Summary
---
Implement `TimeDataHandle`, background loader, LRU caching strategy and a memory-mapped loader option to support large STG files without blocking the UI.

Scope
---
- Add `TimeDataHandle` type and mock loader for tests.
- Implement background loading API integrated with ECS (thread pool / async tasks) and LRU cache behavior.
- Add tests and a sample binary layout or memory-mapped loader example.

Files to add / edit
---
- Add: `src/io/loader.rs` (loader & handle types)
- Edit: `src/graphs/mod.rs` (allow `TimeDataHandle` in edges)
- Add tests: `tests/test_lazy_loader.rs` (mocked I/O)
- Add: example layout or small binary fixture for testing

Detailed checklist
---
- [ ] Create branch `feat/lazy-loading`.
- [ ] Implement `TimeDataHandle` enum: `InMemory`, `Lazy(FileRef)`, `Mmap(Arc<Mmap>)`.
- [ ] Implement mock loader for tests and LRU cache with configurable chunk size.
- [ ] Integrate loader with ECS using background tasks and safe result application via events.
- [ ] Add unit/integration tests for on-demand load, cache eviction, and UI update behavior.
- [ ] Add a small example binary fixture and loader demo.

Suggested tests
---
- `test_mock_loader_returns_chunks()` — loader correctness.
- `test_cache_eviction_lru()` — ensure eviction policy.
- `test_background_load_updates_ecs()` — UI update path.

Commit messages
---
- "feat(io): add TimeDataHandle and mock lazy loader"
- "feat(io): add background loader + LRU cache"
- "test: add lazy loader unit tests"

How to test locally
---
- Run `cargo test` for loader tests.
- Run a small example demonstrating topology loads quickly and time-series loaded async.

Notes
---
- This is a larger and optional piece for MVP — prioritize after API + UI editing features. Complexity: High
