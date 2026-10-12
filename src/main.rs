use rust_decimal::Decimal;
use std::collections::HashMap;
use std::error::Error;
use std::process::ExitCode;
use tradetrack::{Holding, Row, Totals, Transaction, cash_balance, holdings, totals};

mod prices;
mod tui;
const USAGE: &str = "usage: tradetrack <export.csv> <tickers.csv>";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

/// One position together with its current price, if that could be looked up.
struct Position {
    isin: String,
    holding: Holding,
    price: Option<Decimal>,
}

impl Position {
    fn value(&self) -> Option<Decimal> {
        self.price.map(|price| self.holding.value(price))
    }

    fn unrealized(&self) -> Option<Decimal> {
        self.price.map(|price| self.holding.unrealized(price))
    }
}

/// The bottom line of a table: every column added up over its positions.
///
/// `value` and `unrealized` are `None` if any price is missing.
struct Summary {
    cost: Decimal,
    realized: Decimal,
    value: Option<Decimal>,
    unrealized: Option<Decimal>,
    dividends: Decimal,
}

fn summary(positions: &[&Position]) -> Summary {
    let holdings = || positions.iter().map(|position| &position.holding);
    let cost: Decimal = holdings().map(Holding::cost_basis).sum();
    let realized: Decimal = holdings().map(|holding| holding.realized).sum();
    let dividends: Decimal = holdings().map(|holding| holding.dividends).sum();
    let value: Option<Decimal> = positions.iter().map(|position| position.value()).sum();
    let unrealized = value.map(|value| value - cost);

    Summary {
        cost,
        realized,
        value,
        unrealized,
        dividends,
    }
}

/// What the whole account has earned after tax: gains, dividends and interest.
fn total_return(all: &Summary, sums: &Totals) -> Option<Decimal> {
    all.unrealized
        .map(|unrealized| all.realized + unrealized + sums.dividends + sums.interest + sums.taxes)
}

/// Looks up what one share of a position is worth in euros right now.
fn price(
    isin: &str,
    tickers: &HashMap<String, String>,
    eur_usd: Option<Decimal>,
) -> Result<Decimal, Box<dyn Error>> {
    let ticker = tickers
        .get(isin)
        .ok_or_else(|| format!("no ticker for {isin} in the tickers file"))?;
    let eur_usd = eur_usd.ok_or("no EUR/USD rate")?;
    prices::price_in_eur(ticker, eur_usd)
}

/// Formats an amount for the table, or a dash if it is not known.
fn cell(amount: Option<Decimal>) -> String {
    amount.map_or_else(
        || "-".to_string(),
        |amount| format!("{:.2}", amount.round_dp(2)),
    )
}

fn run() -> Result<(), Box<dyn Error>> {
    let path = std::env::args().nth(1).ok_or(USAGE)?;

    let ticker_path = std::env::args().nth(2).ok_or(USAGE)?;

    let mut reader = csv::Reader::from_path(path)?;
    let rows: Vec<Row> = reader.deserialize().collect::<Result<_, _>>()?;
    let transactions: Vec<Transaction> = rows
        .iter()
        .map(Transaction::try_from)
        .collect::<Result<_, _>>()?;

    let mut ticker_reader = csv::Reader::from_path(ticker_path)?;
    let tickers: HashMap<String, String> = ticker_reader.deserialize().collect::<Result<_, _>>()?;

    println!("cash balance: {:.2}", cash_balance(&transactions));
    let sums = totals(&transactions);
    println!(
        "fees: {:.2}  taxes: {:.2}  interest: {:.2} dividends(gross): {:.2}",
        sums.fees, sums.taxes, sums.interest, sums.dividends
    );

    let eur_usd = prices::fetch("EURUSD=X")
        .inspect_err(|error| eprintln!("warning: no EUR/USD rate: {error}"))
        .ok()
        .map(|quote| quote.price);
    if let Some(rate) = eur_usd {
        println!("conversion rate EUR/USD: {rate}");
    }

    let positions: Vec<Position> = holdings(&transactions)
        .into_iter()
        .map(|(isin, holding)| {
            let price = if holding.shares().is_zero() {
                Some(Decimal::ZERO)
            } else {
                price(&isin, &tickers, eur_usd)
                    .inspect_err(|error| eprintln!("warning: {}: {error}", holding.name))
                    .ok()
            };
            Position {
                isin,
                holding,
                price,
            }
        })
        .collect();

    let all: Vec<&Position> = positions.iter().collect();
    let summary = summary(&all);
    let total_return = total_return(&summary, &sums);

    if std::env::args().any(|arg| arg == "--tui") {
        tui::run(&positions, &rows, total_return)?;
        return Ok(());
    }

    println!(
        "{:<32} {:<12} {:>12} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10}",
        "name", "code", "shares", "cost", "avg", "realized", "value", "unrealized", "dividends"
    );

    for position in &positions {
        let isin = &position.isin;
        let holding = &position.holding;
        let value = position.value();
        let unrealized = position.unrealized();
        println!(
            "{:<32} {isin:<12} {:>12} {:>10.2} {:>10.2} {:>10.2} {:>10} {:>10} {:>10.2}",
            holding.name,
            holding.shares().normalize(),
            holding.cost_basis().round_dp(2),
            holding.avg_price().unwrap_or_default().round_dp(2),
            holding.realized.round_dp(2),
            cell(value),
            cell(unrealized),
            holding.dividends.round_dp(2),
        );
    }

    println!(
        "{:<32} {:<12} {:>12} {:>10.2} {:>10} {:>10.2} {:>10} {:>10} {:>10.2}",
        "TOTAL",
        "",
        "",
        summary.cost.round_dp(2),
        "",
        summary.realized.round_dp(2),
        cell(summary.value),
        cell(summary.unrealized),
        summary.dividends.round_dp(2),
    );

    println!("total return after tax: {}", cell(total_return));

    Ok(())
}
