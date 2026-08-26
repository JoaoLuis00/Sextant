use crate::domain::transaction::Transaction;
use crate::ids::PortfolioId;

/// A thin owner of transaction history — never an Engine input itself.
///
/// The Engine only ever sees `&[Transaction]` (see
/// [`crate::engine::portfolio_engine::generate_snapshot`]), because every
/// calculation is derivable from the history alone. That keeps `Portfolio` out
/// of the calculation path entirely: it holds the log, and nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Portfolio {
    id: PortfolioId,
    name: String,
    transactions: Vec<Transaction>,
}

impl Portfolio {
    pub fn new(id: PortfolioId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            transactions: Vec::new(),
        }
    }

    pub fn id(&self) -> PortfolioId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Signature decision: `&mut self` (mutate in place) over consume-and-return
    /// (`fn apply_transaction(self, tx: Transaction) -> Portfolio`).
    ///
    /// A portfolio is an append-only ledger that keeps growing for the life of
    /// the program — every consuming call site (CLI command handlers, a future
    /// "replay all transactions on load" loop) would otherwise have to write
    /// `portfolio = portfolio.apply_transaction(tx)`, moving the whole struct
    /// (and its `Vec<Transaction>`) on every single append. `&mut self` lets
    /// the backing `Vec` grow with ordinary amortized-push cost and no move.
    /// The cost is that callers need a mutable binding — a fair trade for
    /// something that behaves like a growing log rather than an immutable
    /// value type (contrast with `TransactionType::buy`, which *is* a value
    /// and rightly returns a new one instead of mutating).
    pub fn apply_transaction(&mut self, transaction: Transaction) {
        self.transactions.push(transaction);
    }

    /// Lends the history out for replay. Returns a slice rather than a clone:
    /// the Engine only reads it, so there's no reason to hand over ownership
    /// or duplicate the whole `Vec` on every snapshot.
    pub fn transactions(&self) -> &[Transaction] {
        &self.transactions
    }

    pub fn transaction_count(&self) -> usize {
        self.transactions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.transactions.is_empty()
    }

    /// Sorts the ledger into deterministic replay order, in place.
    ///
    /// Transactions can be appended out of order (backfilling an old trade),
    /// and the Engine's average-cost replay is order-sensitive, so this exists
    /// to normalize before valuing. `&mut self` again — sorting a `Vec` in
    /// place beats rebuilding one.
    pub fn sort_transactions(&mut self) {
        self.transactions.sort_by_key(|tx| tx.ordering_key());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::transaction::TransactionType;
    use crate::ids::AssetId;
    use chrono::NaiveDate;
    use rust_decimal::dec;

    fn day(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2024, 1, day).unwrap()
    }

    fn buy_on(asset: AssetId, d: NaiveDate) -> Transaction {
        Transaction::new(
            asset,
            d,
            TransactionType::buy(dec!(10), dec!(100)).unwrap(),
            None,
        )
        .unwrap()
    }

    #[test]
    fn a_new_portfolio_has_no_transactions() {
        let portfolio = Portfolio::new(PortfolioId::new(), "Retirement");
        assert!(portfolio.is_empty());
        assert_eq!(portfolio.transaction_count(), 0);
    }

    #[test]
    fn apply_transaction_appends_to_the_ledger() {
        let mut portfolio = Portfolio::new(PortfolioId::new(), "Retirement");
        let tx = buy_on(AssetId::new(), day(1));

        portfolio.apply_transaction(tx.clone());

        assert_eq!(portfolio.transactions(), &[tx]);
    }

    #[test]
    fn apply_transaction_preserves_insertion_order() {
        let mut portfolio = Portfolio::new(PortfolioId::new(), "Retirement");
        let asset = AssetId::new();
        let first = buy_on(asset, day(1));
        let second = buy_on(asset, day(2));

        portfolio.apply_transaction(first.clone());
        portfolio.apply_transaction(second.clone());

        assert_eq!(portfolio.transactions(), &[first, second]);
    }

    #[test]
    fn sort_transactions_puts_a_backfilled_trade_back_in_date_order() {
        let mut portfolio = Portfolio::new(PortfolioId::new(), "Retirement");
        let asset = AssetId::new();
        let later = buy_on(asset, day(20));
        let backfilled = buy_on(asset, day(2));

        portfolio.apply_transaction(later.clone());
        portfolio.apply_transaction(backfilled.clone());
        portfolio.sort_transactions();

        assert_eq!(portfolio.transactions(), &[backfilled, later]);
    }
}
