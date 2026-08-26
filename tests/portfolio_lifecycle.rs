//! Integration tests — public API only.
//!
//! These reach the crate the same way an external consumer would (`use
//! portfolio::...`), so anything these tests touch is, by definition, part of
//! the public surface. If something needed here is private, that's a signal
//! about the API rather than a reason to reach inside.

use std::collections::HashMap;

use chrono::{NaiveDate, Utc};
use portfolio::{
    generate_snapshot, AssetId, EngineError, InMemoryTransactionRepository, MarketData,
    MarketDataProvider, MockProvider, Portfolio, PortfolioId, Repository, Transaction,
    TransactionError, TransactionType,
};
use rust_decimal::{dec, Decimal};

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

#[test]
fn a_full_portfolio_lifecycle_produces_the_expected_snapshot() {
    let apple = AssetId::new();
    let etf = AssetId::new();

    let mut portfolio = Portfolio::new(PortfolioId::new(), "Retirement");
    portfolio.apply_transaction(buy(apple, 1, dec!(10), dec!(150)));
    portfolio.apply_transaction(buy(etf, 2, dec!(20), dec!(100)));
    portfolio.apply_transaction(sell(apple, 3, dec!(4), dec!(180)));

    let market = prices(&[(apple, dec!(210)), (etf, dec!(110))]);
    let snapshot = generate_snapshot(portfolio.transactions(), &market).unwrap();

    assert_eq!(snapshot.positions().len(), 2);

    // Apple: 6 left @ avg 150 -> basis 900, value 1260, unrealized 360.
    // Realized on the 4 sold: (180 - 150) * 4 = 120.
    // ETF:   20 @ 100 -> basis 2000, value 2200, unrealized 200.
    assert_eq!(snapshot.total_cost_basis(), dec!(2900));
    assert_eq!(snapshot.total_market_value(), dec!(3460));
    assert_eq!(snapshot.total_realized_pnl(), dec!(120));
    assert_eq!(snapshot.total_unrealized_pnl(), dec!(560));
    assert_eq!(snapshot.total_pnl(), dec!(680));
}

#[test]
fn closing_a_position_keeps_its_realized_pnl_but_drops_its_holding() {
    let apple = AssetId::new();
    let etf = AssetId::new();

    let mut portfolio = Portfolio::new(PortfolioId::new(), "Retirement");
    portfolio.apply_transaction(buy(apple, 1, dec!(10), dec!(100)));
    portfolio.apply_transaction(sell(apple, 2, dec!(10), dec!(150)));
    portfolio.apply_transaction(buy(etf, 3, dec!(5), dec!(200)));

    // Apple is fully sold, so no price is supplied for it at all.
    let market = prices(&[(etf, dec!(220))]);
    let snapshot = generate_snapshot(portfolio.transactions(), &market).unwrap();

    assert_eq!(snapshot.positions().len(), 1);
    assert_eq!(snapshot.positions()[0].position().asset_id(), etf);
    assert_eq!(snapshot.total_realized_pnl(), dec!(500));
}

#[test]
fn a_missing_price_for_a_held_asset_is_reported_loudly() {
    let apple = AssetId::new();
    let mut portfolio = Portfolio::new(PortfolioId::new(), "Retirement");
    portfolio.apply_transaction(buy(apple, 1, dec!(10), dec!(150)));

    let result = generate_snapshot(portfolio.transactions(), &HashMap::new());

    assert_eq!(result, Err(EngineError::MissingMarketData(apple)));
}

#[test]
fn overselling_an_asset_is_rejected() {
    let apple = AssetId::new();
    let mut portfolio = Portfolio::new(PortfolioId::new(), "Retirement");
    portfolio.apply_transaction(buy(apple, 1, dec!(5), dec!(150)));
    portfolio.apply_transaction(sell(apple, 2, dec!(6), dec!(180)));

    let market = prices(&[(apple, dec!(210))]);
    let result = generate_snapshot(portfolio.transactions(), &market);

    assert!(matches!(
        result,
        Err(EngineError::OversoldAsset { .. })
    ));
}

