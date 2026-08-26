# Crate structure

Current layout (Phases 1–4). `app/` is planned but not yet created — it lands
with Phases 5–7.

The crate is named `sextant` (the app's name); module and file names stay
domain-oriented, so `domain/portfolio.rs` and the `Portfolio` type are
unchanged.

```
sextant/
├── Cargo.toml
├── tests/
│   └── portfolio_lifecycle.rs  # integration tests — public API only, via lib.rs
└── src/
    ├── main.rs                 # thin demo binary; becomes the clap CLI in Phase 7
    ├── lib.rs                  # module tree + flat re-exports of the public API
    │
    ├── domain.rs
    ├── domain/
    │   ├── asset.rs            # Asset, Ticker, Currency, AssetType
    │   ├── transaction.rs      # Transaction, TransactionType
    │   ├── position.rs         # Position, PositionValuation
    │   ├── snapshot.rs         # PortfolioSnapshot
    │   ├── errors.rs           # TransactionError, AssetError, DomainError
    │   └── portfolio.rs        # split into mutations/queries later if it grows
    │
    ├── engine.rs
    ├── engine/
    │   ├── portfolio_engine.rs # generate_snapshot, build_holdings
    │   ├── repository.rs       # Repository<T> port + InMemoryTransactionRepository
    │   ├── dispatch.rs         # static vs. dynamic dispatch comparison
    │   └── errors.rs           # EngineError
    │
    ├── market_data.rs          # MarketData, MarketDataProvider port, MockProvider
    │
    ├── app.rs                  # PLANNED — pub mod cli, storage, market_data;
    ├── app/                    # PLANNED
    │   ├── cli.rs              # Phase 7
    │   ├── storage.rs          # Phase 5 — implements Repository<T> against SQLite
    │   └── market_data.rs      # Phase 6 — YahooFinanceProvider
    │
    ├── ids.rs                  # crate-global — used by domain, engine, and app alike
    └── errors.rs               # top-level Error, composes Domain/Engine/MarketData via #[from]
```

## Notes

- **`lib.rs` is the crate root; `main.rs` is a consumer of it.** Everything
  worth testing lives in the library, which is what lets `tests/` exercise the
  real public API rather than a copy of it.
- **Errors live next to the code that raises them**, and top-level `errors.rs`
  re-exports them all so callers have one import site.
- **`market_data.rs` is top-level, not under `domain/`** — prices come from
  outside the system and change on their own. The *port* stays here; concrete
  providers belong in `app/market_data.rs` once that exists. (`docs/domain.md`
  and the original version of this file disagreed on this; see the open
  decisions in `docs/roadmap.md`.)
- **`domain/` never imports from `engine/`, and neither imports from `app/`.**
  Dependencies point inward only.
