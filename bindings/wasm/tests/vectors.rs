//! The JavaScript entry point gives the engine's answer on every reference vector.

#![allow(missing_docs)]

use serde_json::Value;

fn run(obligations: &Value, strategy: Option<&str>) -> Value {
    serde_json::from_str(&setoff_engine_wasm::net(&obligations.to_string(), strategy.map(str::to_owned))).unwrap()
}

#[test]
fn every_reference_vector_nets_identically() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/vectors");
    let mut seen = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let v: Value = serde_json::from_str(&std::fs::read_to_string(entry.unwrap().path()).unwrap()).unwrap();
        let out = run(&v["obligations"], None);
        assert_eq!(out["ok"], true, "{}", v["name"]);
        assert_eq!(out["netting"], v["netting"], "{}", v["name"]);
        seen += 1;
    }
    assert!(seen >= 5);
}

#[test]
fn bad_rows_are_reported_against_their_index() {
    let obs = serde_json::json!([
        { "id": "1", "debtor": "A", "creditor": "B", "asset": "USDC", "amount": "100" },
        { "id": "2", "debtor": "B", "creditor": "C", "asset": "USDC", "amount": "1.5" },
        { "id": "3", "debtor": "C", "creditor": "C", "asset": "USDC", "amount": "5" },
    ]);
    let out = run(&obs, None);
    assert_eq!(out["ok"], false);
    assert_eq!(out["errors"][0]["index"], 1, "{out}");

    let selfpay = serde_json::json!([{ "id": "3", "debtor": "C", "creditor": "C", "asset": "USDC", "amount": "5" }]);
    let out = run(&selfpay, None);
    assert_eq!((out["ok"].clone(), out["errors"][0]["index"].clone()), (Value::Bool(false), Value::from(0)));

    assert_eq!(run(&Value::from("nope"), None)["errors"][0]["index"], -1);
    assert_eq!(run(&serde_json::json!([]), Some("sideways"))["ok"], false);
}

#[test]
fn both_strategies_settle_the_same_positions() {
    let obs = serde_json::json!([
        { "id": "1", "debtor": "D1", "creditor": "C2", "asset": "X", "amount": "100" },
        { "id": "2", "debtor": "D2", "creditor": "C1", "asset": "X", "amount": "10" },
    ]);
    let a = run(&obs, Some("participant-order"));
    let b = run(&obs, Some("largest-first"));
    assert_eq!(a["netting"]["positions"], b["netting"]["positions"]);
    assert!(b["netting"]["transfers"].as_array().unwrap().len() <= a["netting"]["transfers"].as_array().unwrap().len());
}
