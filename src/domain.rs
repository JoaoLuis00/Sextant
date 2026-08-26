//! Data only, no business logic.
//!
//! Everything here is either persisted (`Asset`, `Transaction`, `Portfolio`)
//! or produced by the Engine and never stored (`Position`,
//! `PositionValuation`, `PortfolioSnapshot`). Nothing in this module knows
//! about SQL, HTTP, or the CLI.

pub mod asset;
pub mod errors;
pub mod portfolio;
pub mod position;
pub mod snapshot;
pub mod transaction;
