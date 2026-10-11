use rust_decimal::Decimal;
use serde::Deserialize;
use std::str::FromStr;

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
