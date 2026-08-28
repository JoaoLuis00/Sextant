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

- [x] Implement `YahooFinanceProvider: MarketDataProvider` using `reqwest` + `serde`
- [x] Deserialize API responses into domain types (or a DTO layer that converts into domain types)
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
- **`Transaction::from_stored` bypasses id minting.** `Transaction::new` always
  mints a fresh `TransactionId`, but a repository loading a row needs to
  restore the id that's already on disk. Added a `pub(crate)`-only
  constructor rather than a public one, so only code inside the crate
  (storage) can set an arbitrary id — external callers still go through
  `new()`. `TransactionType` doesn't need the same treatment since `buy()`/
  `sell()` re-validate on load anyway.
- **`StorageError` doesn't widen into the top-level `Error`.** Every other
  layer's error composes into `Error` via `#[from]`; `StorageError` wraps
  `rusqlite::Error`, which isn't `Clone`/`PartialEq`, so it can't be added to
  an enum that derives those. Left standalone for now — revisit when Phase 7
  wires storage into the CLI and needs one error type at the boundary.
- **`YahooFinanceProvider` needs a ticker map, not just `AssetId`s.**
  `MarketDataProvider::price` takes only an `AssetId`, but Yahoo's API only
  knows ticker symbols. Rather than changing the trait, the provider carries
  its own `HashMap<AssetId, Ticker>`, set via `.with_ticker(...)` (mirrors
  `MockProvider::with_price`). An asset with no entry fails with the same
  `MarketDataError::NotFound` a missing quote would produce — accurate either
  way, and one fewer error variant to reason about.
- **Yahoo errors stringify into `MarketDataError::FetchFailed`.** Unlike
  `StorageError`, `MarketDataError` is a fixed associated type on the trait
  (not per-implementor), and it already derives `Clone`/`PartialEq`/`Eq` for
  the rest of the crate — so `reqwest::Error`/`serde_json::Error` get
  `.to_string()`'d into a `reason: String` field immediately rather than
  wrapped with `#[from]`.
- **Yahoo's JSON carries price as `f64`.** Converted to `Decimal` once, right
  at deserialization, and never handled as a float again — the one place
  "never `f64` for money" bends, because the wire format isn't ours to
  choose. `Decimal::try_from(f64)` was checked against `rust_decimal`'s own
  doctest (`0.1_f64` → `"0.1"`) before relying on it for real prices.

---

## Later (per your own roadmap — not yet)

- [ ] Concurrency
- [ ] Async programming (revisit storage/market data with `sqlx`/async `reqwest` once comfortable)
- [ ] Web frontend
- [ ] Mobile-friendly interface
- [ ] Research platform aggregation (filings, biotech pipelines, catalysts)
- [ ] Charting