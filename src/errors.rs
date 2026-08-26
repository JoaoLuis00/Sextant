//! Top-level error composition.
//!
//! Each layer owns its own error enum next to the code that raises it
//! (`domain/errors.rs`, `engine/errors.rs`, `market_data.rs`); this module
//! re-exports them so callers have one import site, and defines the crate-wide
//! [`Error`] that composes them via `#[from]`.

use thiserror::Error;

pub use crate::domain::errors::{AssetError, DomainError, TransactionError};
pub use crate::engine::errors::EngineError;
pub use crate::market_data::MarketDataError;

/// The error type a top-level caller (the CLI, an integration test) sees.
///
/// Layers below never construct this — they return their own layer's error,
/// and `?` widens it here at the boundary.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Error {
    #[error(transparent)]
    Domain(#[from] DomainError),

    #[error(transparent)]
    Engine(#[from] EngineError),

    #[error(transparent)]
    MarketData(#[from] MarketDataError),
}

impl From<TransactionError> for Error {
    fn from(err: TransactionError) -> Self {
        Error::Domain(DomainError::Transaction(err))
    }
}

impl From<AssetError> for Error {
    fn from(err: AssetError) -> Self {
        Error::Domain(DomainError::Asset(err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transaction_errors_widen_all_the_way_to_the_top_level_error() {
        let err: Error = TransactionError::NonPositiveQuantity.into();
        assert_eq!(
            err,
            Error::Domain(DomainError::Transaction(
                TransactionError::NonPositiveQuantity
            ))
        );
        // `#[error(transparent)]` all the way down means the top-level message
        // is the original one, with no "domain error: " noise stacked on it.
        assert_eq!(err.to_string(), "quantity must be positive");
    }

    #[test]
    fn engine_errors_widen_to_the_top_level_error() {
        let engine_err = EngineError::Domain(DomainError::Asset(AssetError::UnknownCurrency(
            "XYZ".to_string(),
        )));
        let err: Error = engine_err.into();
        assert_eq!(err.to_string(), "unknown currency code: XYZ");
    }
}
