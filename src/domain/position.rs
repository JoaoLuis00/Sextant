use crate::ids::AssetId;
use rust_decimal::Decimal;

/// Current ownership of a single asset.
///
/// Deliberately excludes price, market value and unrealized P&L — those are a
/// *valuation* concern, and valuation needs market data this type never sees.
/// See [`PositionValuation`] for the type that combines the two.
///
/// Fields are private with accessors: every one of them is either derived
/// (`cost_basis`) or maintained as part of an invariant the Engine establishes
/// while replaying history, so nothing outside `engine/` should be able to set
/// them independently and let them drift apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Position {
    asset_id: AssetId,
    quantity: Decimal,
    average_cost: Decimal,
    cost_basis: Decimal,
    realized_pnl: Decimal,
}

impl Position {
    /// Constructs a position from already-replayed totals. `cost_basis` is
    /// computed here rather than accepted, so it can never disagree with
    /// `quantity * average_cost`.
    pub fn new(
        asset_id: AssetId,
        quantity: Decimal,
        average_cost: Decimal,
        realized_pnl: Decimal,
    ) -> Self {
        Self {
            asset_id,
            quantity,
            average_cost,
            cost_basis: quantity * average_cost,
            realized_pnl,
        }
    }

    pub fn asset_id(&self) -> AssetId {
        self.asset_id
    }

    pub fn quantity(&self) -> Decimal {
        self.quantity
    }

    /// Per-share average cost. Resets to zero whenever `quantity` returns to
    /// zero — a later buy starts a fresh average-cost cycle.
    pub fn average_cost(&self) -> Decimal {
        self.average_cost
    }

    /// Derived: `quantity * average_cost`. Resets with `average_cost`.
    pub fn cost_basis(&self) -> Decimal {
        self.cost_basis
    }

    /// **Cumulative** realized P&L for this asset across all history — not
    /// reset when a position is closed and reopened. This is what preserves
    /// P&L history across buy → sell → buy cycles.
    pub fn realized_pnl(&self) -> Decimal {
        self.realized_pnl
    }

    /// Whether this asset is currently held. Only open positions appear in
    /// [`crate::domain::snapshot::PortfolioSnapshot::positions`].
    pub fn is_open(&self) -> bool {
        self.quantity > Decimal::ZERO
    }
}

/// A [`Position`] combined with the market data used to value it.
///
/// This is what a snapshot holds, rather than a bare `Position` — without it
/// nothing in the model carries per-asset market value, and the snapshot
/// totals could not be summed from anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionValuation {
    position: Position,
    current_price: Decimal,
    market_value: Decimal,
    unrealized_pnl: Decimal,
}

impl PositionValuation {
    /// Smart constructor guarding a fixed invariant.
    ///
    /// The line between domain and Engine here is *"could this represent a
    /// policy that might vary, or is it a fixed mathematical fact given the
    /// inputs?"* `Position::cost_basis` is policy — v1 uses average cost, but
    /// FIFO/LIFO are legitimate alternatives producing different answers from
    /// the same inputs, so the Engine owns it. `market_value` and
    /// `unrealized_pnl` have no alternate algorithm; given a position and a
    /// price there is exactly one correct answer. That makes them an
    /// invariant, safe to guarantee on the type itself so the two fields can
    /// never be built out of sync no matter how many places construct one.
    ///
    /// The Engine still decides *which* position and *which* price get
    /// combined, and *when* — this only guards the arithmetic once those
    /// inputs are chosen.
    pub fn new(position: Position, current_price: Decimal) -> Self {
        let market_value = position.quantity() * current_price;
        let unrealized_pnl = market_value - position.cost_basis();
        Self {
            position,
            current_price,
            market_value,
            unrealized_pnl,
        }
    }

    pub fn position(&self) -> &Position {
        &self.position
    }

    pub fn current_price(&self) -> Decimal {
        self.current_price
    }

    pub fn market_value(&self) -> Decimal {
        self.market_value
    }

    pub fn unrealized_pnl(&self) -> Decimal {
        self.unrealized_pnl
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    #[test]
    fn cost_basis_is_derived_from_quantity_and_average_cost() {
        let position = Position::new(
            AssetId::for_ticker("AAPL", "NASDAQ"),
            dec!(10),
            dec!(150),
            Decimal::ZERO,
        );
        assert_eq!(position.cost_basis(), dec!(1500));
    }

    #[test]
    fn valuation_derives_market_value_and_unrealized_pnl() {
        let position = Position::new(
            AssetId::for_ticker("AAPL", "NASDAQ"),
            dec!(10),
            dec!(150),
            Decimal::ZERO,
        );
        let valuation = PositionValuation::new(position, dec!(210));

        assert_eq!(valuation.market_value(), dec!(2100));
        assert_eq!(valuation.unrealized_pnl(), dec!(600)); // 2100 - 1500
    }

    #[test]
    fn unrealized_pnl_is_negative_when_price_falls_below_cost() {
        let position = Position::new(
            AssetId::for_ticker("AAPL", "NASDAQ"),
            dec!(10),
            dec!(150),
            Decimal::ZERO,
        );
        let valuation = PositionValuation::new(position, dec!(120));

        assert_eq!(valuation.unrealized_pnl(), dec!(-300));
    }

    #[test]
    fn a_zero_quantity_position_is_not_open() {
        let closed = Position::new(
            AssetId::for_ticker("AAPL", "NASDAQ"),
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(500),
        );
        assert!(!closed.is_open());
        // ...but its realized P&L history survives the exit.
        assert_eq!(closed.realized_pnl(), dec!(500));
    }
}
