# Sextant — Rust Learning Roadmap

A phased plan mapping Rust language concepts to milestones in the portfolio project.

---

## Phase 1 — Domain Modeling (structs, enums, impl blocks)

**Goal:** model the core business entities and get comfortable with structs, enums, and methods.

- [x] Define `Asset`, `Transaction`, `Position`, `Snapshot` as structs
- [x] Use `rust_decimal::Decimal` for all monetary values — never `f64`
- [x] Model `TransactionType` as a data-carrying enum, not a flat tag:
  ```rust
  enum TransactionType {
      Buy { quantity: Decimal, price: Decimal },
      Sell { quantity: Decimal, price: Decimal },
      Dividend { amount: Decimal },
      Split { ratio: Decimal },
  }
  ```
- [x] Build `ids.rs` using the newtype pattern (`struct AssetId(Uuid);`, `struct TransactionId(Uuid);`)
- [x] Add basic `impl` blocks with pure methods (no side effects), e.g. `Position::market_value(&self, price: Decimal) -> Decimal`
- [x] Derive `Debug`, `Clone`, `PartialEq` on domain types as needed

---

## Phase 2 — Traits

**Goal:** learn traits through real abstractions the project actually needs.

- [x] Define a `MarketDataProvider` trait:
  ```rust
  trait MarketDataProvider {
      fn price(&self, asset: &AssetId) -> Result<Decimal, MarketDataError>;
  }
  ```
- [x] Implement a `MockProvider` first (for testing), before touching a real API
- [x] Define a `Repository<T>` trait to decouple `engine` from SQLite specifics
- [x] Decide: does `engine` hold `Box<dyn MarketDataProvider>` (dynamic dispatch) or is it generic over `P: MarketDataProvider` (static dispatch)? Try both, understand the trade-off
- [x] Implement `std::error::Error` for your error types (or use `thiserror` to derive it)

---

## Phase 3 — Ownership, Borrowing, Iterators, Collections

**Goal:** internalize ownership rules and iterator adaptors through engine logic.

- [x] Decide `apply_transaction` signature: `&mut self` (mutate in place) vs consume-and-return-new (`fn apply_transaction(self, tx: Transaction) -> Portfolio`)
- [x] Store holdings in `HashMap<AssetId, Position>`
- [x] Compute total portfolio value using `.iter().map().sum()` style iterator chains
- [x] Practice borrowing patterns: when to take `&T` vs `&mut T` vs `T` in engine functions
- [x] Get comfortable with `Result`/`Option` combinators (`?`, `.map()`, `.and_then()`, `.unwrap_or_default()`)

---

## Phase 4 — Testing

**Goal:** exploit the fact that domain/engine are pure and decoupled — test them thoroughly.

- [x] Write unit tests for cost-basis calculation
- [x] Write unit tests for realized/unrealized P&L
- [x] Write unit tests for holdings aggregation from a transaction list
- [x] Keep tests inline via `#[cfg(test)] mod tests { ... }` at the bottom of each file
- [x] Write tests *as you write each engine function*, not after — catches bad enum/struct modeling early
- [x] Add integration tests in `tests/` that exercise only the public API via `lib.rs`

---

## Phase 5 — Storage

**Goal:** persist data without dragging in async before you're ready.

- [x] Start with `rusqlite` (sync) rather than `sqlx` (async) — one less concept to fight
- [x] Implement the `Repository<T>` trait from Phase 2 against SQLite
- [x] Keep `domain`/`engine` completely unaware of SQL — only `storage` module touches `rusqlite`
- [x] Feature-gate `storage` in `Cargo.toml` (`dep:rusqlite` as optional dependency)

---

## Phase 6 — Market Data

**Goal:** implement the trait from Phase 2 against a real external API.

- [x] Implement `YahooFinanceProvider: MarketDataProvider`, backed by the
      `yahoo_finance_api` crate (its `blocking` feature) rather than a
      hand-rolled `reqwest` + `serde` client — see the open decision below
      for why
- [x] Handle API failures gracefully (`Result<_, MarketDataError>`, retries/timeouts later)
- [x] Feature-gate `market_data` in `Cargo.toml`

---

## Phase 7 — CLI

**Goal:** thin presentation layer over an already-tested core.

- [ ] Use `clap` for argument parsing
- [ ] `main.rs` stays thin — just parses args and calls into the lib
- [ ] Commands should map directly to engine functions already covered by tests
- [ ] Feature-gate `cli` in `Cargo.toml`

---

## Open decisions for review

Raised while implementing Phases 3–4; none block Phase 5, but each is a
judgement call worth confirming rather than inheriting silently.

