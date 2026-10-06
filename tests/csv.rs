//! CSV in and out (`--features csv`).

#![cfg(feature = "csv")]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, missing_docs)]

use setoff_engine::{net, read_csv, write_plan_csv};

#[test]
fn reads_the_setoff_net_format() {
    let csv = "id,debtor,creditor,asset,amount\n1,A,B,USDC,100\n2,B,A,USDC,60\n";
    let obs = read_csv(csv.as_bytes()).unwrap();
    assert_eq!(obs.len(), 2);
    assert_eq!(net(&obs).unwrap().assets[0].settled, 40);
}

#[test]
fn errors_name_the_line() {
    let bad_header = read_csv("debtor,creditor\nA,B\n".as_bytes()).unwrap_err();
    assert_eq!(bad_header.line, 1);
    let bad_amount =
        read_csv("id,debtor,creditor,asset,amount\n1,A,B,USDC,100\n2,B,A,USDC,1.5\n".as_bytes()).unwrap_err();
    assert_eq!(bad_amount.line, 3);
    assert!(bad_amount.reason.contains("1.5"));
}

#[test]
fn writes_the_plan_for_payments_systems() {
    let csv = "id,debtor,creditor,asset,amount
1,A,B,USDC,100
2,B,C,USDC,70
3,C,A,EURC,5
";
    let n = net(&read_csv(csv.as_bytes()).unwrap()).unwrap();
    let mut out = Vec::new();
    write_plan_csv(&mut out, &n).unwrap();
    let text = String::from_utf8(out).unwrap();
    let mut lines = text.lines();
    assert_eq!(lines.next(), Some("asset,from,to,amount"));
    let rows: Vec<&str> = lines.collect();
    assert_eq!(rows.len(), n.transfers.len());
    for (row, t) in rows.iter().zip(&n.transfers) {
        assert_eq!(*row, format!("{},{},{},{}", t.asset, t.from, t.to, t.amount));
    }
}

#[test]
fn awkward_names_are_quoted() {
    let n = net(&[setoff_engine::Obligation::new("1", "Acme, Inc.", "B \"Co\"", "USDC", 9)]).unwrap();
    let mut out = Vec::new();
    write_plan_csv(&mut out, &n).unwrap();
    let mut r = csv::Reader::from_reader(out.as_slice());
    let row = r.records().next().unwrap().unwrap();
    assert_eq!((&row[1], &row[2], &row[3]), ("Acme, Inc.", "B \"Co\"", "9"));
}
