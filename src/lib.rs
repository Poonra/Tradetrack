use rust_decimal::Decimal;
use serde::Deserialize;
use std::collections::{BTreeMap, VecDeque};

#[derive(Debug)]
pub struct Lot {
    pub shares: Decimal,
    pub cost: Decimal,
}

#[derive(Debug, Default)]
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

/// One line of the export, exactly as the CSV has it.
#[derive(Debug, Deserialize)]
pub struct Row {
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    pub symbol: Option<String>,
    pub shares: Option<Decimal>,
    pub amount: Option<Decimal>,
    pub fee: Option<Decimal>,
    pub tax: Option<Decimal>,
}

#[derive(Debug)]
pub enum Transaction {
    Buy {
        isin: String,
        name: String,
        shares: Decimal,
        amount: Decimal,
        fee: Decimal,
    },
    Sell {
        isin: String,
        name: String,
        shares: Decimal,
        amount: Decimal,
        fee: Decimal,
        tax: Decimal,
    },
    Split {
        isin: String,
        name: String,
        shares: Decimal,
    },
    Dividend {
        isin: String,
        name: String,
        amount: Decimal,
        tax: Decimal,
    },
    Interest {
        amount: Decimal,
        tax: Decimal,
    },
    /// Transfers, card payments, saveback and tax corrections: cash only.
    Other {
        amount: Decimal,
        tax: Decimal,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    #[error("unknown transaction type: {0}")]
    UnknownType(String),
    #[error("{kind} row has no {field}")]
    MissingField { kind: String, field: &'static str },
}

/// Takes the value out of a cell that this kind of row must have.
fn required<T>(value: Option<T>, kind: &str, field: &'static str) -> Result<T, ParseError> {
    value.ok_or_else(|| ParseError::MissingField {
        kind: kind.to_string(),
        field,
    })
}

impl TryFrom<Row> for Transaction {
    type Error = ParseError;

    fn try_from(row: Row) -> Result<Self, Self::Error> {
        let kind = row.kind.as_str();
        let fee = row.fee.unwrap_or_default();
        let tax = row.tax.unwrap_or_default();

        match kind {
            "BUY" => Ok(Self::Buy {
                isin: required(row.symbol, kind, "symbol")?,
                name: row.name,
                shares: required(row.shares, kind, "shares")?,
                amount: required(row.amount, kind, "amount")?,
                fee,
            }),
            "SELL" => Ok(Self::Sell {
                isin: required(row.symbol, kind, "symbol")?,
                name: row.name,
                shares: -required(row.shares, kind, "shares")?,
                amount: required(row.amount, kind, "amount")?,
                fee,
                tax,
            }),
            "SPLIT" => Ok(Self::Split {
                isin: required(row.symbol, kind, "symbol")?,
                name: row.name,
                shares: required(row.shares, kind, "shares")?,
            }),
            "DIVIDEND" => Ok(Self::Dividend {
                isin: required(row.symbol, kind, "symbol")?,
                name: row.name,
                amount: required(row.amount, kind, "amount")?,
                tax,
            }),
            "INTEREST_PAYMENT" => Ok(Self::Interest {
                amount: required(row.amount, kind, "amount")?,
                tax,
            }),
            "BENEFITS_SAVEBACK"
            | "TAX_OPTIMIZATION"
            | "CARD_TRANSACTION"
            | "CARD_TRANSACTION_INTERNATIONAL"
            | "TRANSFER_INSTANT_INBOUND"
            | "TRANSFER_INSTANT_OUTBOUND" => Ok(Self::Other {
                amount: row.amount.unwrap_or_default(),
                tax,
            }),
            other => Err(ParseError::UnknownType(other.to_string())),
        }
    }
}

impl Transaction {
    /// Money this transaction moved into (positive) or out of (negative) the account.
    fn cash(&self) -> Decimal {
        match self {
            Self::Buy { amount, fee, .. } => amount + fee,
            Self::Sell {
                amount, fee, tax, ..
            } => amount + fee + tax,
            Self::Dividend { amount, tax, .. }
            | Self::Interest { amount, tax }
            | Self::Other { amount, tax } => amount + tax,
            Self::Split { .. } => Decimal::ZERO,
        }
    }
}

/// Finds the holding for an ISIN, creating an empty one the first time it is seen.
fn position<'a>(
    positions: &'a mut BTreeMap<String, Holding>,
    isin: &str,
    name: &str,
) -> &'a mut Holding {
    positions
        .entry(isin.to_string())
        .or_insert_with(|| Holding {
            name: name.to_string(),
            ..Holding::default()
        })
}

/// Builds every position from the transactions, keyed by ISIN.
#[must_use]
pub fn holdings(transactions: &[Transaction]) -> BTreeMap<String, Holding> {
    let mut positions = BTreeMap::new();

    for transaction in transactions {
        match transaction {
            Transaction::Buy {
                isin,
                name,
                shares,
                amount,
                fee,
            } => position(&mut positions, isin, name).lots.push_back(Lot {
                shares: *shares,
                cost: -(amount + fee),
            }),
            Transaction::Sell {
                isin,
                name,
                shares,
                amount,
                fee,
                ..
            } => {
                let holding = position(&mut positions, isin, name);
                let cost = holding.sell(*shares);
                holding.realized += amount + fee - cost;
            }
            Transaction::Split { isin, name, shares } => {
                position(&mut positions, isin, name).split(*shares);
            }
            Transaction::Dividend {
                isin,
                name,
                amount,
                tax,
            } => position(&mut positions, isin, name).dividends += amount + tax,
            Transaction::Interest { .. } | Transaction::Other { .. } => {}
        }
    }

    positions
}

/// Account-wide sums over every transaction.
#[derive(Debug, Default)]
pub struct Totals {
    pub fees: Decimal,
    pub taxes: Decimal,
    pub interest: Decimal,
    pub dividends: Decimal,
}

/// Adds up the fees, taxes, interest and gross dividends of every transaction.
#[must_use]
pub fn totals(transactions: &[Transaction]) -> Totals {
    let mut totals = Totals::default();
    for transaction in transactions {
        match transaction {
            Transaction::Buy { fee, .. } => totals.fees += fee,
            Transaction::Sell { fee, tax, .. } => {
                totals.fees += fee;
                totals.taxes += tax;
            }
            Transaction::Dividend { amount, tax, .. } => {
                totals.dividends += amount;
                totals.taxes += tax;
            }
            Transaction::Interest { amount, tax } => {
                totals.interest += amount;
                totals.taxes += tax;
            }
            Transaction::Other { tax, .. } => totals.taxes += tax,
            Transaction::Split { .. } => {}
        }
    }
    totals
}

/// Adds up the cash effect of every transaction: the money left in the account.
#[must_use]
pub fn cash_balance(transactions: &[Transaction]) -> Decimal {
    transactions.iter().map(Transaction::cash).sum()
}
