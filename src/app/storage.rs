//! SQLite-backed `Repository<Transaction>` and `Repository<Asset>`.
//!
//! Decimals and dates are stored as `TEXT`, not `REAL`, to keep
//! `rust_decimal` precision exact through a round trip.

use std::str::FromStr;

use chrono::NaiveDate;
use rusqlite::{Connection, params};
use rust_decimal::Decimal;
use thiserror::Error;

use crate::domain::asset::{Asset, AssetType, Currency, Ticker};
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

type RawAssetRow = (
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
);

pub struct SqliteAssetRepository {
    conn: Connection,
}

impl SqliteAssetRepository {
    pub fn open(conn: Connection) -> Result<Self, StorageError> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS assets (
                id TEXT PRIMARY KEY,
                ticker TEXT NOT NULL,
                exchange TEXT NOT NULL,
                name TEXT NOT NULL,
                currency TEXT NOT NULL,
                asset_type TEXT NOT NULL,
                sector TEXT,
                industry TEXT,
                country TEXT
            )",
            [],
        )?;
        Ok(Self { conn })
    }

    /// No `id` column read back here — `Asset::new` re-derives it from
    /// `(ticker, exchange)` via `AssetId::for_ticker`, which always
    /// reproduces exactly what's already in the row that got us here.
    fn parse_row(
        (ticker, exchange, name, currency, asset_type, sector, industry, country): RawAssetRow,
    ) -> Result<Asset, StorageError> {
        let currency = Currency::from_str(&currency).map_err(invalid)?;
        let asset_type = AssetType::from_str(&asset_type).map_err(invalid)?;

        Ok(
            Asset::new(Ticker::new(ticker), name, exchange, currency, asset_type)
                .with_classification(sector, industry, country),
        )
    }
}

impl Repository<Asset> for SqliteAssetRepository {
    type Id = AssetId;
    type Error = StorageError;

    fn save(&mut self, item: Asset) -> Result<Self::Id, Self::Error> {
        self.conn.execute(
            "INSERT INTO assets (id, ticker, exchange, name, currency, asset_type, sector, industry, country)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET
                ticker = excluded.ticker,
                exchange = excluded.exchange,
                name = excluded.name,
                currency = excluded.currency,
                asset_type = excluded.asset_type,
                sector = excluded.sector,
                industry = excluded.industry,
                country = excluded.country",
            params![
                item.id().to_string(),
                item.ticker().as_str(),
                item.exchange(),
                item.name(),
                item.currency().code(),
                item.asset_type().label(),
                item.sector(),
                item.industry(),
                item.country(),
            ],
        )?;
        Ok(item.id())
    }

    fn find_by_id(&self, id: &Self::Id) -> Result<Option<Asset>, Self::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT ticker, exchange, name, currency, asset_type, sector, industry, country
             FROM assets WHERE id = ?1",
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
                row.get(7)?,
            ))?)),
            None => Ok(None),
        }
    }

    fn find_all(&self) -> Result<Vec<Asset>, Self::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT ticker, exchange, name, currency, asset_type, sector, industry, country
             FROM assets",
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
                row.get(7)?,
            ))
        })?;

        rows.map(|row| Self::parse_row(row?))
            .collect::<Result<Vec<_>, _>>()
    }

    fn delete(&mut self, id: &Self::Id) -> Result<(), Self::Error> {
        self.conn
            .execute("DELETE FROM assets WHERE id = ?1", params![id.to_string()])?;
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
        let tx = tx_on(AssetId::for_ticker("AAPL", "NASDAQ"), 1);

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
            AssetId::for_ticker("AAPL", "NASDAQ"),
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
        let tx = tx_on(AssetId::for_ticker("AAPL", "NASDAQ"), 1);
        repo.save(tx.clone()).unwrap();
        repo.save(tx).unwrap();

        assert_eq!(repo.find_all().unwrap().len(), 1);
    }

    #[test]
    fn delete_removes_the_transaction() {
        let mut repo = repo();
        let id = repo
            .save(tx_on(AssetId::for_ticker("AAPL", "NASDAQ"), 1))
            .unwrap();

        repo.delete(&id).unwrap();

        assert_eq!(repo.find_by_id(&id).unwrap(), None);
    }

    #[test]
    fn find_all_returns_transactions_in_replay_order() {
        let mut repo = repo();
        let asset = AssetId::for_ticker("AAPL", "NASDAQ");
        let later = tx_on(asset, 20);
        let earlier = tx_on(asset, 2);

        repo.save(later.clone()).unwrap();
        repo.save(earlier.clone()).unwrap();

        assert_eq!(repo.find_all().unwrap(), vec![earlier, later]);
    }

    fn asset_repo() -> SqliteAssetRepository {
        SqliteAssetRepository::open(Connection::open_in_memory().unwrap()).unwrap()
    }

    fn apple() -> Asset {
        Asset::new(
            Ticker::new("AAPL"),
            "Apple Inc.".to_string(),
            "NASDAQ".to_string(),
            Currency::Usd,
            AssetType::Stock,
        )
    }

    #[test]
    fn save_then_find_asset_by_id_round_trips() {
        let mut repo = asset_repo();
        let asset = apple();

        repo.save(asset.clone()).unwrap();

        assert_eq!(repo.find_by_id(&asset.id()).unwrap(), Some(asset));
    }

    #[test]
    fn a_saved_assets_id_is_derived_not_stored_verbatim() {
        let mut repo = asset_repo();
        repo.save(apple()).unwrap();

        let expected_id = AssetId::for_ticker("AAPL", "NASDAQ");
        assert_eq!(
            repo.find_by_id(&expected_id).unwrap().map(|a| a.id()),
            Some(expected_id)
        );
    }

    #[test]
    fn find_asset_by_id_returns_none_for_an_unknown_id() {
        let repo = asset_repo();
        assert_eq!(
            repo.find_by_id(&AssetId::for_ticker("MISSING", "TEST"))
                .unwrap(),
            None
        );
    }

    #[test]
    fn saving_the_same_asset_twice_updates_rather_than_duplicates() {
        let mut repo = asset_repo();
        let corrected = apple().with_classification(
            Some("Technology".to_string()),
            Some("Consumer Electronics".to_string()),
            Some("US".to_string()),
        );

        repo.save(apple()).unwrap();
        repo.save(corrected.clone()).unwrap();

        assert_eq!(repo.find_all().unwrap(), vec![corrected]);
    }

    #[test]
    fn delete_removes_the_asset() {
        let mut repo = asset_repo();
        let asset = apple();
        repo.save(asset.clone()).unwrap();

        repo.delete(&asset.id()).unwrap();

        assert_eq!(repo.find_by_id(&asset.id()).unwrap(), None);
    }

    #[test]
    fn find_all_assets_returns_everything_saved() {
        let mut repo = asset_repo();
        let apple = apple();
        let vwce = Asset::new(
            Ticker::new("VWCE"),
            "Vanguard FTSE All-World".to_string(),
            "XETRA".to_string(),
            Currency::Usd,
            AssetType::Etf,
        );

        repo.save(apple.clone()).unwrap();
        repo.save(vwce.clone()).unwrap();

        let mut all = repo.find_all().unwrap();
        all.sort_by_key(|a| a.id());
        let mut expected = vec![apple, vwce];
        expected.sort_by_key(|a| a.id());

        assert_eq!(all, expected);
    }
}
