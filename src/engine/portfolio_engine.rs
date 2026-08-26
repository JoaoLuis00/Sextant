//! The portfolio Engine: stateless, deterministic, pure.
//!
//! Only two things flow in — transactions and market data. Nothing else: not
//! `Portfolio`, not a previous snapshot. The Engine has no memory between
//! calls, so the same two inputs always produce the same output.

use crate::domain::position::{Position, PositionValuation};
use crate::domain::snapshot::PortfolioSnapshot;
use crate::domain::transaction::{Transaction, TransactionType};
use crate::engine::errors::EngineError;
use crate::ids::AssetId;
use crate::market_data::MarketData;
use chrono::Utc;
use rust_decimal::Decimal;
use std::collections::HashMap;

/// Replays a transaction log into per-asset positions.
///
/// Returns **every** asset that appears in the history, including ones fully
/// sold off (`quantity == 0`). Callers that only want current holdings filter
/// with [`Position::is_open`] — but the closed ones must stay reachable here,
/// because their `realized_pnl` is part of the portfolio's history and would
/// otherwise be silently dropped from the totals.
///
/// Takes `&[Transaction]` (borrow) rather than `Vec<Transaction>` (own):
/// callers only lend their history for the duration of the call, and the
/// Engine has no reason to take it away from them.
pub fn build_holdings(
    transactions: &[Transaction],
) -> Result<HashMap<AssetId, Position>, EngineError> {
    // Group first so each asset's history can be replayed independently.
    // `&Transaction` in the buckets — grouping shouldn't require cloning.
    let mut by_asset: HashMap<AssetId, Vec<&Transaction>> = HashMap::new();
    for tx in transactions {
        by_asset.entry(tx.asset_id()).or_default().push(tx);
    }

    by_asset
        .into_iter()
        .map(|(asset_id, txs)| Ok((asset_id, build_position(asset_id, &txs)?)))
        .collect()
}

/// Folds one asset's transactions into a `Position` using **average cost
/// basis**.
///
/// This is the Engine's job precisely because it's a *policy*: FIFO and LIFO
/// are legitimate alternatives that produce different (equally correct)
/// answers from the same inputs. v1 picks average cost; swapping in FIFO later
/// means changing this function and nothing else.
///
/// Transactions are replayed in the order given — [`crate::domain::portfolio::Portfolio::sort_transactions`]
/// is what normalizes that order, since average-cost replay is order-sensitive.
fn build_position(
    asset_id: AssetId,
    transactions: &[&Transaction],
) -> Result<Position, EngineError> {
    let mut quantity = Decimal::ZERO;
    let mut average_cost = Decimal::ZERO;
    let mut realized_pnl = Decimal::ZERO;

    for tx in transactions {
        match *tx.transaction_type() {
            TransactionType::Buy {
                quantity: buy_qty,
                price,
            } => {
                let new_quantity = quantity + buy_qty;
                // weighted average of old holdings + new purchase
                average_cost = (average_cost * quantity + price * buy_qty) / new_quantity;
                quantity = new_quantity;
            }
            TransactionType::Sell {
                quantity: sell_qty,
                price,
            } => {
                // v1 doesn't model shorts, so selling more than is held is
                // always a data-entry error. Fail loud rather than carrying a
                // negative quantity and a nonsense average cost forward.
                if sell_qty > quantity {
                    return Err(EngineError::OversoldAsset {
                        asset_id,
                        held: quantity,
                        attempted: sell_qty,
                    });
                }
                realized_pnl += (price - average_cost) * sell_qty;
                quantity -= sell_qty;
                if quantity == Decimal::ZERO {
                    // Fresh average-cost cycle on full exit. Only
                    // `realized_pnl` carries forward across the boundary.
                    average_cost = Decimal::ZERO;
                }
            }
        }
    }

    Ok(Position::new(asset_id, quantity, average_cost, realized_pnl))
}

