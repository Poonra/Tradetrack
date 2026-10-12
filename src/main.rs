use rust_decimal::Decimal;
use std::collections::HashMap;
use std::error::Error;
use tradetrack::{Row, cash_balance, holdings};

mod prices;
const USAGE: &str = "usage: tradetrack <export.csv> <tickers.csv>";

fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args().nth(1).ok_or(USAGE)?;

    let ticker_path = std::env::args().nth(2).ok_or(USAGE)?;

    let mut reader = csv::Reader::from_path(path)?;
    let rows: Vec<Row> = reader.deserialize().collect::<Result<_, _>>()?;

    let mut ticker_reader = csv::Reader::from_path(ticker_path)?;
    let tickers: HashMap<String, String> = ticker_reader.deserialize().collect::<Result<_, _>>()?;

    println!("cash balance: {}", cash_balance(&rows));
    let eur_usd = prices::fetch("EURUSD=X")?;
    println!(
        "conversion rate EUR/USD: {} {}",
        eur_usd.price, eur_usd.currency
    );

    println!(
        "{:<32} {:<12} {:>12} {:>10} {:>10} {:>10} {:>10} {:>10}",
        "name", "code", "shares", "cost", "realized", "value", "unrealized", "dividends"
    );

    let mut total_cost = Decimal::ZERO;
    let mut total_realized = Decimal::ZERO;
    let mut total_value = Decimal::ZERO;
    let mut total_dividends = Decimal::ZERO;

    for (isin, holding) in &holdings(&rows)? {
        let price = if holding.shares().is_zero() {
            Decimal::ZERO
        } else {
            let ticker = tickers
                .get(isin)
                .ok_or_else(|| format!("no ticker for {isin} in the tickers file"))?;
            prices::price_in_eur(ticker, eur_usd.price)?
        };
        println!(
            "{:<32} {isin:<12} {:>12} {:>10.2} {:>10.2} {:>10.2} {:>10.2} {:>10.2}",
            holding.name,
            holding.shares().normalize(),
            holding.cost_basis().round_dp(2),
            holding.realized.round_dp(2),
            holding.value(price).round_dp(2),
            holding.unrealized(price).round_dp(2),
            holding.dividends.round_dp(2)
        );
        total_cost += holding.cost_basis();
        total_realized += holding.realized;
        total_value += holding.value(price);
        total_dividends += holding.dividends;
    }

    println!(
        "{:<32} {:<12} {:>12} {:>10.2} {:>10.2} {:>10.2} {:>10.2} {:>10.2}",
        "TOTAL",
        "",
        "",
        total_cost.round_dp(2),
        total_realized.round_dp(2),
        total_value.round_dp(2),
        (total_value - total_cost).round_dp(2),
        total_dividends.round_dp(2),
    );

    Ok(())
}
