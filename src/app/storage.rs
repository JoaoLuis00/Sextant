//! SQLite-backed `Repository<Transaction>`.
//!
//! Decimals and dates are stored as `TEXT`, not `REAL`, to keep
//! `rust_decimal` precision exact through a round trip.

use std::str::FromStr;

use chrono::NaiveDate;
use rusqlite::{Connection, params};
use rust_decimal::Decimal;
use thiserror::Error;

use crate::domain::transaction::{Transaction, TransactionType};
use crate::engine::repository::Repository;
use crate::ids::{AssetId, TransactionId};

#[derive(Debug, Error)]
pub enum StorageError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error("invalid stored transaction: {0}")]
    InvalidData(String),
}

type RawRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
);

fn invalid(e: impl std::fmt::Display) -> StorageError {
    StorageError::InvalidData(e.to_string())
}

pub struct SqliteTransactionRepository {
    conn: Connection,
}

impl SqliteTransactionRepository {
    pub fn open(conn: Connection) -> Result<Self, StorageError> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS transactions (
                id TEXT PRIMARY KEY,
                asset_id TEXT NOT NULL,
                date TEXT NOT NULL,
                kind TEXT NOT NULL,
                quantity TEXT NOT NULL,
                price TEXT NOT NULL,
                notes TEXT
            )",
            [],
        )?;
        Ok(Self { conn })
    }

    fn parse_row(
        (id, asset_id, date, kind, quantity, price, notes): RawRow,
    ) -> Result<Transaction, StorageError> {
        let id = TransactionId::from_str(&id).map_err(invalid)?;
        let asset_id = AssetId::from_str(&asset_id).map_err(invalid)?;
        let date = NaiveDate::from_str(&date).map_err(invalid)?;
        let quantity = Decimal::from_str(&quantity).map_err(invalid)?;
        let price = Decimal::from_str(&price).map_err(invalid)?;

        let transaction_type = match kind.as_str() {
            "buy" => TransactionType::buy(quantity, price),
            "sell" => TransactionType::sell(quantity, price),
            other => return Err(StorageError::InvalidData(format!("unknown kind: {other}"))),
        }
        .map_err(invalid)?;

        Ok(Transaction::from_stored(
            id,
            asset_id,
            date,
            transaction_type,
            notes,
        ))
    }
}

impl Repository<Transaction> for SqliteTransactionRepository {
    type Id = TransactionId;
    type Error = StorageError;

    fn save(&mut self, item: Transaction) -> Result<Self::Id, Self::Error> {
        let (kind, quantity, price) = match *item.transaction_type() {
            TransactionType::Buy { quantity, price } => ("buy", quantity, price),
            TransactionType::Sell { quantity, price } => ("sell", quantity, price),
        };

        self.conn.execute(
            "INSERT INTO transactions (id, asset_id, date, kind, quantity, price, notes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
                asset_id = excluded.asset_id,
                date = excluded.date,
                kind = excluded.kind,
                quantity = excluded.quantity,
                price = excluded.price,
                notes = excluded.notes",
            params![
                item.id().to_string(),
                item.asset_id().to_string(),
                item.date().to_string(),
                kind,
                quantity.to_string(),
                price.to_string(),
                item.notes(),
            ],
        )?;
        Ok(item.id())
    }

    fn find_by_id(&self, id: &Self::Id) -> Result<Option<Transaction>, Self::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, asset_id, date, kind, quantity, price, notes
             FROM transactions WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id.to_string()])?;
        match rows.next()? {
            Some(row) => Ok(Some(Self::parse_row((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
            ))?)),
            None => Ok(None),
        }
    }

    /// Ordered by `(date, id)`, matching `InMemoryTransactionRepository`.
    fn find_all(&self) -> Result<Vec<Transaction>, Self::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, asset_id, date, kind, quantity, price, notes
             FROM transactions ORDER BY date, id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
            ))
        })?;

        rows.map(|row| Self::parse_row(row?))
            .collect::<Result<Vec<_>, _>>()
    }

    fn delete(&mut self, id: &Self::Id) -> Result<(), Self::Error> {
        self.conn.execute(
            "DELETE FROM transactions WHERE id = ?1",
            params![id.to_string()],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    fn repo() -> SqliteTransactionRepository {
        SqliteTransactionRepository::open(Connection::open_in_memory().unwrap()).unwrap()
    }

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
        let mut repo = repo();
        let tx = tx_on(AssetId::new(), 1);

        let id = repo.save(tx.clone()).unwrap();

        assert_eq!(repo.find_by_id(&id).unwrap(), Some(tx));
    }

    #[test]
    fn find_by_id_returns_none_for_an_unknown_id() {
        assert_eq!(repo().find_by_id(&TransactionId::new()).unwrap(), None);
    }

    #[test]
    fn sell_transaction_and_notes_round_trip() {
        let mut repo = repo();
        let tx = Transaction::new(
            AssetId::new(),
            NaiveDate::from_ymd_opt(2024, 3, 1).unwrap(),
            TransactionType::sell(dec!(2.5), dec!(99.99)).unwrap(),
            Some("trimmed position".to_string()),
        )
        .unwrap();

        let id = repo.save(tx.clone()).unwrap();

        assert_eq!(repo.find_by_id(&id).unwrap(), Some(tx));
    }

    #[test]
    fn saving_the_same_id_twice_updates_rather_than_duplicates() {
        let mut repo = repo();
        let tx = tx_on(AssetId::new(), 1);
        repo.save(tx.clone()).unwrap();
        repo.save(tx).unwrap();

        assert_eq!(repo.find_all().unwrap().len(), 1);
    }

    #[test]
    fn delete_removes_the_transaction() {
        let mut repo = repo();
        let id = repo.save(tx_on(AssetId::new(), 1)).unwrap();

        repo.delete(&id).unwrap();

        assert_eq!(repo.find_by_id(&id).unwrap(), None);
    }

    #[test]
    fn find_all_returns_transactions_in_replay_order() {
        let mut repo = repo();
        let asset = AssetId::new();
        let later = tx_on(asset, 20);
        let earlier = tx_on(asset, 2);

        repo.save(later.clone()).unwrap();
        repo.save(earlier.clone()).unwrap();

        assert_eq!(repo.find_all().unwrap(), vec![earlier, later]);
    }
}
