use std::collections::HashMap;
use std::error::Error;

use tradetrack::TransactionType;
fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: tradeTrack /private/Transaktionsexport.csv")?;

    let mut reader = csv::Reader::from_path(path)?;
    let mut counts: HashMap<TransactionType, u32> = HashMap::new();

    for result in reader.records() {
        let record = result?;
        let text = record.get(4).ok_or("row has no type column")?;
        let kind: TransactionType = text.parse()?;
        *counts.entry(kind).or_insert(0) += 1;
    }

    for (kind, count) in &counts {
        println!("{kind:?}: {count}");
    }

    Ok(())
}
