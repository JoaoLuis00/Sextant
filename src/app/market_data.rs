//! `YahooFinanceProvider`: a `MarketDataProvider` backed by the
//! `yahoo_finance_api` crate.
//!
//! Uses its `blocking` feature (sync, matches `MarketDataProvider::price`,
//! no runtime needed) and `decimal` feature (returns `rust_decimal::Decimal`
//! directly, no `f64` conversion of our own).

use std::collections::HashMap;

use yahoo_finance_api::{YahooConnector, YahooError};

use crate::domain::asset::Ticker;
use crate::ids::AssetId;
use crate::market_data::{MarketDataError, MarketDataProvider};
use rust_decimal::Decimal;

/// Maps our `AssetId`s to the ticker symbols Yahoo expects.
pub struct YahooFinanceProvider {
    connector: YahooConnector,
    tickers: HashMap<AssetId, Ticker>,
}

impl YahooFinanceProvider {
    /// Fallible, unlike `MockProvider::new()` — building the HTTP client can fail.
    pub fn new() -> Result<Self, MarketDataError> {
        let connector = YahooConnector::new()
            .map_err(|e| MarketDataError::ProviderUnavailable(e.to_string()))?;
        Ok(Self {
            connector,
            tickers: HashMap::new(),
        })
    }

    pub fn with_ticker(mut self, asset_id: AssetId, ticker: Ticker) -> Self {
        self.tickers.insert(asset_id, ticker);
        self
    }
}

impl MarketDataProvider for YahooFinanceProvider {
    fn price(&self, asset_id: &AssetId) -> Result<Decimal, MarketDataError> {
        let ticker = self
            .tickers
            .get(asset_id)
            .ok_or(MarketDataError::NotFound(*asset_id))?;

        let fetch_failed = |e: YahooError| MarketDataError::FetchFailed {
            asset_id: *asset_id,
            reason: e.to_string(),
        };

        let response = self
            .connector
            .get_latest_quotes(ticker.as_str(), "1d")
            .map_err(fetch_failed)?;

        response
            .last_quote()
            .map(|quote| quote.close)
            .map_err(fetch_failed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn price_fails_loud_for_an_asset_with_no_ticker_mapping() {
        let provider = YahooFinanceProvider::new().unwrap();
        let asset = AssetId::new();

        assert_eq!(
            provider.price(&asset),
            Err(MarketDataError::NotFound(asset))
        );
    }

    // Hits the real API. Run with `cargo test --features market_data -- --ignored`.
    #[test]
    #[ignore]
    fn price_fetches_a_real_quote_for_a_known_ticker() {
        let asset = AssetId::new();
        let provider = YahooFinanceProvider::new()
            .unwrap()
            .with_ticker(asset, Ticker::new("AAPL"));

        assert!(provider.price(&asset).unwrap() > Decimal::ZERO);
    }
}
