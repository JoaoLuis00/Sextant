use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TransactionError {
    #[error("quantity must be positive")]
    NonPositiveQuantity,
    #[error("price must be positive")]
    NonPositivePrice,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AssetError {
    #[error("unknown currency code: {0}")]
    UnknownCurrency(String),
    #[error("unknown asset type: {0}")]
    UnknownAssetType(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DomainError {
    #[error(transparent)]
    Transaction(#[from] TransactionError),
    #[error(transparent)]
    Asset(#[from] AssetError),
}
