//! Persistence port.
//!
//! The Engine defines the trait; the storage layer implements it. That
//! direction is the whole point — `engine/` never learns that SQLite exists,
//! and swapping `rusqlite` for something else touches only the implementor.

use crate::domain::errors::TransactionError;
use crate::domain::transaction::Transaction;
use crate::ids::TransactionId;
use std::collections::HashMap;

/// Generic over the stored type, with `Id` and `Error` as associated types
/// rather than extra parameters: each implementor has exactly one natural id
/// and one natural error, so pinning them per-impl reads better at call sites
/// than `Repository<Transaction, TransactionId, TransactionError>`.
pub trait Repository<T> {
    type Id;
    type Error;

    fn save(&mut self, item: T) -> Result<Self::Id, Self::Error>;
    fn find_by_id(&self, id: &Self::Id) -> Result<Option<T>, Self::Error>;
    fn find_all(&self) -> Result<Vec<T>, Self::Error>;
    fn delete(&mut self, id: &Self::Id) -> Result<(), Self::Error>;
}

/// Used during implementation in place of SQLite — lets the Engine and CLI be
/// built and tested before Phase 5 lands any real persistence.
#[derive(Debug, Clone, Default)]
pub struct InMemoryTransactionRepository {
    transactions: HashMap<TransactionId, Transaction>,
}

impl InMemoryTransactionRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.transactions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.transactions.is_empty()
    }
}

impl Repository<Transaction> for InMemoryTransactionRepository {
    type Id = TransactionId;
    type Error = TransactionError;

    fn save(&mut self, item: Transaction) -> Result<Self::Id, Self::Error> {
        let id = item.id();
        self.transactions.insert(id, item);
        Ok(id)
    }

    fn find_by_id(&self, id: &Self::Id) -> Result<Option<Transaction>, Self::Error> {
        Ok(self.transactions.get(id).cloned())
    }

    /// Returned in deterministic `(date, id)` order rather than `HashMap`
    /// order, so a caller can feed the result straight into the Engine —
    /// average-cost replay is order-sensitive.
    fn find_all(&self) -> Result<Vec<Transaction>, Self::Error> {
        let mut all: Vec<Transaction> = self.transactions.values().cloned().collect();
        all.sort_by_key(|tx| tx.ordering_key());
        Ok(all)
    }

    fn delete(&mut self, id: &Self::Id) -> Result<(), Self::Error> {
        self.transactions.remove(id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::transaction::TransactionType;
    use crate::ids::AssetId;
    use chrono::NaiveDate;
    use rust_decimal::dec;

    fn tx_on(asset: AssetId, day: u32) -> Transaction {
        Transaction::new(
            asset,
            NaiveDate::from_ymd_opt(2024, 1, day).unwrap(),
            TransactionType::buy(dec!(1), dec!(10)).unwrap(),
            None,
        )
        .unwrap()
    }

    #[test]
    fn save_then_find_by_id_round_trips() {
        let mut repo = InMemoryTransactionRepository::new();
        let tx = tx_on(AssetId::for_ticker("AAPL", "NASDAQ"), 1);

        let id = repo.save(tx.clone()).unwrap();

        assert_eq!(repo.find_by_id(&id).unwrap(), Some(tx));
    }

    #[test]
    fn find_by_id_returns_none_for_an_unknown_id() {
        let repo = InMemoryTransactionRepository::new();
        assert_eq!(repo.find_by_id(&TransactionId::new()).unwrap(), None);
    }

    #[test]
    fn delete_removes_the_transaction() {
        let mut repo = InMemoryTransactionRepository::new();
        let id = repo
            .save(tx_on(AssetId::for_ticker("AAPL", "NASDAQ"), 1))
            .unwrap();

        repo.delete(&id).unwrap();

        assert_eq!(repo.find_by_id(&id).unwrap(), None);
        assert!(repo.is_empty());
    }

    #[test]
    fn deleting_an_absent_id_is_not_an_error() {
        let mut repo = InMemoryTransactionRepository::new();
        assert!(repo.delete(&TransactionId::new()).is_ok());
    }

    #[test]
    fn saving_the_same_id_twice_updates_rather_than_duplicates() {
        let mut repo = InMemoryTransactionRepository::new();
        let tx = tx_on(AssetId::for_ticker("AAPL", "NASDAQ"), 1);
        repo.save(tx.clone()).unwrap();
        repo.save(tx).unwrap();

        assert_eq!(repo.len(), 1);
    }

    #[test]
    fn find_all_returns_transactions_in_replay_order() {
        let mut repo = InMemoryTransactionRepository::new();
        let asset = AssetId::for_ticker("AAPL", "NASDAQ");
        let later = tx_on(asset, 20);
        let earlier = tx_on(asset, 2);

        repo.save(later.clone()).unwrap();
        repo.save(earlier.clone()).unwrap();

        assert_eq!(repo.find_all().unwrap(), vec![earlier, later]);
    }
}
