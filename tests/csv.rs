//! CSV input (`--features csv`).

#![cfg(feature = "csv")]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, missing_docs)]

use setoff_engine::{net, read_csv};

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