- **UUIDv7 needs the monotonic constructor** (fixed, no action needed —
  recorded because the doc's stated guarantee quietly depended on it).
  `Uuid::new_v7(Timestamp::now(NoContext))` randomizes the sub-millisecond
  bits, so ids minted in the same millisecond sorted randomly and the
  "same-day ordering for free" design decision did not actually hold.
  Now `Uuid::now_v7()`, which uses a shared monotonic counter.
- **Oversell now fails loud.** Selling more than is held returns
  `EngineError::OversoldAsset` instead of silently producing a negative
  quantity. Consistent with the "fail loud" choice already made for missing
  market data, but it does mean v1 cannot represent short positions at all.
- **`Transaction.kind` vs `transaction_type`.** `docs/domain.md` names the
  field `kind`; the code uses `transaction_type`. Cosmetic — flagging so the
  doc and code get reconciled in one direction.
- **Where `market_data` lives.** `docs/domain.md` puts it at top-level
  `market_data.rs`; `docs/crate_structure.md` puts it at `app/market_data.rs`.
  Currently top-level, since `app/` doesn't exist yet. The reconciliation that
  probably works: the *port* (`MarketDataProvider`, `MarketData`) stays
  top-level, and concrete providers (Yahoo) land in `app/market_data.rs`.
- **`created_at` / `updated_at` on `Transaction`.** Gap #4 in `domain.md`, not
  implemented. Matters more than usual because editing a transaction silently
  changes every snapshot derived from it.
- **Single-currency assumption.** `Asset.currency` is now a `Currency` enum,
  but snapshot totals still carry no currency tag — mixing denominations would
  silently sum incomparable numbers. Documented on `PortfolioSnapshot`; needs
  an FX layer before it can be relaxed.
- **`Transaction::from_stored` bypasses id minting.** `Transaction::new`
  always mints a fresh id; a repository reloading a row needs to restore
  the existing one instead. `pub(crate)`-only, so external callers still go
  through `new()`.
- **`StorageError` doesn't widen into the top-level `Error`.** It wraps
  `rusqlite::Error`, which isn't `Clone`/`PartialEq` — can't join an enum
  that derives those. Revisit at Phase 7 when the CLI needs one error type.
- **`YahooFinanceProvider` uses the `yahoo_finance_api` crate, not a
  hand-rolled scraper.** First cut used `reqwest` + `serde` directly against
  Yahoo's chart endpoint. Switched early, since the crate already handles
  Yahoo's cookie/crumb auth churn, has a genuinely sync `blocking` mode, and
  its `decimal` feature skips our own `f64` conversion. Trade-off: a much
  bigger dependency tree, and less of our own HTTP/parsing code practiced.
- **`YahooFinanceProvider` carries its own ticker map.**
  `MarketDataProvider::price` takes only an `AssetId`, but Yahoo needs a
  ticker string. Provider holds `HashMap<AssetId, Ticker>` via
  `.with_ticker(...)` (mirrors `MockProvider::with_price`); an unmapped
  asset just returns `NotFound`.
- **Yahoo errors stringify into `MarketDataError`.** `YahooError` isn't
  `Clone`, so it's `.to_string()`'d into `FetchFailed`/`ProviderUnavailable`
  right away instead of wrapped with `#[from]`.
- **`AssetId` switched from random (UUIDv7) to deterministic (UUIDv5).**
  Found while wiring up storage: a random id meant re-creating the same
  asset next run gave a different id than what old transactions reference.
  `AssetId::for_ticker(ticker, exchange)` derives the same id every time, no
  persistence needed. `Asset::new` now calls it internally instead of taking
  an `id` parameter. Doesn't handle ticker rebrands (a renamed ticker still
  derives a new id) — deferred, since that's a rare event needing an
  explicit migration regardless of id scheme.
- **`AssetId::new()` (random UUIDv7) removed entirely.** Once `for_ticker`
  existed, keeping a random constructor around contradicted the invariant
  it was for — every real `AssetId` should trace back to a `(ticker,
  exchange)` pair, not be arbitrary. `for_ticker` is now the only way to
  construct one; ~60 test call sites that used `new()` purely for "some
  distinct id" switched to `for_ticker` with placeholder or real-looking
  ticker/exchange literals.
- **Added `SqliteAssetRepository`, ahead of Phase 7.** `Asset` was never
  persisted — only `Transaction` was — so a CLI had no way to show "AAPL"
  instead of a raw `AssetId` after a fresh process start. Deterministic ids
  solve *recovering the id*, but not storing the asset's other fields
  (name, currency, sector, ...) across runs. Mirrors
  `SqliteTransactionRepository` exactly (same upsert idiom, same
  `TEXT`-columns approach); no `from_stored`-style bypass needed since
  `Asset::new` already re-derives the same id from `(ticker, exchange)` on
  every reconstruction, so a stored row's `id` column is only ever used for
  the `WHERE` lookup, never parsed back into a value.

---

## Later (per your own roadmap — not yet)

- [ ] Concurrency
- [ ] Async programming (revisit storage/market data with `sqlx`/async `reqwest` once comfortable)
- [ ] Web frontend
- [ ] Mobile-friendly interface
- [ ] Ticker search/autocomplete for a future UI — type "AAPL", get suggestions
      across exchanges ("AAPL.US", "AAPL.DE", ...). `yahoo_finance_api` already
      has `search_ticker`/`search_ticker_opt` for this; add it as its own port
      (e.g. a `TickerSearchProvider` trait in `market_data.rs`, separate from
      `MarketDataProvider` — searching for a ticker and pricing an asset you
      already have an id for are different concerns). Needs a decision on
      exchange representation: this project stores full names ("NASDAQ",
      "XETRA") in `Asset.exchange`, while Yahoo's own symbols use suffixes
      (bare `AAPL`, `SAP.DE`) — normalize one way or adopt Yahoo's codes as
      canonical, rather than maintaining both conventions. Note: the CLI's
      `resolve_or_register_asset` ambiguity handling (`AmbiguousTicker`,
      `--exchange` disambiguation — see `PHASE_7_CLI_PLAN.md`) exists to
      handle blind typing from a terminal; a UI's autocomplete resolves
      `(ticker, exchange)` before ever calling into that logic, so it
      wouldn't hit that fallback path at all.
- [ ] Research platform aggregation (filings, biotech pipelines, catalysts)
- [ ] Charting