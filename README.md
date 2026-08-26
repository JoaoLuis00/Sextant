# Investment Intelligence Platform

A personal investment platform developed to learn Rust through a real-world project.

Goals

- Learn idiomatic Rust.
- Build a robust portfolio engine.
- Aggregate investment research.
- Deploy as a self-hosted application.

Current Stage

Phases 1–4 complete — domain model, traits, engine, and tests. Phase 5 (storage) is next.

The core is a one-way flow: transaction history plus market data go into a
stateless engine, and a portfolio snapshot comes out. Only assets and
transactions are ever persisted; positions and snapshots are always
regenerated, never stored.

```
cargo test     # 64 tests: unit tests inline, integration tests in tests/
cargo run      # demo binary until Phase 7 replaces it with a real CLI
```

See [`docs/roadmap.md`](docs/roadmap.md) for phased progress and open decisions,
[`docs/domain.md`](docs/domain.md) for the domain model, and
[`docs/crate_structure.md`](docs/crate_structure.md) for module layout.
