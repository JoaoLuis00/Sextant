use rust_decimal::Decimal;
use thiserror::Error;

use crate::domain::errors::DomainError;
use crate::ids::AssetId;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EngineError {
    /// A currently-held asset had no price in the market data map.
    ///
    /// v1 fails loud rather than zeroing or falling back to a stale cached
    /// price: the Engine is meant to be deterministic, and a missing price is
    /// a real problem worth surfacing immediately instead of silently
    /// approximating a portfolio total.
    #[error("no market data available for asset {0}")]
    MissingMarketData(AssetId),

    /// The transaction history sells more of an asset than it ever bought.
    ///
    /// v1 does not model short positions, so this is always a data-entry
    /// error (a missing buy, or a duplicated sell) rather than a legitimate
    /// negative holding — same "fail loud" reasoning as above.
    #[error("asset {asset_id} oversold: tried to sell {attempted} with only {held} held")]
    OversoldAsset {
        asset_id: AssetId,
        held: Decimal,
        attempted: Decimal,
    },

    #[error(transparent)]
    Domain(#[from] DomainError),
}
