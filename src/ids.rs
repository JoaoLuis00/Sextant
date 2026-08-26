use core::fmt;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AssetId(Uuid);

impl AssetId {
    pub fn new() -> Self {
        AssetId(Uuid::now_v7())
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
    fn ids_of_different_kinds_are_distinct_types() {
        // Compile-time guarantee: an AssetId can never be passed where a
        // TransactionId is expected. Nothing to assert at runtime — the fact
        // that this file compiles with both newtypes is the test.
        let asset = AssetId::new();
        let transaction = TransactionId::new();
        assert_ne!(asset.to_string(), transaction.to_string());
    }
}
