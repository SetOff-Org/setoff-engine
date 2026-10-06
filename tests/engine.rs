//! Reference vectors (shared with setoff-clearing) and properties of netting.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::arithmetic_side_effects, missing_docs)]

use std::collections::BTreeMap;

use proptest::prelude::*;
use serde::{Deserialize, Serialize};
use setoff_engine::{
    Error, Netter, Netting, Obligation, Strategy as Plan, compare, net, net_with, participants, validate,
};

const VECTORS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/vectors");

#[derive(Serialize, Deserialize)]
struct Vector {
    name: String,
    description: String,
    obligations: Vec<Obligation>,
    netting: Netting,
}

fn ob(id: &str, debtor: &str, creditor: &str, asset: &str, amount: i128) -> Obligation {
    Obligation::new(id, debtor, creditor, asset, amount)
}

/// Deterministic pseudo-random window: same seed, same obligations, everywhere.
fn generated(seed: u64, n: usize, participants: usize) -> Vec<Obligation> {
    let mut x = seed;
    let mut next = || {
        x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        x >> 33
    };
    (0..n)
        .map(|i| {
            let d = next() as usize % participants;
            let c = (d + 1 + next() as usize % (participants - 1)) % participants;
            let asset = if next() % 4 == 0 { "EURC" } else { "USDC" };
            ob(
                &format!("ob-{i:03}"),
                &format!("P{d:02}"),
                &format!("P{c:02}"),
                asset,
                i128::from(1 + next() % 1_000_000) * 100,
            )
        })
        .collect()
}

fn scenarios() -> Vec<(&'static str, &'static str, Vec<Obligation>)> {
    vec![
        (
            "cycle",
            "Three participants owe each other the same amount in a ring: everything cancels.",
            vec![ob("1", "A", "B", "USDC", 100), ob("2", "B", "C", "USDC", 100), ob("3", "C", "A", "USDC", 100)],
        ),
        (
            "bilateral",
            "Two participants owe each other; only the difference moves.",
            vec![ob("1", "A", "B", "USDC", 100), ob("2", "B", "A", "USDC", 60)],
        ),
        (
            "remittance-corridor",
            "Anchors in a corridor with unequal flows in two assets.",
            vec![
                ob("1", "anchor-ng", "anchor-us", "USDC", 5_000_000_000),
                ob("2", "anchor-us", "anchor-ng", "USDC", 3_200_000_000),
                ob("3", "anchor-ng", "anchor-ke", "USDC", 1_100_000_000),
                ob("4", "anchor-ke", "anchor-us", "USDC", 900_000_000),
                ob("5", "anchor-us", "anchor-ke", "USDC", 1_250_000_000),
                ob("6", "anchor-eu", "anchor-ng", "EURC", 700_000_000),
                ob("7", "anchor-ng", "anchor-eu", "EURC", 650_000_000),
            ],
        ),
        (
            "fan-in",
            "Many payers, one payee: netting cannot help, and says so.",
            (0..5).map(|i| ob(&format!("{i}"), &format!("payer-{i}"), "merchant", "USDC", 10 * (i + 1))).collect(),
        ),
        ("generated-200", "200 pseudo-random obligations among 12 participants in two assets.", generated(42, 200, 12)),
    ]
}

#[test]
fn reference_vectors_are_current() {
    let update = std::env::var_os("UPDATE_FIXTURES").is_some();
    for (name, description, obligations) in scenarios() {
        let path = format!("{VECTORS}/{name}.json");
        let v = Vector {
            name: name.into(),
            description: description.into(),
            netting: net(&obligations).unwrap(),
            obligations,
        };
        let json = serde_json::to_string_pretty(&v).unwrap() + "\n";
        if update {
            std::fs::write(&path, &json).unwrap();
        }
        let committed = std::fs::read_to_string(&path).unwrap().replace("\r\n", "\n");
        assert_eq!(committed, json, "{name}: run UPDATE_FIXTURES=1 cargo test to refresh");
    }
}

#[test]
fn cycle_needs_no_transfers() {
    let n = net(&scenarios()[0].2).unwrap();
    assert!(n.positions.is_empty() && n.transfers.is_empty());
    assert_eq!(n.assets[0].saving_bps(), 10_000);
}

#[test]
fn bilateral_moves_only_the_difference() {
    let n = net(&scenarios()[1].2).unwrap();
    assert_eq!(n.transfers.len(), 1);
    assert_eq!((n.transfers[0].from.as_str(), n.transfers[0].to.as_str(), n.transfers[0].amount), ("A", "B", 40));
    assert_eq!(n.assets[0].saving_bps(), 7_500);
}

#[test]
fn fan_in_saves_nothing() {
    let n = net(&scenarios()[3].2).unwrap();
    assert_eq!(n.assets[0].saving_bps(), 0);
    assert_eq!(n.transfers.len(), 5);
}

