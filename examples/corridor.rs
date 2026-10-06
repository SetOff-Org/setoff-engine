//! Nets a remittance corridor and compares settlement styles.
//!
//! cargo run --example corridor

#![allow(clippy::unwrap_used, clippy::arithmetic_side_effects, missing_docs)]

use setoff_engine::{Obligation, compare, net, participants};

fn main() {
    let ob =
        |id: &str, d: &str, c: &str, asset: &str, units: i128| Obligation::new(id, d, c, asset, units * 10_000_000);
    let window = [
        ob("1", "anchor-ng", "anchor-us", "USDC", 500),
        ob("2", "anchor-us", "anchor-ng", "USDC", 320),
        ob("3", "anchor-ng", "anchor-ke", "USDC", 110),
        ob("4", "anchor-ke", "anchor-us", "USDC", 90),
        ob("5", "anchor-us", "anchor-ke", "USDC", 125),
        ob("6", "anchor-eu", "anchor-ng", "EURC", 70),
        ob("7", "anchor-ng", "anchor-eu", "EURC", 65),
    ];
    let units = |v: i128| v as f64 / 1e7;

    println!("{:<6} {:>10} {:>10} {:>13} {:>8}", "asset", "gross", "bilateral", "multilateral", "saved");
    for c in compare(&window).unwrap() {
        let saved = 100.0 * (1.0 - c.multilateral as f64 / c.gross as f64);
        println!(
            "{:<6} {:>10.2} {:>10.2} {:>13.2} {:>7.1}%",
            c.asset,
            units(c.gross),
            units(c.bilateral),
            units(c.multilateral),
            saved
        );
    }

    println!("\ncollateral each anchor must post");
    for p in participants(&window).unwrap().iter().filter(|p| p.collateral > 0) {
        println!("  {:<10} {:>8.2} {}", p.participant, units(p.collateral), p.asset);
    }

    println!("\nsettlement plan");
    for t in net(&window).unwrap().transfers {
        println!("  {} -> {}  {:.2} {}", t.from, t.to, units(t.amount), t.asset);
    }
}
