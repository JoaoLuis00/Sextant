use core::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// Fixed, arbitrary — only needs to stay constant so `for_ticker` derives the
/// same id on every run. Not a secret; any fixed UUID would do.
const ASSET_ID_NAMESPACE: Uuid = uuid::uuid!("73d9f46c-e2d4-459f-b1f1-2642f68be42b");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AssetId(Uuid);

impl AssetId {
    pub fn new() -> Self {
        AssetId(Uuid::now_v7())
    }

    /// Deterministic: the same `(ticker, exchange)` always derives the same
    /// id, so there's nothing to persist or look up to recover it later.
    pub fn for_ticker(ticker: &str, exchange: &str) -> Self {
        let name = format!("{ticker}:{exchange}");
        AssetId(Uuid::new_v5(&ASSET_ID_NAMESPACE, name.as_bytes()))
    }
}

impl Default for AssetId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for AssetId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(AssetId(Uuid::from_str(value)?))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PortfolioId(Uuid);

impl PortfolioId {
    pub fn new() -> Self {
        PortfolioId(Uuid::now_v7())
    }
}

impl Default for PortfolioId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for PortfolioId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// UUIDv7 rather than v4: time-ordered by construction, so sorting
/// transactions by `(date, id)` puts same-day entries in creation order with
/// no extra `sequence` field to invent or keep in sync.
///
/// Must be `Uuid::now_v7()`, **not** `Uuid::new_v7(Timestamp::now(NoContext))`.
/// `NoContext` fills the sub-millisecond bits randomly, so two ids minted in
/// the same millisecond sort in random order — which silently breaks the very
/// same-day ordering guarantee this newtype exists to provide. `now_v7()` uses
/// a shared monotonic counter, so ids from one process are always ordered by
/// creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TransactionId(Uuid);

impl TransactionId {
    pub fn new() -> Self {
        TransactionId(Uuid::now_v7())
    }
}

impl Default for TransactionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TransactionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for TransactionId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(TransactionId(Uuid::from_str(value)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transaction_ids_are_time_ordered() {
        // Minting in a tight loop puts many of these in the same millisecond,
        // which is exactly the case a non-monotonic v7 context gets wrong.
        // Strict `<`: ids must be distinct as well as ordered.
        let ids: Vec<TransactionId> = (0..1_000).map(|_| TransactionId::new()).collect();

        for pair in ids.windows(2) {
            assert!(
                pair[0] < pair[1],
                "ids must be strictly increasing in creation order, got {} then {}",
                pair[0],
                pair[1]
            );
        }
    }

    #[test]
    fn asset_id_round_trips_through_its_string_form() {
        let id = AssetId::new();
        assert_eq!(id.to_string().parse::<AssetId>().unwrap(), id);
    }

    #[test]
    fn for_ticker_is_deterministic() {
        assert_eq!(
            AssetId::for_ticker("AAPL", "NASDAQ"),
            AssetId::for_ticker("AAPL", "NASDAQ")
        );
    }

    #[test]
    fn for_ticker_distinguishes_the_same_ticker_across_exchanges() {
        assert_ne!(
            AssetId::for_ticker("BP", "LSE"),
            AssetId::for_ticker("BP", "NYSE")
        );
    }

    #[test]
    fn ids_of_different_kinds_are_distinct_types() {
        // Compile-time guarantee: an AssetId can never be passed where a
        // TransactionId is expected. Nothing to assert at runtime — the fact
        // that this file compiles with both newtypes is the test.
        let asset = AssetId::new();
        let transaction = TransactionId::new();
        assert_ne!(asset.to_string(), transaction.to_string());
    }
}
