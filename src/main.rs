//! Thin demo binary.
//!
//! Placeholder until Phase 7 replaces it with a real `clap` CLI — it exists so
//! the library can be exercised end to end from the command line. All the
//! logic lives in the library; `main` only wires inputs and prints outputs.

use std::collections::HashMap;

use chrono::{NaiveDate, Utc};
use rust_decimal::dec;

use sextant::{
    generate_snapshot, Asset, AssetId, AssetType, Currency, MarketData, Portfolio, PortfolioId,
    PortfolioSnapshot, Ticker, Transaction, TransactionType,
};

fn main() -> sextant::Result<()> {
    let apple_id = AssetId::new();
    let apple = Asset::new(
        apple_id,
        Ticker::new("AAPL"),
        "Apple Inc.".to_string(),
        "NASDAQ".to_string(),
        Currency::Usd,
        AssetType::Stock,
    )
    .with_classification(
        Some("Technology".to_string()),
        Some("Consumer Electronics".to_string()),
        Some("US".to_string()),
    );

    let vwce_id = AssetId::new();
    let vwce = Asset::new(
        vwce_id,
        Ticker::new("VWCE"),
        "Vanguard FTSE All-World".to_string(),
        "XETRA".to_string(),
        Currency::Usd,
        AssetType::Etf,
    );

    let mut portfolio = Portfolio::new(PortfolioId::new(), "Retirement");
    for tx in [
        // Apple: buy, partial sell — stays open with realized P&L behind it.
        transaction(apple_id, 2024, 1, 15, TransactionType::buy(dec!(10), dec!(150))?)?,
        transaction(apple_id, 2024, 6, 1, TransactionType::sell(dec!(4), dec!(180))?)?,
        // VWCE: bought, fully exited — proves closed positions keep their P&L
        // without appearing as a holding.
        transaction(vwce_id, 2024, 2, 1, TransactionType::buy(dec!(20), dec!(100))?)?,
        transaction(vwce_id, 2024, 8, 1, TransactionType::sell(dec!(20), dec!(115))?)?,
    ] {
        portfolio.apply_transaction(tx);
    }
    portfolio.sort_transactions();

    // Only the still-held asset needs a price. VWCE is fully sold, so the
    // Engine never asks for one.
    let mut market_data = HashMap::new();
    market_data.insert(apple_id, MarketData::new(apple_id, dec!(210), Utc::now()));

    let snapshot = generate_snapshot(portfolio.transactions(), &market_data)?;
    print_report(&portfolio, &[&apple, &vwce], &snapshot);

    Ok(())
}

fn transaction(
    asset_id: AssetId,
    year: i32,
    month: u32,
    day: u32,
    kind: TransactionType,
) -> sextant::Result<Transaction> {
    let date = NaiveDate::from_ymd_opt(year, month, day).expect("hardcoded demo date is valid");
    Ok(Transaction::new(asset_id, date, kind, None)?)
}

fn print_report(portfolio: &Portfolio, assets: &[&Asset], snapshot: &PortfolioSnapshot) {
    println!("Portfolio: {}", portfolio.name());
    println!("Transactions: {}", portfolio.transaction_count());
    println!("Generated at: {}", snapshot.generated_at());
    println!();

    println!("Holdings");
    if snapshot.is_empty() {
        println!("  (none — every position has been closed)");
    }
    for valuation in snapshot.positions() {
        let position = valuation.position();
        let label = assets
            .iter()
            .find(|a| a.id() == position.asset_id())
            .map(|a| a.ticker().to_string())
            .unwrap_or_else(|| position.asset_id().to_string());

        println!(
            "  {:<6} {:>8} @ avg {:>8}  value {:>10}  unrealized {:>10}",
            label,
            position.quantity(),
            position.average_cost(),
            valuation.market_value(),
            valuation.unrealized_pnl(),
        );
    }

    println!();
    println!("Totals");
    println!("  Cost basis      {:>12}", snapshot.total_cost_basis());
    println!("  Market value    {:>12}", snapshot.total_market_value());
    println!("  Realized P&L    {:>12}", snapshot.total_realized_pnl());
    println!("  Unrealized P&L  {:>12}", snapshot.total_unrealized_pnl());
    println!("  Total P&L       {:>12}", snapshot.total_pnl());
}
