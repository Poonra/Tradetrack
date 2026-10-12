use rust_decimal::Decimal;
use serde::Deserialize;
use std::collections::{BTreeMap, VecDeque};
use std::str::FromStr;

#[derive(Debug)]
pub struct Lot {
    pub shares: Decimal,
    pub cost: Decimal,
}

#[derive(Debug)]
pub struct Holding {
    pub name: String,
    pub lots: VecDeque<Lot>,
    pub realized: Decimal,
    pub dividends: Decimal,
}

impl Holding {
    #[must_use]
    pub fn shares(&self) -> Decimal {
        self.lots.iter().map(|lot| lot.shares).sum()
    }

    #[must_use]
    pub fn cost_basis(&self) -> Decimal {
        self.lots.iter().map(|lot| lot.cost).sum()
    }

    /// What one held share cost on average, or `None` if no shares are held.
    #[must_use]
    pub fn avg_price(&self) -> Option<Decimal> {
        self.cost_basis().checked_div(self.shares())
    }

    /// Removes sold shares from the oldest lots first and returns what they cost.
    fn sell(&mut self, mut shares: Decimal) -> Decimal {
        let mut cost = Decimal::ZERO;
        while let Some(lot) = self.lots.front_mut() {
            if lot.shares <= shares {
                shares -= lot.shares;
                cost += lot.cost;
                self.lots.pop_front();
            } else {
                let part = lot.cost * shares / lot.shares;
                lot.shares -= shares;
                lot.cost -= part;
                cost += part;
                break;
            }
        }
        cost
    }

    /// Spreads the extra shares of a split over the existing lots.
    fn split(&mut self, extra: Decimal) {
        let before = self.shares();
        if before.is_zero() {
            return;
        }
        let factor = (before + extra) / before;
        for lot in &mut self.lots {
            lot.shares *= factor;
        }
    }
    #[must_use]
    pub fn value(&self, price: Decimal) -> Decimal {
        self.shares() * price
    }

    /// Gain or loss on the shares held: their value minus what they cost.
    #[must_use]
    pub fn unrealized(&self, price: Decimal) -> Decimal {
        self.value(price) - self.cost_basis()
    }
}

#[derive(Debug, Deserialize)]
pub struct Row {
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    pub symbol: String,
    pub shares: Option<Decimal>,
    pub amount: Option<Decimal>,
    pub fee: Option<Decimal>,
    pub tax: Option<Decimal>,
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
            TransactionType::Buy
                | TransactionType::Sell
                | TransactionType::Split
                | TransactionType::Dividend
        ) {
            continue;
        }

        let shares = row.shares.unwrap_or_default();
        let money = row.amount.unwrap_or_default() + row.fee.unwrap_or_default();

        let holding = positions
            .entry(row.symbol.clone())
            .or_insert_with(|| Holding {
                name: row.name.clone(),
                lots: VecDeque::new(),
                realized: Decimal::ZERO,
                dividends: Decimal::ZERO,
            });
        match kind {
            TransactionType::Buy => holding.lots.push_back(Lot {
                shares,
                cost: -money,
            }),
            TransactionType::Sell => {
                let cost = holding.sell(-shares);
                holding.realized += money - cost;
            }
            TransactionType::Split => holding.split(shares),

            TransactionType::Dividend => {
                holding.dividends += row.amount.unwrap_or_default() + row.tax.unwrap_or_default();
            }
            _ => {}
        }
    }

    Ok(positions)
}

/// Account-wide sums over every row of the export.
#[derive(Debug, Default)]
pub struct Totals {
    pub fees: Decimal,
    pub taxes: Decimal,
    pub interest: Decimal,
}

/// Adds up the fees, taxes and interest of every row.
///
/// # Errors
///
/// Returns an error if a row has a transaction type this program does not know.
pub fn totals(rows: &[Row]) -> Result<Totals, UnknownTransactionType> {
    let mut totals = Totals::default();
    for row in rows {
        totals.fees += row.fee.unwrap_or_default();
        totals.taxes += row.tax.unwrap_or_default();
        if row.kind.parse::<TransactionType>()? == TransactionType::InterestPayment {
            totals.interest += row.amount.unwrap_or_default();
        }
    }
    Ok(totals)
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
