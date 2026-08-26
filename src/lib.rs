//! # Sextant
//!
//! A personal investment platform. A sextant fixes your position by taking a
//! reading at a moment in time — which is exactly what this does: [`Position`]
//! and [`PortfolioSnapshot`] are the two types everything else serves.
//!
//! The architecture is a one-way flow:
//!
//! ```text
//! Transaction history ──┐
//!                       ├──> engine::generate_snapshot ──> PortfolioSnapshot
//! Market data ──────────┘
//! ```
//!
//! Only `Asset` and `Transaction` are ever persisted. `Position`,
//! `PositionValuation` and `PortfolioSnapshot` are pure Engine output,
//! regenerated on demand and never a source of truth — which is what makes
//! "always reproducible from history" true rather than aspirational.
//!
//! Module boundaries:
//! - [`domain`] — data only, no logic, no I/O.
//! - [`engine`] — stateless calculations over domain data.
//! - [`market_data`] — prices from outside the system, plus the provider port.
//! - [`ids`] — crate-global newtypes, used by every layer.
//! - [`errors`] — per-layer errors, composed into one [`Error`].

pub mod domain;
pub mod engine;
pub mod errors;
pub mod ids;
pub mod market_data;

// Flat re-exports: the public API callers should reach for. Deep paths like
// `sextant::domain::position::PositionValuation` stay available, but nothing
// outside the crate should need them.
pub use domain::asset::{Asset, AssetType, Currency, Ticker};
pub use domain::portfolio::Portfolio;
pub use domain::position::{Position, PositionValuation};
pub use domain::snapshot::PortfolioSnapshot;
pub use domain::transaction::{Transaction, TransactionType};
pub use engine::portfolio_engine::{build_holdings, generate_snapshot};
pub use engine::repository::{InMemoryTransactionRepository, Repository};
pub use errors::{AssetError, DomainError, EngineError, Error, MarketDataError, TransactionError};
pub use ids::{AssetId, PortfolioId, TransactionId};
pub use market_data::{MarketData, MarketDataProvider, MockProvider};

/// Crate-wide result alias, so signatures at the app boundary read as
/// `fn run() -> sextant::Result<()>`.
pub type Result<T> = std::result::Result<T, Error>;