#[test]
fn invalid_windows_are_rejected() {
    assert_eq!(net(&[ob("1", "A", "B", "X", 0)]), Err(Error::NonPositiveAmount("1".into())));
    assert_eq!(net(&[ob("1", "A", "A", "X", 5)]), Err(Error::SelfObligation("1".into())));
    assert_eq!(net(&[ob("1", "A", "B", "X", 5), ob("1", "B", "C", "X", 5)]), Err(Error::DuplicateId("1".into())));
    assert_eq!(net(&[ob("1", "", "B", "X", 5)]), Err(Error::Empty { id: "1".into(), field: "debtor" }));
    assert_eq!(net(&[ob("1", "A", "B", "X", i128::MAX), ob("2", "C", "B", "X", 1)]), Err(Error::Overflow));
}

#[test]
fn amounts_must_be_canonical_strings() {
    let parse = |amount: &str| {
        serde_json::from_str::<Obligation>(&format!(
            r#"{{"id":"1","debtor":"A","creditor":"B","asset":"X","amount":"{amount}"}}"#
        ))
    };
    assert_eq!(parse("170141183460469231731687303715884105727").unwrap().amount, i128::MAX);
    for bad in ["", "+5", "05", "-0", "1.5", "1e3"] {
        assert!(parse(bad).is_err(), "{bad:?}");
    }
}

fn window() -> impl Strategy<Value = Vec<Obligation>> {
    let participant = prop::sample::select(vec!["A", "B", "C", "D", "E", "F"]);
    let asset = prop::sample::select(vec!["USDC", "EURC"]);
    prop::collection::vec((participant.clone(), participant, asset, 1i128..1_000_000_000), 0..60).prop_map(|v| {
        v.into_iter()
            .enumerate()
            .filter(|(_, (d, c, _, _))| d != c)
            .map(|(i, (d, c, a, amt))| ob(&i.to_string(), d, c, a, amt))
            .collect()
    })
}

proptest! {
    #[test]
    fn positions_sum_to_zero_and_the_plan_settles_them_exactly(obs in window()) {
        let n = net(&obs).unwrap();
        let mut by_asset: BTreeMap<&str, i128> = BTreeMap::new();
        let mut settled: BTreeMap<(&str, &str), i128> = BTreeMap::new();
        for p in &n.positions {
            *by_asset.entry(&p.asset).or_default() += p.net;
        }
        prop_assert!(by_asset.values().all(|s| *s == 0));
        for t in &n.transfers {
            prop_assert!(t.amount > 0 && t.from != t.to);
            *settled.entry((&t.asset, &t.from)).or_default() -= t.amount;
            *settled.entry((&t.asset, &t.to)).or_default() += t.amount;
        }
        for p in &n.positions {
            prop_assert_eq!(settled.get(&(p.asset.as_str(), p.participant.as_str())).copied().unwrap_or(0), p.net);
        }
        prop_assert_eq!(settled.values().filter(|v| **v != 0).count(), n.positions.len());
    }

    #[test]
    fn plans_use_fewer_transfers_than_participants(obs in window()) {
        let n = net(&obs).unwrap();
        for a in &n.assets {
            let k = n.positions.iter().filter(|p| p.asset == a.asset).count();
            prop_assert!(a.transfers <= k.saturating_sub(1));
            prop_assert!(a.settled <= a.gross);
        }
    }

    #[test]
    fn order_of_obligations_never_matters(obs in window(), seed in any::<u64>()) {
        let mut shuffled = obs.clone();
        let len = shuffled.len();
        let mut x = seed;
        for i in (1..len).rev() {
            x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            shuffled.swap(i, (x >> 33) as usize % (i + 1));
        }
        prop_assert_eq!(net(&obs).unwrap(), net(&shuffled).unwrap());
    }
}

#[test]
fn bilateral_netting_sits_between_gross_and_multilateral() {
    let corridor = &scenarios()[2].2;
    let usdc = compare(corridor).unwrap().into_iter().find(|c| c.asset == "USDC").unwrap();
    assert_eq!(usdc.gross, 11_450_000_000);
    // ng<->us offset 5000 vs 3200, ke<->us 900 vs 1250, ng->ke 1100 alone.
    assert_eq!(usdc.bilateral, 1_800_000_000 + 350_000_000 + 1_100_000_000);
    assert_eq!(usdc.multilateral, 2_900_000_000);
    assert!(usdc.multilateral <= usdc.bilateral);
    assert_eq!(usdc.bilateral_transfers, 3);
}

#[test]
fn a_cycle_is_invisible_to_bilateral_netting() {
    let cycle = compare(&scenarios()[0].2).unwrap();
    assert_eq!((cycle[0].gross, cycle[0].bilateral, cycle[0].multilateral), (300, 300, 0));
}

