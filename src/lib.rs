use rust_decimal::Decimal;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::str::FromStr;

#[derive(Debug)]
pub struct Holding {
    pub name: String,
    pub shares: Decimal,
}

#[derive(Debug, Deserialize)]
pub struct Row {
    pub datetime: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    pub symbol: String,
    pub shares: Option<Decimal>,
    pub price: Option<Decimal>,
    pub amount: Option<Decimal>,
    pub fee: Option<Decimal>,
    pub tax: Option<Decimal>,
    pub transaction_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransactionType {
    Buy,
    Sell,
    Dividend,
    Split,
    BenefitsSaveback,
    InterestPayment,
    TaxOptimization,
    CardTransaction,
    CardTransactionInternational,
    TransferInstantInbound,
    TransferInstantOutbound,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown transaction type: {0}")]
pub struct UnknownTransactionType(pub String);

impl FromStr for TransactionType {
    type Err = UnknownTransactionType;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "BUY" => Ok(Self::Buy),
            "SELL" => Ok(Self::Sell),
            "DIVIDEND" => Ok(Self::Dividend),
            "SPLIT" => Ok(Self::Split),
            "BENEFITS_SAVEBACK" => Ok(Self::BenefitsSaveback),
            "INTEREST_PAYMENT" => Ok(Self::InterestPayment),
            "TAX_OPTIMIZATION" => Ok(Self::TaxOptimization),
            "CARD_TRANSACTION" => Ok(Self::CardTransaction),
            "CARD_TRANSACTION_INTERNATIONAL" => Ok(Self::CardTransactionInternational),
            "TRANSFER_INSTANT_INBOUND" => Ok(Self::TransferInstantInbound),
            "TRANSFER_INSTANT_OUTBOUND" => Ok(Self::TransferInstantOutbound),
            other => Err(UnknownTransactionType(other.to_string())),
        }
    }
}

/// Adds up the shares of every position, keyed by ISIN.
///
/// # Errors
///
/// Returns an error if a row has a transaction type this program does not know.
pub fn holdings(rows: &[Row]) -> Result<BTreeMap<String, Holding>, UnknownTransactionType> {
    let mut positions: BTreeMap<String, Holding> = BTreeMap::new();

    for row in rows {
        let kind: TransactionType = row.kind.parse()?;
        if !matches!(
            kind,
            TransactionType::Buy | TransactionType::Sell | TransactionType::Split
        ) {
            continue;
        }

        let holding = positions
            .entry(row.symbol.clone())
            .or_insert_with(|| Holding {
                name: row.name.clone(),
                shares: Decimal::ZERO,
            });
        holding.shares += row.shares.unwrap_or_default();
    }

    positions.retain(|_, holding| !holding.shares.is_zero());
    Ok(positions)
}

/// Adds up amount, fee and tax of every row: the money left in the account.
#[must_use]
pub fn cash_balance(rows: &[Row]) -> Decimal {
    let mut cash = Decimal::ZERO;
    for row in rows {
        cash += row.amount.unwrap_or_default()
            + row.fee.unwrap_or_default()
            + row.tax.unwrap_or_default();
    }
    cash
}
