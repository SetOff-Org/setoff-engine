//! The SetOff netting engine for JavaScript.
//!
//! ```js
//! import init, { net } from "./setoff_engine_wasm.js";
//! await init();
//! const out = JSON.parse(net(JSON.stringify(obligations), "participant-order"));
//! if (!out.ok) showErrors(out.errors); // [{ index, message }]
//! else render(out.netting);           // positions, transfers, assets
//! ```
//!
//! Obligations use the engine's JSON form: `{ id, debtor, creditor, asset,
//! amount }` with `amount` a canonical integer string. Results are exactly the
//! engine's, so they match every reference vector.

use serde::Serialize;
use setoff_engine::{Obligation, Strategy, net_with, validate};
use wasm_bindgen::prelude::*;

/// What `net` returns: `ok` is a real boolean, so `if (!out.ok)` works in JS.
#[derive(Serialize)]
struct Outcome {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    netting: Option<setoff_engine::Netting>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    errors: Vec<RowError>,
}

impl Outcome {
    fn rejected(errors: Vec<RowError>) -> Self {
        Self { ok: false, netting: None, errors }
    }
}

#[derive(Serialize)]
struct RowError {
    /// Index of the obligation in the input, or -1 for the input as a whole.
    index: i64,
    message: String,
}

/// Nets a window of obligations given as JSON. `strategy` is
/// `"participant-order"` (the default, used by the reference vectors) or
/// `"largest-first"`. Never throws: invalid input yields `{ ok: false, errors }`.
#[wasm_bindgen]
pub fn net(obligations_json: &str, strategy: Option<String>) -> String {
    serde_json::to_string(&net_outcome(obligations_json, strategy.as_deref()))
        .unwrap_or_else(|_| r#"{"ok":false,"errors":[{"index":-1,"message":"internal error"}]}"#.into())
}

fn net_outcome(obligations_json: &str, strategy: Option<&str>) -> Outcome {
    let whole = |message: String| Outcome::rejected(vec![RowError { index: -1, message }]);
    let rows: Vec<serde_json::Value> = match serde_json::from_str(obligations_json) {
        Ok(v) => v,
        Err(e) => return whole(format!("not a list of obligations: {e}")),
    };
    // Parse rows one by one so a bad amount is reported against its own row.
    let mut obligations = Vec::with_capacity(rows.len());
    let mut errors = Vec::new();
    for (i, row) in rows.into_iter().enumerate() {
        match serde_json::from_value::<Obligation>(row) {
            Ok(o) => obligations.push(o),
            Err(e) => errors.push(RowError { index: i64::try_from(i).unwrap_or(-1), message: e.to_string() }),
        }
    }
    if !errors.is_empty() {
        return Outcome::rejected(errors);
    }
    let strategy = match strategy.unwrap_or("participant-order") {
        "participant-order" => Strategy::ParticipantOrder,
        "largest-first" => Strategy::LargestFirst,
        other => return whole(format!("unknown strategy {other:?}")),
    };
    let problems = validate(&obligations);
    if !problems.is_empty() {
        let errors = problems
            .into_iter()
            .map(|(i, e)| RowError { index: i64::try_from(i).unwrap_or(-1), message: e.to_string() })
            .collect();
        return Outcome::rejected(errors);
    }
    match net_with(&obligations, strategy) {
        Ok(netting) => Outcome { ok: true, netting: Some(netting), errors: Vec::new() },
        Err(e) => whole(e.to_string()),
    }
}

/// The engine version, for display.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").into()
}