#[test]
fn invalid_transactions_are_rejected_at_construction() {
    let apple = AssetId::new();

    assert_eq!(
        Transaction::new(
            apple,
            day(1),
            TransactionType::Buy {
                quantity: Decimal::ZERO,
                price: dec!(100),
            },
            None,
        ),
        Err(TransactionError::NonPositiveQuantity)
    );
}

#[test]
fn an_empty_portfolio_snapshots_cleanly() {
    let portfolio = Portfolio::new(PortfolioId::new(), "Empty");
    let snapshot = generate_snapshot(portfolio.transactions(), &HashMap::new()).unwrap();

    assert!(snapshot.is_empty());
    assert_eq!(snapshot.total_pnl(), Decimal::ZERO);
}

#[test]
fn out_of_order_entry_is_normalized_before_replay() {
    let apple = AssetId::new();

    // Same trades, entered in different orders. Average-cost replay is
    // order-sensitive, so sorting must make the two agree.
    let mut in_order = Portfolio::new(PortfolioId::new(), "A");
    in_order.apply_transaction(buy(apple, 1, dec!(10), dec!(100)));
    in_order.apply_transaction(buy(apple, 2, dec!(10), dec!(200)));
    in_order.apply_transaction(sell(apple, 3, dec!(5), dec!(300)));

    let mut backfilled = Portfolio::new(PortfolioId::new(), "B");
    backfilled.apply_transaction(buy(apple, 2, dec!(10), dec!(200)));
    backfilled.apply_transaction(sell(apple, 3, dec!(5), dec!(300)));
    backfilled.apply_transaction(buy(apple, 1, dec!(10), dec!(100)));
    backfilled.sort_transactions();

    let market = prices(&[(apple, dec!(250))]);
    let a = generate_snapshot(in_order.transactions(), &market).unwrap();
    let b = generate_snapshot(backfilled.transactions(), &market).unwrap();

    assert_eq!(a.total_cost_basis(), b.total_cost_basis());
    assert_eq!(a.total_realized_pnl(), b.total_realized_pnl());
    assert_eq!(a.total_unrealized_pnl(), b.total_unrealized_pnl());
}

#[test]
fn transactions_survive_a_round_trip_through_the_repository() {
    let apple = AssetId::new();
    let mut repo = InMemoryTransactionRepository::new();

    repo.save(sell(apple, 5, dec!(4), dec!(180))).unwrap();
    repo.save(buy(apple, 1, dec!(10), dec!(150))).unwrap();

    // `find_all` returns replay order, so it can feed the Engine directly.
    let history = repo.find_all().unwrap();
    let market = prices(&[(apple, dec!(210))]);
    let snapshot = generate_snapshot(&history, &market).unwrap();

    assert_eq!(snapshot.positions().len(), 1);
    assert_eq!(snapshot.total_realized_pnl(), dec!(120));
}

#[test]
fn a_provider_can_supply_the_prices_the_engine_needs() {
    let apple = AssetId::new();
    let provider = MockProvider::new().with_price(apple, dec!(210));

    let mut portfolio = Portfolio::new(PortfolioId::new(), "Retirement");
    portfolio.apply_transaction(buy(apple, 1, dec!(10), dec!(150)));

    // The impure step: fetch prices, then hand them to the pure Engine as data.
    let market: HashMap<AssetId, MarketData> = portfolio
        .transactions()
        .iter()
        .map(|tx| tx.asset_id())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .map(|id| {
            let price = provider.price(&id)?;
            Ok((id, MarketData::new(id, price, Utc::now())))
        })
        .collect::<Result<_, portfolio::MarketDataError>>()
        .unwrap();

    let snapshot = generate_snapshot(portfolio.transactions(), &market).unwrap();

    assert_eq!(snapshot.total_market_value(), dec!(2100));
}
