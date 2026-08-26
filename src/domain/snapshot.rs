use crate::domain::position::PositionValuation;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

/// A point-in-time valuation of the whole portfolio.
///
/// Pure Engine output — never persisted, never a source of truth, always
/// regenerable by replaying transaction history against current prices.
///
/// **Currency:** v1 assumes a single currency across the portfolio. The
/// totals below carry no currency tag, so mixing assets denominated in
/// different currencies would silently sum incomparable numbers. Multi-currency
/// aggregation needs an FX layer before that assumption can be relaxed.
#[derive(Debug, Clone, PartialEq)]
pub struct PortfolioSnapshot {
    generated_at: DateTime<Utc>,
    positions: Vec<PositionValuation>,
    total_cost_basis: Decimal,
    total_market_value: Decimal,
    total_realized_pnl: Decimal,
    total_unrealized_pnl: Decimal,
}

impl PortfolioSnapshot {
    /// Only `engine/` should be building these — the totals must agree with
    /// `positions`, and the Engine is what establishes that.
    pub fn new(
        generated_at: DateTime<Utc>,
        positions: Vec<PositionValuation>,
        total_cost_basis: Decimal,
        total_market_value: Decimal,
        total_realized_pnl: Decimal,
        total_unrealized_pnl: Decimal,
    ) -> Self {
        Self {
            generated_at,
            positions,
            total_cost_basis,
            total_market_value,
            total_realized_pnl,
            total_unrealized_pnl,
        }
    }

    pub fn generated_at(&self) -> DateTime<Utc> {
        self.generated_at
    }

    /// Currently-held assets only (`quantity > 0`). An asset that was fully
    /// exited has no line here — but its realized P&L still shows up in
    /// [`Self::total_realized_pnl`].
    pub fn positions(&self) -> &[PositionValuation] {
        &self.positions
    }

    /// Sum over currently-open positions only.
    pub fn total_cost_basis(&self) -> Decimal {
        self.total_cost_basis
    }

    /// Sum over currently-open positions only.
    pub fn total_market_value(&self) -> Decimal {
        self.total_market_value
    }

    /// Derived from the **full transaction history**, including assets that
    /// have been fully sold and no longer appear in [`Self::positions`]. This
    /// is where closed-position P&L history lives.
    pub fn total_realized_pnl(&self) -> Decimal {
        self.total_realized_pnl
    }

    /// Sum over currently-open positions only.
    pub fn total_unrealized_pnl(&self) -> Decimal {
        self.total_unrealized_pnl
    }

    /// Realized plus unrealized — what the portfolio has made overall.
    pub fn total_pnl(&self) -> Decimal {
        self.total_realized_pnl + self.total_unrealized_pnl
    }

    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }
}
