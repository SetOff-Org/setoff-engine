//! CSV in and out (`csv` feature).
//!
//! Obligations are read with the header `id,debtor,creditor,asset,amount`, the
//! format `setoff net` reads. The settlement plan is written as
//! `asset,from,to,amount`, ready for a payments system to import. Amounts are
//! canonical integers in the asset's smallest unit, as in JSON.

use std::io::{Read, Write};

use crate::{Netting, Obligation, parse_amount};

/// A CSV row that could not be read.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("line {line}: {reason}")]
pub struct CsvError {
    /// 1-based line number (the header is line 1).
    pub line: u64,
    /// What was wrong.
    pub reason: String,
}

const HEADER: [&str; 5] = ["id", "debtor", "creditor", "asset", "amount"];

/// Reads every obligation from a CSV document.
pub fn read_csv(reader: impl Read) -> Result<Vec<Obligation>, CsvError> {
    let mut r = csv::ReaderBuilder::new().has_headers(true).from_reader(reader);
    let err = |line: u64, reason: String| CsvError { line, reason };
    let header = r.headers().map_err(|e| err(1, e.to_string()))?;
    if header.iter().collect::<Vec<_>>() != HEADER {
        return Err(err(1, format!("header must be {}", HEADER.join(","))));
    }
    let mut out = Vec::new();
    for record in r.records() {
        let record = record.map_err(|e| err(e.position().map_or(0, |p| p.line()), e.to_string()))?;
        let line = record.position().map_or(0, |p| p.line());
        let field = |i: usize| record.get(i).unwrap_or_default().to_owned();
        let amount =
            parse_amount(&field(4)).ok_or_else(|| err(line, format!("not a canonical integer: {:?}", field(4))))?;
        out.push(Obligation { id: field(0), debtor: field(1), creditor: field(2), asset: field(3), amount });
    }
    Ok(out)
}

/// Header of the plan CSV.
pub const PLAN_HEADER: [&str; 4] = ["asset", "from", "to", "amount"];

/// Writes the settlement plan, one transfer per row, in plan order.
pub fn write_plan_csv(writer: impl Write, netting: &Netting) -> Result<(), csv::Error> {
    let mut w = csv::Writer::from_writer(writer);
    w.write_record(PLAN_HEADER)?;
    for t in &netting.transfers {
        w.write_record([t.asset.as_str(), t.from.as_str(), t.to.as_str(), &t.amount.to_string()])?;
    }
    w.flush()?;
    Ok(())
}
