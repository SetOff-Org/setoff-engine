//! Nets one real day of mainnet payments between institutions and reports
//! what settling the net instead of every payment would have saved.
//!
//! python3 tools/mainnet_day.py 2026-10-21 > data/mainnet-2026-10-21.csv
//! cargo run --example mainnet_day --features csv -- data/mainnet-2026-10-21.csv

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    missing_docs
)]

use std::collections::BTreeSet;

use setoff_engine::{compare, net, participants, read_csv};

fn main() {
    let path = std::env::args().nth(1).expect("usage: mainnet_day <obligations.csv>");
    let obligations = read_csv(std::fs::File::open(&path).expect("open the CSV")).expect("read the CSV");
    let netting = net(&obligations).expect("net the obligations");
    let units = |v: i128| v as f64 / 1e7;

    let institutions: BTreeSet<&str> =
        obligations.iter().flat_map(|o| [o.debtor.as_str(), o.creditor.as_str()]).collect();
    println!("{path}");
    println!("{} payments between {} institutions\n", obligations.len(), institutions.len());

    for c in compare(&obligations).unwrap() {
        let payments = obligations.iter().filter(|o| o.asset == c.asset).count();
        let saved = |v: i128| 100.0 * (1.0 - v as f64 / c.gross as f64);
        println!("{}            moved {:>18}   transfers", c.asset, "saved");
        println!("  gross         {:>16.2}   {:>8}   {payments:>9}", units(c.gross), "");
        println!(
            "  bilateral     {:>16.2}   {:>7.1}%   {:>9}",
            units(c.bilateral),
            saved(c.bilateral),
            c.bilateral_transfers
        );
        println!(
            "  multilateral  {:>16.2}   {:>7.1}%   {:>9}",
            units(c.multilateral),
            saved(c.multilateral),
            c.multilateral_transfers
        );
    }
    debug_assert_eq!(
        netting.transfers.len(),
        compare(&obligations).unwrap().iter().map(|c| c.multilateral_transfers).sum::<usize>()
    );

    let mut debtors: Vec<_> = participants(&obligations).unwrap().into_iter().filter(|p| p.collateral > 0).collect();
    debtors.sort_by_key(|p| std::cmp::Reverse(p.collateral));
    println!("\nlargest net debtors (what each would actually have to pay)");
    for p in debtors.iter().take(10) {
        println!("  {:<28} {:>16.2} {}", p.participant, units(p.collateral), p.asset);
    }
}
