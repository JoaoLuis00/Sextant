//! `YahooFinanceProvider`: a `MarketDataProvider` backed by Yahoo Finance's
//! chart endpoint.
//!
//! Blocking, not async — same reasoning as `storage.rs` using sync
//! `rusqlite`: `MarketDataProvider::price` is a sync trait method, and async
//! is a deliberately later upgrade (see the roadmap's "Later" section).

use std::collections::HashMap;

use reqwest::blocking::Client;
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::domain::asset::Ticker;
use crate::ids::AssetId;
use crate::market_data::{MarketDataError, MarketDataProvider};

const BASE_URL: &str = "https://query1.finance.yahoo.com/v8/finance/chart";
const USER_AGENT: &str = "Mozilla/5.0";

#[derive(Debug, Deserialize)]
struct ChartResponse {
    chart: Chart,
}

#[derive(Debug, Deserialize)]
struct Chart {
    result: Option<Vec<ChartResult>>,
    error: Option<ChartError>,
}

#[derive(Debug, Deserialize)]
struct ChartResult {
    meta: ChartMeta,
}

#[derive(Debug, Deserialize)]
struct ChartMeta {
    #[serde(rename = "regularMarketPrice")]
    regular_market_price: f64,
}

#[derive(Debug, Deserialize)]
struct ChartError {
    description: String,
}

/// Split out from `price()` so the JSON-handling logic — the one place a
/// real bug can hide — is testable against fixture strings, with no network
/// call involved.
fn extract_price(body: &str, asset_id: AssetId) -> Result<Decimal, MarketDataError> {
    let fetch_failed = |reason: String| MarketDataError::FetchFailed { asset_id, reason };

    let response: ChartResponse =
        serde_json::from_str(body).map_err(|e| fetch_failed(e.to_string()))?;

    if let Some(error) = response.chart.error {
        return Err(fetch_failed(error.description));
    }

    let result = response
        .chart
        .result
        .and_then(|results| results.into_iter().next())
        .ok_or(MarketDataError::NotFound(asset_id))?;

    // Yahoo's wire format is a float. Converted to `Decimal` right here, at
    // the boundary, and never touched as a float again — the one exception
    // to "never `f64` for money" that a third-party API forces on us.
    Decimal::try_from(result.meta.regular_market_price).map_err(|e| fetch_failed(e.to_string()))
}

/// Maps our internal `AssetId`s to the ticker symbols Yahoo expects — the
/// wire API has no notion of our ids, so the provider has to carry that
/// mapping itself.
#[derive(Debug, Clone, Default)]
pub struct YahooFinanceProvider {
    client: Client,
    tickers: HashMap<AssetId, Ticker>,
}

impl YahooFinanceProvider {
    pub fn new() -> Self {
        Self::default()
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

        let body = self
            .client
            .get(format!("{BASE_URL}/{}", ticker.as_str()))
            .query(&[("interval", "1d"), ("range", "1d")])
            .header("User-Agent", USER_AGENT)
            .send()
            .and_then(|response| response.error_for_status())
            .and_then(|response| response.text())
            .map_err(|e| MarketDataError::FetchFailed {
                asset_id: *asset_id,
                reason: e.to_string(),
            })?;

        extract_price(&body, *asset_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AAPL_RESPONSE: &str = r#"{
        "chart": {
            "result": [{ "meta": { "regularMarketPrice": 319.7 } }],
            "error": null
        }
    }"#;

    const UNKNOWN_TICKER_RESPONSE: &str = r#"{
        "chart": {
            "result": null,
            "error": { "code": "Not Found", "description": "No data found, symbol may be delisted" }
        }
    }"#;

    #[test]
    fn extract_price_reads_the_regular_market_price() {
        let asset = AssetId::new();
        assert_eq!(
            extract_price(AAPL_RESPONSE, asset),
            Ok(rust_decimal::dec!(319.7))
        );
    }

    #[test]
    fn extract_price_surfaces_yahoos_error_description() {
        let asset = AssetId::new();
        assert_eq!(
            extract_price(UNKNOWN_TICKER_RESPONSE, asset),
            Err(MarketDataError::FetchFailed {
                asset_id: asset,
                reason: "No data found, symbol may be delisted".to_string(),
            })
        );
    }

    #[test]
    fn extract_price_rejects_malformed_json() {
        let asset = AssetId::new();
        assert!(matches!(
            extract_price("not json", asset),
            Err(MarketDataError::FetchFailed { .. })
        ));
    }

    #[test]
    fn price_fails_loud_for_an_asset_with_no_ticker_mapping() {
        let provider = YahooFinanceProvider::new();
        let asset = AssetId::new();

        assert_eq!(
            provider.price(&asset),
            Err(MarketDataError::NotFound(asset))
        );
    }
}
