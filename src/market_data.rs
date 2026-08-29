//! Market data types and the provider port.
//!
//! Deliberately *not* under `domain/`: prices come from outside the system and
//! change on their own, which is exactly the kind of concern the domain module
//! is meant to stay ignorant of. The Engine consumes [`MarketData`] as an
//! input; concrete [`MarketDataProvider`] implementations (Yahoo Finance, a
//! cache) belong further out still, in the app layer.

use crate::ids::AssetId;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MarketDataError {
    #[error("no market data available for asset {0}")]
    NotFound(AssetId),
    #[error("failed to fetch market data for asset {asset_id}: {reason}")]
    FetchFailed { asset_id: AssetId, reason: String },
    #[error("market data provider unavailable: {0}")]
    ProviderUnavailable(String),
}

/// One price point for one asset. The Engine needs prices for *every* held
/// asset in a single call, so what it actually receives is a
/// `HashMap<AssetId, MarketData>` — this struct is the per-asset element, not
/// the whole input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketData {
    pub asset_id: AssetId,
    pub price: Decimal,
    pub as_of: DateTime<Utc>,
}

impl MarketData {
    pub fn new(asset_id: AssetId, price: Decimal, as_of: DateTime<Utc>) -> Self {
        Self {
            asset_id,
            price,
            as_of,
        }
    }
}

/// Kept object-safe on purpose (no generic methods, no `Self` return) so it
/// can be used as `Box<dyn MarketDataProvider>` — see the dispatch trade-off
/// documented in `engine/portfolio_engine.rs`.
pub trait MarketDataProvider {
    fn price(&self, asset_id: &AssetId) -> Result<Decimal, MarketDataError>;
}

/// In-memory provider used by tests and by the demo binary, so the Engine can
/// be exercised end to end before any real API exists.
#[derive(Debug, Clone, Default)]
pub struct MockProvider {
    prices: std::collections::HashMap<AssetId, Decimal>,
}

impl MockProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_price(mut self, asset_id: AssetId, price: Decimal) -> Self {
        self.prices.insert(asset_id, price);
        self
    }
}

impl MarketDataProvider for MockProvider {
    fn price(&self, asset_id: &AssetId) -> Result<Decimal, MarketDataError> {
        self.prices
            .get(asset_id)
            .copied()
            .ok_or(MarketDataError::NotFound(*asset_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    #[test]
    fn returns_configured_price_for_known_asset() {
        let asset = AssetId::new();
        let provider = MockProvider::new().with_price(asset, dec!(20.0));

        assert_eq!(provider.price(&asset), Ok(dec!(20)));
    }

    #[test]
    fn returns_not_found_for_unknown_asset() {
        let missing_asset = AssetId::new();
        let provider = MockProvider::new();

        assert_eq!(
            provider.price(&missing_asset),
            Err(MarketDataError::NotFound(missing_asset))
        );
    }
}
