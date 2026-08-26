# Sextant

A personal investment platform developed to learn Rust through a real-world project.

what this does: transaction history plus market data in, a portfolio snapshot out.

Goals

- Learn idiomatic Rust.
- Build a robust portfolio engine.
- Aggregate investment research.
- Deploy as a self-hosted application.

Current Stage

Phases 1–4 complete — domain model, traits, engine, and tests. Phase 5 (storage) is next.

The engine is stateless and pure: only assets and transactions are ever
persisted, while positions and snapshots are regenerated on demand and never
stored. That is what keeps every figure reproducible from history alone.

```
cargo test     # 65 tests: unit tests inline, integration tests in tests/
cargo run      # demo binary until Phase 7 replaces it with a real CLI
```

See [`docs/roadmap.md`](docs/roadmap.md) for phased progress and open decisions,
[`docs/domain.md`](docs/domain.md) for the domain model, and
[`docs/crate_structure.md`](docs/crate_structure.md) for module layout.
