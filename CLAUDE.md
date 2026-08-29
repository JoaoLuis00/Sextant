# Sextant

Personal investment platform, built to learn idiomatic Rust. Stateless/pure
engine: only `Asset`s and `Transaction`s are persisted — positions and
snapshots are always regenerated from history, never stored.

## Docs

- [`docs/roadmap.md`](docs/roadmap.md) — phased plan, what's done, open decisions
- [`docs/domain.md`](docs/domain.md) — domain model and types
- [`docs/crate_structure.md`](docs/crate_structure.md) — module layout

Check the roadmap before starting work to see which phase is active and
which language concepts it's meant to exercise.

## Commands

```
cargo test                                # unit tests inline, integration tests in tests/
cargo test --features storage,market_data # also exercises SQLite and Yahoo Finance quote fetching
cargo run                                 # demo binary until Phase 7 replaces it with a real CLI
```

## Conventions

- Monetary values use `rust_decimal::Decimal` — never `f64`.
- IDs are newtypes over `Uuid`: `TransactionId`/`PortfolioId` use UUIDv7
  (time-ordering); `AssetId` uses UUIDv5, deterministically derived from
  `(ticker, exchange)` via `AssetId::for_ticker` — the only way to construct
  one.
- Domain logic stays pure — no side effects in `impl` methods.
- No tautological tests (set a value, immediately assert it back) — they
  don't catch anything.
- Comments only where the why isn't obvious from the code; don't over-comment.
- Prefer the simplest implementation that works; don't overcomplicate.
- Don't commit without user review.
- Commit messages: no `Claude-Session:` link, keep the body short (summarize,
  don't narrate).
