use crate::domain::errors::TransactionError;
use crate::ids::AssetId;
use crate::ids::TransactionId;
use chrono::NaiveDate;
use rust_decimal::Decimal;

/// Data-carrying enum rather than a flat tag plus `quantity`/`price` fields.
///
/// This is what lets `Dividend`/`Split`/`Interest` be added later without
/// those variants dragging along a `quantity` and `price` that mean nothing
/// for them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionType {
    Buy { quantity: Decimal, price: Decimal },
    Sell { quantity: Decimal, price: Decimal },
    // later, each variant only carries what it actually needs:
    // Dividend { amount: Decimal },
    // Split { ratio: Decimal },
    // Interest { amount: Decimal },
}

impl TransactionType {
    pub fn buy(quantity: Decimal, price: Decimal) -> Result<Self, TransactionError> {
        Self::validate(quantity, price)?;
        Ok(Self::Buy { quantity, price })
    }

    pub fn sell(quantity: Decimal, price: Decimal) -> Result<Self, TransactionError> {
        Self::validate(quantity, price)?;
        Ok(Self::Sell { quantity, price })
    }

    fn validate(quantity: Decimal, price: Decimal) -> Result<(), TransactionError> {
        if quantity <= Decimal::ZERO {
            return Err(TransactionError::NonPositiveQuantity);
        }
        if price <= Decimal::ZERO {
            return Err(TransactionError::NonPositivePrice);
        }
        Ok(())
    }

    /// Re-checks an already-constructed value — cheap safety net in case
    /// something bypassed `buy()`/`sell()` via a direct struct literal.
    fn validate_self(&self) -> Result<(), TransactionError> {
        match self {
            Self::Buy { quantity, price } | Self::Sell { quantity, price } => {
                Self::validate(*quantity, *price)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    id: TransactionId,
    asset_id: AssetId,
    date: NaiveDate,
    transaction_type: TransactionType,
    notes: Option<String>,
    //fees:
    //taxes
    //broker
}

impl Transaction {
    /// Smart constructor — enforces the "quantity and price always positive"
    /// rule at construction time instead of leaving it as a doc comment.
    pub fn new(
        asset_id: AssetId,
        date: NaiveDate,
        transaction_type: TransactionType,
        notes: Option<String>,
    ) -> Result<Self, TransactionError> {
        transaction_type.validate_self()?;
        Ok(Self {
            id: TransactionId::new(),
            asset_id,
            date,
            transaction_type,
            notes,
        })
    }

    /// Rebuilds a `Transaction` with a caller-supplied id instead of minting
    /// one, for repositories reloading a row that already has one. Not
    /// `pub`: only code inside this crate should ever set an id directly.
    pub(crate) fn from_stored(
        id: TransactionId,
        asset_id: AssetId,
        date: NaiveDate,
        transaction_type: TransactionType,
        notes: Option<String>,
    ) -> Self {
        Self {
            id,
            asset_id,
            date,
            transaction_type,
            notes,
        }
    }

    pub fn id(&self) -> TransactionId {
        self.id
    }

    pub fn asset_id(&self) -> AssetId {
        self.asset_id
    }

    pub fn date(&self) -> NaiveDate {
        self.date
    }

    pub fn transaction_type(&self) -> &TransactionType {
        &self.transaction_type
    }

    pub fn notes(&self) -> Option<&str> {
        self.notes.as_deref()
    }

    /// Sort key giving a deterministic replay order. Same-day transactions
    /// fall back to the UUIDv7 id, which is time-ordered by construction, so
    /// they come out in creation order with no extra sequence field.
    pub fn ordering_key(&self) -> (NaiveDate, TransactionId) {
        (self.date, self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    fn date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()
    }

    #[test]
    fn buy_rejects_non_positive_quantity() {
        assert_eq!(
            TransactionType::buy(Decimal::ZERO, dec!(100)),
            Err(TransactionError::NonPositiveQuantity)
        );
        assert_eq!(
            TransactionType::buy(dec!(-5), dec!(100)),
            Err(TransactionError::NonPositiveQuantity)
        );
    }

    #[test]
    fn buy_rejects_non_positive_price() {
        assert_eq!(
            TransactionType::buy(dec!(10), Decimal::ZERO),
            Err(TransactionError::NonPositivePrice)
        );
    }

    #[test]
    fn sell_applies_the_same_validation_as_buy() {
        assert_eq!(
            TransactionType::sell(dec!(-1), dec!(100)),
            Err(TransactionError::NonPositiveQuantity)
        );
        assert_eq!(
            TransactionType::sell(dec!(1), dec!(-100)),
            Err(TransactionError::NonPositivePrice)
        );
    }

    #[test]
    fn new_propagates_validation_failures() {
        // Bypasses the `buy()` smart constructor with a struct literal —
        // `Transaction::new` must still catch it.
        let bad = TransactionType::Buy {
            quantity: Decimal::ZERO,
            price: dec!(100),
        };
        assert_eq!(
            Transaction::new(AssetId::new(), date(), bad, None),
            Err(TransactionError::NonPositiveQuantity)
        );
    }

    #[test]
    fn new_builds_a_valid_transaction() {
        let asset = AssetId::new();
        let tx = Transaction::new(
            asset,
            date(),
            TransactionType::buy(dec!(10), dec!(150)).unwrap(),
            Some("initial position".to_string()),
        )
        .unwrap();

        assert_eq!(tx.asset_id(), asset);
        assert_eq!(tx.date(), date());
        assert_eq!(tx.notes(), Some("initial position"));
    }

    #[test]
    fn same_day_transactions_order_by_creation_time() {
        let asset = AssetId::new();
        // All on the same date, minted back to back — so the date component of
        // the sort key ties and the UUIDv7 id is what actually breaks it.
        let transactions: Vec<Transaction> = (0..100)
            .map(|_| {
                Transaction::new(
                    asset,
                    date(),
                    TransactionType::buy(dec!(1), dec!(1)).unwrap(),
                    None,
                )
                .unwrap()
            })
            .collect();

        for pair in transactions.windows(2) {
            assert!(pair[0].ordering_key() < pair[1].ordering_key());
        }
    }

    #[test]
    fn sorting_by_ordering_key_recovers_creation_order() {
        let asset = AssetId::new();
        let original: Vec<Transaction> = (0..50)
            .map(|_| {
                Transaction::new(
                    asset,
                    date(),
                    TransactionType::buy(dec!(1), dec!(1)).unwrap(),
                    None,
                )
                .unwrap()
            })
            .collect();

        let mut shuffled = original.clone();
        shuffled.reverse();
        shuffled.sort_by_key(|tx| tx.ordering_key());

        assert_eq!(shuffled, original);
    }
}
