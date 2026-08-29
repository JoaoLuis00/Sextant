//! Static vs. dynamic dispatch, built both ways on purpose.
//!
//! Neither is "correct" — they trade different things, and having both here
//! keeps the comparison concrete rather than theoretical. The pure
//! [`crate::engine::portfolio_engine::generate_snapshot`] path needs neither:
//! it takes prices as data (`&HashMap<AssetId, MarketData>`) rather than
//! calling a provider at all, which is what keeps it deterministic. These
//! wrappers are for the *impure* edge that fetches those prices first.

use crate::ids::AssetId;
use crate::market_data::{MarketDataError, MarketDataProvider};
use rust_decimal::Decimal;

/// Static dispatch: `P` is a concrete type fixed at compile time, so the
/// compiler monomorphizes a fresh copy of `StaticPortfolioEngine<P>` (and all
/// its methods) per provider type actually used. `provider.price()` can be
/// inlined — no vtable indirection — but that per-type code duplication grows
/// the binary, and a given `StaticPortfolioEngine<P>` is locked to exactly one
/// provider type: it can't hold a different provider at runtime or be stored
/// alongside engines over other provider types without an enum or generics
/// leaking into whatever holds it.
pub struct StaticPortfolioEngine<P: MarketDataProvider> {
    provider: P,
}

impl<P: MarketDataProvider> StaticPortfolioEngine<P> {
    pub fn new(provider: P) -> Self {
        Self { provider }
    }

    pub fn latest_price(&self, asset_id: &AssetId) -> Result<Decimal, MarketDataError> {
        self.provider.price(asset_id)
    }
}

/// Dynamic dispatch: the concrete provider type is erased behind a trait
/// object, so `DynamicPortfolioEngine` is one concrete type no matter which
/// provider backs it. That lets it be built at runtime (e.g. from a CLI flag
/// choosing Mock vs. a real API) and stored in ordinary fields/collections
/// without generic parameters spreading to every caller. The cost: a heap
/// allocation for the `Box`, one vtable-indirected call per `price()`, and
/// `MarketDataProvider` has to stay object-safe (no generic methods, no `Self`
/// return).
pub struct DynamicPortfolioEngine {
    provider: Box<dyn MarketDataProvider>,
}

impl DynamicPortfolioEngine {
    pub fn new(provider: Box<dyn MarketDataProvider>) -> Self {
        Self { provider }
    }

    pub fn latest_price(&self, asset_id: &AssetId) -> Result<Decimal, MarketDataError> {
        self.provider.price(asset_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market_data::MockProvider;
    use rust_decimal::dec;

    #[test]
    fn static_and_dynamic_engines_agree_on_price() {
        let asset = AssetId::for_ticker("AAPL", "NASDAQ");

        let static_engine =
            StaticPortfolioEngine::new(MockProvider::new().with_price(asset, dec!(42)));
        let dynamic_engine =
            DynamicPortfolioEngine::new(Box::new(MockProvider::new().with_price(asset, dec!(42))));

        assert_eq!(static_engine.latest_price(&asset), Ok(dec!(42)));
        assert_eq!(dynamic_engine.latest_price(&asset), Ok(dec!(42)));
    }

    #[test]
    fn only_dynamic_dispatch_lets_one_collection_hold_mixed_provider_types() {
        // With `Box<dyn MarketDataProvider>`, providers backed by different
        // concrete types can live side by side in one `Vec`. The static,
        // generic-parameter version can't express this without an enum
        // wrapping every provider variant — `StaticPortfolioEngine<P>` is a
        // different type for every `P`.
        struct AlwaysZeroProvider;
        impl MarketDataProvider for AlwaysZeroProvider {
            fn price(&self, _asset_id: &AssetId) -> Result<Decimal, MarketDataError> {
                Ok(Decimal::ZERO)
            }
        }

        let asset = AssetId::for_ticker("AAPL", "NASDAQ");
        let providers: Vec<Box<dyn MarketDataProvider>> = vec![
            Box::new(MockProvider::new().with_price(asset, dec!(10))),
            Box::new(AlwaysZeroProvider),
        ];

        let prices: Vec<Decimal> = providers.iter().map(|p| p.price(&asset).unwrap()).collect();

        assert_eq!(prices, vec![dec!(10), Decimal::ZERO]);
    }

    #[test]
    fn a_missing_price_propagates_through_both_dispatch_styles() {
        let missing = AssetId::for_ticker("MISSING", "TEST");

        let static_engine = StaticPortfolioEngine::new(MockProvider::new());
        let dynamic_engine = DynamicPortfolioEngine::new(Box::new(MockProvider::new()));

        assert_eq!(
            static_engine.latest_price(&missing),
            Err(MarketDataError::NotFound(missing))
        );
        assert_eq!(
            dynamic_engine.latest_price(&missing),
            Err(MarketDataError::NotFound(missing))
        );
    }
}