#[test]
fn participant_summaries_add_up() {
    let s = participants(&scenarios()[1].2).unwrap();
    assert_eq!(s.len(), 2);
    let a = &s[0];
    assert_eq!((a.participant.as_str(), a.owes, a.owed, a.net, a.collateral), ("A", 100, 60, -40, 40));
    assert_eq!(s[1].collateral, 0);
}

proptest! {
    #[test]
    fn netting_never_moves_more_than_bilateral_or_gross(obs in window()) {
        for c in compare(&obs).unwrap() {
            prop_assert!(c.multilateral <= c.bilateral && c.bilateral <= c.gross);
        }
        let total_collateral: i128 = participants(&obs).unwrap().iter().map(|p| p.collateral).sum();
        let settled: i128 = net(&obs).unwrap().assets.iter().map(|a| a.settled).sum();
        prop_assert_eq!(total_collateral, settled, "collateral needed equals what netting moves");
    }
}

#[test]
fn a_rejected_obligation_leaves_the_netter_untouched() {
    let mut n = Netter::default();
    n.add(&ob("1", "A", "B", "X", i128::MAX)).unwrap();
    let before = n.netting().unwrap();
    assert_eq!(n.add(&ob("2", "C", "B", "X", 1)), Err(Error::Overflow));
    assert_eq!(n.len(), 1);
    assert_eq!(n.netting().unwrap(), before);
    assert_eq!(n.position("X", "C"), 0, "the debtor of the rejected obligation was not touched");
}

proptest! {
    #[test]
    fn incremental_netting_matches_batch_netting(obs in window()) {
        let mut n = Netter::default();
        for o in &obs {
            n.add(o).unwrap();
        }
        prop_assert_eq!(n.netting().unwrap(), net(&obs).unwrap());
        for p in net(&obs).unwrap().positions {
            prop_assert_eq!(n.position(&p.asset, &p.participant), p.net);
        }
    }
}

#[test]
fn validate_reports_every_bad_obligation() {
    let window = [
        ob("1", "A", "B", "X", 5),
        ob("2", "A", "A", "X", 5),
        ob("3", "A", "B", "X", 0),
        ob("1", "B", "C", "X", 5),
        ob("4", "", "C", "X", 5),
    ];
    let problems = validate(&window);
    assert_eq!(problems.iter().map(|(i, _)| *i).collect::<Vec<_>>(), vec![1, 2, 3, 4]);
    assert_eq!(problems[2].1, Error::DuplicateId("1".into()));
    assert!(validate(&scenarios()[2].2).is_empty());
}

#[test]
fn largest_first_pairs_big_debtors_with_big_creditors() {
    // D1 owes 100, D2 owes 10; C1 is owed 10, C2 is owed 100.
    let window = [ob("1", "D1", "C2", "X", 100), ob("2", "D2", "C1", "X", 10)];
    let ordered = net(&window).unwrap();
    let largest = net_with(&window, Plan::LargestFirst).unwrap();
    assert_eq!(ordered.transfers.len(), 3, "id order splits D1's debt");
    assert_eq!(largest.transfers.len(), 2, "largest-first matches 100 with 100");
    assert_eq!(largest.positions, ordered.positions);
}

proptest! {
    #[test]
    fn every_strategy_settles_exactly(obs in window()) {
        let n = net_with(&obs, Plan::LargestFirst).unwrap();
        let mut moved: BTreeMap<(&str, &str), i128> = BTreeMap::new();
        for t in &n.transfers {
            *moved.entry((&t.asset, &t.from)).or_default() -= t.amount;
            *moved.entry((&t.asset, &t.to)).or_default() += t.amount;
        }
        for p in &n.positions {
            prop_assert_eq!(moved.get(&(p.asset.as_str(), p.participant.as_str())).copied().unwrap_or(0), p.net);
        }
        for a in &n.assets {
            let k = n.positions.iter().filter(|p| p.asset == a.asset).count();
            prop_assert!(a.transfers <= k.saturating_sub(1));
        }
        prop_assert_eq!(n.positions, net(&obs).unwrap().positions);
    }
}

proptest! {
    #[test]
    fn json_round_trips_every_value(obs in window(), extreme in any::<i128>()) {
        let n = net(&obs).unwrap();
        let back: Netting = serde_json::from_str(&serde_json::to_string(&n).unwrap()).unwrap();
        prop_assert_eq!(back, n);
        let o = ob("x", "A", "B", "X", extreme);
        let back: Obligation = serde_json::from_str(&serde_json::to_string(&o).unwrap()).unwrap();
        prop_assert_eq!(back.amount, extreme, "i128 survives JSON as a string");
    }
}