/// Values a transaction history against current prices.
///
/// Takes `&[Transaction]` rather than `&Portfolio` deliberately: every
/// calculation derives from history alone, so coupling the Engine to a domain
/// type it doesn't need would buy nothing.
///
/// `PortfolioId` appears nowhere in the output types — v1 has one portfolio,
/// so "which portfolio" is just "whichever transactions were passed in". If
/// multi-portfolio support arrives, the caller filters by portfolio *before*
/// calling; the Engine still wouldn't need to know.
pub fn generate_snapshot(
    transactions: &[Transaction],
    market_data: &HashMap<AssetId, MarketData>,
) -> Result<PortfolioSnapshot, EngineError> {
    let holdings = build_holdings(transactions)?;

    // Realized P&L spans the *entire* history — including assets fully sold
    // off, which have no line in `positions` below. Summing this over the open
    // positions instead would silently lose the P&L of every closed position.
    let total_realized_pnl = holdings.values().map(|p| p.realized_pnl()).sum();

    // Only currently-held assets get valued. A closed position needs no price,
    // so filtering *before* the market-data lookup also avoids demanding a
    // quote for something the portfolio no longer owns.
    let mut positions = holdings
        .into_values()
        .filter(Position::is_open)
        .map(|position| {
            let price = market_data
                .get(&position.asset_id())
                .map(|md| md.price)
                .ok_or(EngineError::MissingMarketData(position.asset_id()))?;
            Ok(PositionValuation::new(position, price))
        })
        .collect::<Result<Vec<PositionValuation>, EngineError>>()?;

    // `HashMap` iteration order is deliberately unspecified; sort so a
    // snapshot is reproducible and tests/CLI output don't shuffle between runs.
    positions.sort_by_key(|valuation| valuation.position().asset_id());

    let total_cost_basis = positions.iter().map(|p| p.position().cost_basis()).sum();
    let total_market_value = positions.iter().map(|p| p.market_value()).sum();
    let total_unrealized_pnl = positions.iter().map(|p| p.unrealized_pnl()).sum();

    Ok(PortfolioSnapshot::new(
        Utc::now(),
        positions,
        total_cost_basis,
        total_market_value,
        total_realized_pnl,
        total_unrealized_pnl,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::AssetId;
    use chrono::NaiveDate;
    use rust_decimal::dec;

    fn day(d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2024, 1, d).unwrap()
    }

    fn buy(asset: AssetId, d: u32, quantity: Decimal, price: Decimal) -> Transaction {
        Transaction::new(
            asset,
            day(d),
            TransactionType::buy(quantity, price).unwrap(),
            None,
        )
        .unwrap()
    }

    fn sell(asset: AssetId, d: u32, quantity: Decimal, price: Decimal) -> Transaction {
        Transaction::new(
            asset,
            day(d),
            TransactionType::sell(quantity, price).unwrap(),
            None,
        )
        .unwrap()
    }

    fn prices(entries: &[(AssetId, Decimal)]) -> HashMap<AssetId, MarketData> {
        entries
            .iter()
            .map(|(id, price)| (*id, MarketData::new(*id, *price, Utc::now())))
            .collect()
    }

    // ---- cost basis ----

    #[test]
    fn single_buy_sets_average_cost_to_the_purchase_price() {
        let asset = AssetId::new();
        let holdings = build_holdings(&[buy(asset, 1, dec!(10), dec!(150))]).unwrap();
        let position = &holdings[&asset];

        assert_eq!(position.quantity(), dec!(10));
        assert_eq!(position.average_cost(), dec!(150));
        assert_eq!(position.cost_basis(), dec!(1500));
    }

    #[test]
    fn repeated_buys_produce_a_weighted_average_cost() {
        let asset = AssetId::new();
        let holdings = build_holdings(&[
            buy(asset, 1, dec!(10), dec!(100)),
            buy(asset, 2, dec!(30), dec!(200)),
        ])
        .unwrap();
        let position = &holdings[&asset];

        // (10*100 + 30*200) / 40 = 7000 / 40 = 175
        assert_eq!(position.quantity(), dec!(40));
        assert_eq!(position.average_cost(), dec!(175));
        assert_eq!(position.cost_basis(), dec!(7000));
    }

    #[test]
    fn a_sell_does_not_change_average_cost_of_the_remainder() {
        let asset = AssetId::new();
        let holdings = build_holdings(&[
            buy(asset, 1, dec!(10), dec!(100)),
            sell(asset, 2, dec!(4), dec!(180)),
        ])
        .unwrap();
        let position = &holdings[&asset];

        assert_eq!(position.quantity(), dec!(6));
        assert_eq!(position.average_cost(), dec!(100));
        assert_eq!(position.cost_basis(), dec!(600));
    }

    #[test]
    fn a_full_exit_resets_average_cost_and_starts_a_fresh_cycle() {
        let asset = AssetId::new();
        let holdings = build_holdings(&[
            buy(asset, 1, dec!(10), dec!(100)),
            sell(asset, 2, dec!(10), dec!(180)),
            buy(asset, 3, dec!(5), dec!(50)),
        ])
        .unwrap();
        let position = &holdings[&asset];

        // Without the reset, the re-entry would average against the old 100.
        assert_eq!(position.average_cost(), dec!(50));
        assert_eq!(position.quantity(), dec!(5));
        assert_eq!(position.cost_basis(), dec!(250));
    }

    // ---- realized P&L ----

    #[test]
    fn realized_pnl_is_gain_over_average_cost_times_quantity_sold() {
        let asset = AssetId::new();
        let holdings = build_holdings(&[
            buy(asset, 1, dec!(10), dec!(100)),
            sell(asset, 2, dec!(4), dec!(180)),
        ])
        .unwrap();

        // (180 - 100) * 4 = 320
        assert_eq!(holdings[&asset].realized_pnl(), dec!(320));
    }

    #[test]
    fn realized_pnl_can_be_negative() {
        let asset = AssetId::new();
        let holdings = build_holdings(&[
            buy(asset, 1, dec!(10), dec!(100)),
            sell(asset, 2, dec!(10), dec!(60)),
        ])
        .unwrap();

        assert_eq!(holdings[&asset].realized_pnl(), dec!(-400));
    }

    #[test]
    fn realized_pnl_accumulates_across_close_and_reopen_cycles() {
        let asset = AssetId::new();
        let holdings = build_holdings(&[
            buy(asset, 1, dec!(10), dec!(100)),
            sell(asset, 2, dec!(10), dec!(150)), // +500
            buy(asset, 3, dec!(10), dec!(200)),
            sell(asset, 4, dec!(10), dec!(250)), // +500
        ])
        .unwrap();

        // Cumulative across both cycles, not reset on reopen.
        assert_eq!(holdings[&asset].realized_pnl(), dec!(1000));
        assert!(!holdings[&asset].is_open());
    }

    // ---- oversell guard ----

    #[test]
    fn selling_more_than_held_is_rejected() {
        let asset = AssetId::new();
        let result = build_holdings(&[
            buy(asset, 1, dec!(5), dec!(100)),
            sell(asset, 2, dec!(10), dec!(150)),
        ]);

        assert_eq!(
            result,
            Err(EngineError::OversoldAsset {
                asset_id: asset,
                held: dec!(5),
                attempted: dec!(10),
            })
        );
    }

    #[test]
    fn selling_with_nothing_held_is_rejected() {
        let asset = AssetId::new();
        let result = build_holdings(&[sell(asset, 1, dec!(1), dec!(100))]);

        assert!(matches!(
            result,
            Err(EngineError::OversoldAsset { .. })
        ));
    }

    // ---- holdings aggregation ----

    #[test]
    fn holdings_are_grouped_per_asset() {
        let apple = AssetId::new();
        let msft = AssetId::new();
        let holdings = build_holdings(&[
            buy(apple, 1, dec!(10), dec!(100)),
            buy(msft, 1, dec!(5), dec!(200)),
            sell(apple, 2, dec!(4), dec!(120)),
        ])
        .unwrap();

        assert_eq!(holdings.len(), 2);
        assert_eq!(holdings[&apple].quantity(), dec!(6));
        assert_eq!(holdings[&msft].quantity(), dec!(5));
    }

    #[test]
    fn build_holdings_retains_fully_closed_assets() {
        let asset = AssetId::new();
        let holdings = build_holdings(&[
            buy(asset, 1, dec!(10), dec!(100)),
            sell(asset, 2, dec!(10), dec!(150)),
        ])
        .unwrap();

        // Still present here (so its realized P&L is reachable) even though it
        // will be filtered out of the snapshot's `positions`.
        assert_eq!(holdings.len(), 1);
        assert!(!holdings[&asset].is_open());
        assert_eq!(holdings[&asset].realized_pnl(), dec!(500));
    }

    #[test]
    fn an_empty_history_produces_no_holdings() {
        assert!(build_holdings(&[]).unwrap().is_empty());
    }

    // ---- snapshot ----

    #[test]
    fn snapshot_sums_totals_across_open_positions() {
        let apple = AssetId::new();
        let msft = AssetId::new();
        let transactions = [buy(apple, 1, dec!(10), dec!(100)), buy(msft, 1, dec!(5), dec!(200))];
        let market = prices(&[(apple, dec!(150)), (msft, dec!(180))]);

        let snapshot = generate_snapshot(&transactions, &market).unwrap();

        assert_eq!(snapshot.positions().len(), 2);
        // 10*150 + 5*180 = 1500 + 900
        assert_eq!(snapshot.total_market_value(), dec!(2400));
        // 10*100 + 5*200 = 1000 + 1000
        assert_eq!(snapshot.total_cost_basis(), dec!(2000));
        // 2400 - 2000
        assert_eq!(snapshot.total_unrealized_pnl(), dec!(400));
        assert_eq!(snapshot.total_realized_pnl(), Decimal::ZERO);
    }

    #[test]
    fn a_fully_exited_asset_is_dropped_from_positions() {
        let apple = AssetId::new();
        let msft = AssetId::new();
        let transactions = [
            buy(apple, 1, dec!(10), dec!(100)),
            sell(apple, 2, dec!(10), dec!(150)),
            buy(msft, 1, dec!(5), dec!(200)),
        ];
        let market = prices(&[(msft, dec!(220))]);

        let snapshot = generate_snapshot(&transactions, &market).unwrap();

        assert_eq!(snapshot.positions().len(), 1);
        assert_eq!(snapshot.positions()[0].position().asset_id(), msft);
    }

    #[test]
    fn realized_pnl_of_a_closed_asset_survives_in_the_totals() {
        let apple = AssetId::new();
        let msft = AssetId::new();
        let transactions = [
            buy(apple, 1, dec!(10), dec!(100)),
            sell(apple, 2, dec!(10), dec!(150)), // +500 realized, then closed
            buy(msft, 1, dec!(5), dec!(200)),
        ];
        let market = prices(&[(msft, dec!(220))]);

        let snapshot = generate_snapshot(&transactions, &market).unwrap();

        // Apple has no line in `positions`, but its P&L is still counted.
        assert!(snapshot.positions().iter().all(|p| p.position().asset_id() != apple));
        assert_eq!(snapshot.total_realized_pnl(), dec!(500));
        assert_eq!(snapshot.total_unrealized_pnl(), dec!(100)); // 5*220 - 1000
        assert_eq!(snapshot.total_pnl(), dec!(600));
    }

    #[test]
    fn a_closed_asset_does_not_require_market_data() {
        let asset = AssetId::new();
        let transactions = [
            buy(asset, 1, dec!(10), dec!(100)),
            sell(asset, 2, dec!(10), dec!(150)),
        ];
        // Deliberately empty: nothing is held, so nothing needs a price.
        let market = HashMap::new();

        let snapshot = generate_snapshot(&transactions, &market).unwrap();

        assert!(snapshot.is_empty());
        assert_eq!(snapshot.total_realized_pnl(), dec!(500));
    }

    #[test]
    fn a_held_asset_without_market_data_fails_loud() {
        let asset = AssetId::new();
        let transactions = [buy(asset, 1, dec!(1), dec!(1))];

        let result = generate_snapshot(&transactions, &HashMap::new());

        assert_eq!(result, Err(EngineError::MissingMarketData(asset)));
    }

    #[test]
    fn an_empty_history_produces_an_empty_snapshot() {
        let snapshot = generate_snapshot(&[], &HashMap::new()).unwrap();

        assert!(snapshot.is_empty());
        assert_eq!(snapshot.total_market_value(), Decimal::ZERO);
        assert_eq!(snapshot.total_cost_basis(), Decimal::ZERO);
        assert_eq!(snapshot.total_realized_pnl(), Decimal::ZERO);
        assert_eq!(snapshot.total_unrealized_pnl(), Decimal::ZERO);
    }

    #[test]
    fn positions_come_out_in_a_stable_order() {
        let a = AssetId::new();
        let b = AssetId::new();
        let transactions = [buy(a, 1, dec!(1), dec!(10)), buy(b, 1, dec!(1), dec!(10))];
        let market = prices(&[(a, dec!(10)), (b, dec!(10))]);

        // HashMap iteration order varies run to run; the snapshot must not.
        let first = generate_snapshot(&transactions, &market).unwrap();
        let second = generate_snapshot(&transactions, &market).unwrap();

        let ids = |s: &PortfolioSnapshot| -> Vec<AssetId> {
            s.positions().iter().map(|p| p.position().asset_id()).collect()
        };
        assert_eq!(ids(&first), ids(&second));
        assert_eq!(ids(&first), { let mut v = vec![a, b]; v.sort(); v });
    }

    #[test]
    fn the_engine_is_deterministic_for_the_same_inputs() {
        let asset = AssetId::new();
        let transactions = [
            buy(asset, 1, dec!(10), dec!(100)),
            sell(asset, 2, dec!(3), dec!(140)),
        ];
        let market = prices(&[(asset, dec!(160))]);

        let a = generate_snapshot(&transactions, &market).unwrap();
        let b = generate_snapshot(&transactions, &market).unwrap();

        assert_eq!(a.total_market_value(), b.total_market_value());
        assert_eq!(a.total_cost_basis(), b.total_cost_basis());
        assert_eq!(a.total_realized_pnl(), b.total_realized_pnl());
        assert_eq!(a.total_unrealized_pnl(), b.total_unrealized_pnl());
    }
}
