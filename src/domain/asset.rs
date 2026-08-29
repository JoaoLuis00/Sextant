use std::fmt;
use std::str::FromStr;

use crate::errors::AssetError;
use crate::ids::AssetId;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ticker(String);

impl Ticker {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    //Example of a method which we can invoke to retrive the Ticker as &str (string slice)
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Ticker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Enum rather than `String` so `"USD"` / `"Usd"` / `"$"` can't drift apart
/// across the codebase — a typo becomes a compile error instead of a silent
/// mismatch when totals get grouped by currency later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Currency {
    Usd,
    Eur,
    Gbp,
    Chf,
    Jpy,
}

impl Currency {
    /// ISO 4217 code — the form used on the wire and in storage.
    pub fn code(&self) -> &'static str {
        match self {
            Currency::Usd => "USD",
            Currency::Eur => "EUR",
            Currency::Gbp => "GBP",
            Currency::Chf => "CHF",
            Currency::Jpy => "JPY",
        }
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl FromStr for Currency {
    type Err = AssetError;

    /// Parses at the input boundary (CLI args, stored rows) so everything
    /// downstream holds a validated value instead of re-checking text.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_uppercase().as_str() {
            "USD" => Ok(Currency::Usd),
            "EUR" => Ok(Currency::Eur),
            "GBP" => Ok(Currency::Gbp),
            "CHF" => Ok(Currency::Chf),
            "JPY" => Ok(Currency::Jpy),
            _ => Err(AssetError::UnknownCurrency(value.to_string())),
        }
    }
}

/// `Stock` and `Etf` are what v1 supports. `Adr`, `Bond`, `MutualFund`,
/// `Index`, and options come later — the enum is the place that decision gets
/// recorded, and adding a variant makes every `match` that needs updating fail
/// to compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetType {
    Stock,
    Etf,
}

impl AssetType {
    pub fn label(&self) -> &'static str {
        match self {
            AssetType::Stock => "Stock",
            AssetType::Etf => "ETF",
        }
    }
}

impl fmt::Display for AssetType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

impl FromStr for AssetType {
    type Err = AssetError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "stock" | "equity" => Ok(AssetType::Stock),
            "etf" => Ok(AssetType::Etf),
            _ => Err(AssetError::UnknownAssetType(value.to_string())),
        }
    }
}

/// Static descriptive information about a tradable instrument. The one true
/// leaf of the domain graph: referenced by `Transaction`, `Position`, and
/// `MarketData`, but referencing nothing itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    id: AssetId,
    ticker: Ticker,
    name: String,
    exchange: String,
    currency: Currency,
    asset_type: AssetType,
    sector: Option<String>,
    industry: Option<String>,
    country: Option<String>,
    //isin: Option<String>,
    //cusip: Option<String>,
    //description: Option<String>
}

impl Asset {
    /// The classification fields (`sector`/`industry`/`country`) are all
    /// optional metadata, so they're set through [`Asset::with_classification`]
    /// rather than padding this signature out to nine positional arguments
    /// where three adjacent `Option<String>`s would be trivially easy to
    /// transpose at a call site.
    pub fn new(
        ticker: Ticker,
        name: String,
        exchange: String,
        currency: Currency,
        asset_type: AssetType,
    ) -> Self {
        let id = AssetId::for_ticker(ticker.as_str(), &exchange);
        Asset {
            id,
            ticker,
            name,
            exchange,
            currency,
            asset_type,
            sector: None,
            industry: None,
            country: None,
        }
    }

    pub fn with_classification(
        mut self,
        sector: Option<String>,
        industry: Option<String>,
        country: Option<String>,
    ) -> Self {
        self.sector = sector;
        self.industry = industry;
        self.country = country;
        self
    }

    pub fn id(&self) -> AssetId {
        self.id
    }

    pub fn ticker(&self) -> &Ticker {
        &self.ticker
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn exchange(&self) -> &str {
        &self.exchange
    }

    pub fn currency(&self) -> Currency {
        self.currency
    }

    pub fn asset_type(&self) -> AssetType {
        self.asset_type
    }

    pub fn sector(&self) -> Option<&str> {
        self.sector.as_deref()
    }

    pub fn industry(&self) -> Option<&str> {
        self.industry.as_deref()
    }

    pub fn country(&self) -> Option<&str> {
        self.country.as_deref()
    }
}

// This is a trait required for printing ("println! macro"). Here we can define the format of the printed string
// https://doc.rust-lang.org/stable/std/fmt/index.html
// More on the docs above
impl fmt::Display for Asset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({}) — {}", self.ticker, self.exchange, self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn id_is_derived_from_ticker_and_exchange() {
        assert_eq!(apple().id(), AssetId::for_ticker("AAPL", "NASDAQ"));
    }

    #[test]
    fn new_leaves_classification_unset() {
        let asset = apple();
        assert_eq!(asset.sector(), None);
        assert_eq!(asset.industry(), None);
        assert_eq!(asset.country(), None);
    }

    #[test]
    fn with_classification_populates_optional_metadata() {
        let asset = apple().with_classification(
            Some("Technology".to_string()),
            Some("Consumer Electronics".to_string()),
            Some("US".to_string()),
        );

        assert_eq!(asset.sector(), Some("Technology"));
        assert_eq!(asset.industry(), Some("Consumer Electronics"));
        assert_eq!(asset.country(), Some("US"));
    }

    #[test]
    fn currency_parses_case_insensitively() {
        assert_eq!("usd".parse::<Currency>(), Ok(Currency::Usd));
        assert_eq!("EUR".parse::<Currency>(), Ok(Currency::Eur));
    }

    #[test]
    fn currency_rejects_unknown_codes() {
        assert_eq!(
            "XYZ".parse::<Currency>(),
            Err(AssetError::UnknownCurrency("XYZ".to_string()))
        );
    }

    #[test]
    fn asset_type_parses_known_labels() {
        assert_eq!("stock".parse::<AssetType>(), Ok(AssetType::Stock));
        assert_eq!("ETF".parse::<AssetType>(), Ok(AssetType::Etf));
        assert!("bond".parse::<AssetType>().is_err());
    }

    #[test]
    fn currency_round_trips_through_its_code() {
        for currency in [
            Currency::Usd,
            Currency::Eur,
            Currency::Gbp,
            Currency::Chf,
            Currency::Jpy,
        ] {
            assert_eq!(currency.code().parse::<Currency>(), Ok(currency));
        }
    }
}
