use std::collections::HashMap;
use std::error::Error;

use rust_decimal::Decimal;
use tradetrack::{Row, TransactionType};

fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: tradeTrack /private/Transaktionsexport.csv")?;

    let mut reader = csv::Reader::from_path(path)?;
    let mut counts: HashMap<TransactionType, u32> = HashMap::new();
    let mut cash = Decimal::ZERO;

    for result in reader.deserialize() {
        let row: Row = result?;
        let kind: TransactionType = row.kind.parse()?;
        *counts.entry(kind).or_insert(0) += 1;

        cash += row.amount.unwrap_or_default()
            + row.fee.unwrap_or_default()
            + row.tax.unwrap_or_default();
    }

    for (kind, count) in &counts {
        println!("{kind:?}: {count}");
    }
    println!("cash balance: {cash}");

    Ok(())
}
