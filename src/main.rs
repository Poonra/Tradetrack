use std::error::Error;
use tradetrack::{Row, cash_balance, holdings};

fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: tradeTrack /private/Transaktionsexport.csv")?;

    let mut reader = csv::Reader::from_path(path)?;
    let rows: Vec<Row> = reader.deserialize().collect::<Result<_, _>>()?;

    println!("cash balance: {}", cash_balance(&rows));

    for (isin, holding) in &holdings(&rows)? {
        println!(
            "{:<32} {isin:<12} {:>12} {:>10.2} {:>10.2}",
            holding.name,
            holding.shares().normalize(),
            holding.cost_basis().round_dp(2),
            holding.realized.round_dp(2)
        );
    }
    Ok(())
}
