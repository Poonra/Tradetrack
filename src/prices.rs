use std::error::Error;

use rust_decimal::Decimal;
use serde::Deserialize;

#[derive(Deserialize)]
struct Response {
    chart: Chart,
}

#[derive(Deserialize)]
struct Chart {
    result: Vec<ChartResult>,
}

#[derive(Deserialize)]
struct ChartResult {
    meta: Meta,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Meta {
    regular_market_price: f64,
    currency: String,
}

pub struct Quote {
    pub price: Decimal,
    pub currency: String,
}

/// Asks Yahoo Finance for the latest price of one ticker.
pub fn fetch(ticker: &str) -> Result<Quote, Box<dyn Error>> {
    let url = format!("https://query1.finance.yahoo.com/v8/finance/chart/{ticker}");
    let response: Response = ureq::get(&url)
        .header("User-Agent", "Mozilla/5.0")
        .call()?
        .body_mut()
        .read_json()?;

    let meta = response
        .chart
        .result
        .into_iter()
        .next()
        .ok_or_else(|| format!("no price data for {ticker}"))?
        .meta;

    Ok(Quote {
        price: Decimal::try_from(meta.regular_market_price)?,
        currency: meta.currency,
    })
}

/// Fetches the latest price of one ticker and converts it to euros if needed.
pub fn price_in_eur(ticker: &str, eur_usd: Decimal) -> Result<Decimal, Box<dyn Error>> {
    let quote = fetch(ticker)?;
    match quote.currency.as_str() {
        "EUR" => Ok(quote.price),
        "USD" => Ok(quote.price / eur_usd),
        other => Err(format!("{ticker} is priced in {other}, which is not supported").into()),
    }
}
