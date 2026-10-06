//! Deterministic multilateral netting.
//!
//! Give it every obligation of a settlement window; it returns each
//! participant's net position per asset and a settlement plan that moves only
//! those nets. A ring of payments that cancels out settles with no transfers
//! at all.
//!
//! ```
//! use setoff_engine::{Obligation, net};
//!
//! let ob = |id: &str, from: &str, to: &str, amount| Obligation::new(id, from, to, "USDC", amount);
//! // A owes B 100, B owes C 100, C owes A 100: a perfect cycle.
//! let n = net(&[ob("1", "A", "B", 100), ob("2", "B", "C", 100), ob("3", "C", "A", 100)])?;
//! assert!(n.transfers.is_empty());
//! assert_eq!(n.assets[0].gross, 300);
//! assert_eq!(n.assets[0].settled, 0);
//! # Ok::<(), setoff_engine::Error>(())
//! ```
//!
//! The output depends only on the set of obligations, never on their order,
//! so independent implementations agree byte for byte. `tests/vectors` holds
//! the reference cases.
//!
//! The crate is `no_std` with `alloc` when the default `std` feature is off,
//! so the same algorithm can run inside a Soroban contract or a zkVM.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

mod analysis;
pub use analysis::{Comparison, ParticipantSummary, compare, participants};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

/// Something one participant owes another.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Obligation {
    /// Unique within the window.
    pub id: String,
    /// Who pays.
    pub debtor: String,
    /// Who is paid.
    pub creditor: String,
    /// Asset identifier, e.g. `USDC:G…` or a contract address.
    pub asset: String,
    /// Amount in the asset's smallest unit.
    #[serde(with = "amount")]
    pub amount: i128,
}

impl Obligation {
    /// Convenience constructor.
    pub fn new(id: &str, debtor: &str, creditor: &str, asset: &str, amount: i128) -> Self {
        Self { id: id.into(), debtor: debtor.into(), creditor: creditor.into(), asset: asset.into(), amount }
    }
}

/// A participant's net result for one asset: positive means it receives.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    /// Asset.
    pub asset: String,
    /// Participant.
    pub participant: String,
    /// Net amount; positive receives, negative pays.
    #[serde(with = "amount")]
    pub net: i128,
}

/// One movement of the settlement plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transfer {
    /// Asset.
    pub asset: String,
    /// Payer (a net debtor).
    pub from: String,
    /// Payee (a net creditor).
    pub to: String,
    /// Amount.
    #[serde(with = "amount")]
    pub amount: i128,
}

/// Per-asset totals.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetSummary {
    /// Asset.
    pub asset: String,
    /// Obligations in the asset.
    pub obligations: usize,
    /// Sum of all obligations: what gross settlement would move.
    #[serde(with = "amount")]
    pub gross: i128,
    /// Sum of all net debits: what netted settlement moves.
    #[serde(with = "amount")]
    pub settled: i128,
    /// Transfers in the plan.
    pub transfers: usize,
}

impl AssetSummary {
    /// Share of gross volume that never has to move, in basis points (0–10000).
    pub fn saving_bps(&self) -> u32 {
        if self.gross == 0 {
            return 0;
        }
        let saved = self.gross.saturating_sub(self.settled).saturating_mul(10_000);
        u32::try_from(saved.checked_div(self.gross).unwrap_or(0)).unwrap_or(0)
    }
}

/// The result of netting a window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Netting {
    /// Non-zero positions, sorted by asset then participant.
    pub positions: Vec<Position>,
    /// Settlement plan, sorted by asset then the greedy pairing order.
    pub transfers: Vec<Transfer>,
    /// Totals per asset, sorted by asset.
    pub assets: Vec<AssetSummary>,
}

/// Why a window cannot be netted.
#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// Amounts must be positive.
    #[error("obligation {0}: amount must be positive")]
    NonPositiveAmount(String),
    /// A participant cannot owe itself.
    #[error("obligation {0}: debtor and creditor are the same")]
    SelfObligation(String),
    /// IDs must be unique within a window.
    #[error("obligation {0}: duplicate id")]
    DuplicateId(String),
    /// IDs, participants and assets must be non-empty.
    #[error("obligation {id:?}: {field} is empty")]
    Empty {
        /// Obligation id.
        id: String,
        /// Field name.
        field: &'static str,
    },
    /// A total exceeded the i128 range.
    #[error("amounts overflow")]
    Overflow,
}

/// Every problem in a window, in input order, instead of stopping at the first.
///
/// Useful when importing obligations from a file: one pass lists every bad
/// row. Overflow is detected cumulatively, as `net` would.
pub fn validate(obligations: &[Obligation]) -> Vec<(usize, Error)> {
    let mut n = Netter::default();
    obligations.iter().enumerate().filter_map(|(i, o)| n.add(o).err().map(|e| (i, e))).collect()
}

/// How the settlement plan pairs debtors with creditors.
///
/// Both strategies settle every position exactly with at most `k - 1`
/// transfers; they differ only in which pairs pay each other.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Strategy {
    /// Walk debtors and creditors in participant-id order. The default, and
    /// the one every reference vector (and every other implementation) uses.
    #[default]
    ParticipantOrder,
    /// Pair the largest debtors with the largest creditors first (ties by id).
    /// Tends to produce fewer, larger transfers.
    LargestFirst,
}

/// Nets a window with a chosen plan strategy.
pub fn net_with(obligations: &[Obligation], strategy: Strategy) -> Result<Netting, Error> {
    let mut n = Netter::default();
    for o in obligations {
        n.add(o)?;
    }
    n.netting_with(strategy)
}

/// Nets a window of obligations.
pub fn net(obligations: &[Obligation]) -> Result<Netting, Error> {
    let mut n = Netter::default();
    for o in obligations {
        n.add(o)?;
    }
    n.netting()
}

/// Nets obligations as they arrive, for services that keep a window open.
///
/// `add` validates each obligation and either records it or leaves the netter
/// unchanged; `position` is a map lookup; `netting` produces exactly what
/// [`net`] would for the same obligations.
#[derive(Clone, Debug, Default)]
pub struct Netter {
    ids: BTreeSet<String>,
    // asset -> participant -> net
    book: BTreeMap<String, BTreeMap<String, i128>>,
    // asset -> (count, gross)
    totals: BTreeMap<String, (usize, i128)>,
}

impl Netter {
    /// Records one obligation, or returns why it is invalid and changes nothing.
    pub fn add(&mut self, o: &Obligation) -> Result<(), Error> {
        for (field, value) in [("id", &o.id), ("debtor", &o.debtor), ("creditor", &o.creditor), ("asset", &o.asset)] {
            if value.is_empty() {
                return Err(Error::Empty { id: o.id.clone(), field });
            }
        }
        if o.amount <= 0 {
            return Err(Error::NonPositiveAmount(o.id.clone()));
        }
        if o.debtor == o.creditor {
            return Err(Error::SelfObligation(o.id.clone()));
        }
        if self.ids.contains(&o.id) {
            return Err(Error::DuplicateId(o.id.clone()));
        }
        // Compute every new value before writing any, so a failure leaves no trace.
        let asset = self.book.get(&o.asset);
        let get = |p: &str| asset.and_then(|a| a.get(p)).copied().unwrap_or(0);
        let debtor = get(&o.debtor).checked_sub(o.amount).ok_or(Error::Overflow)?;
        let creditor = get(&o.creditor).checked_add(o.amount).ok_or(Error::Overflow)?;
        let (count, gross) = self.totals.get(&o.asset).copied().unwrap_or_default();
        let gross = gross.checked_add(o.amount).ok_or(Error::Overflow)?;

        // Only allocate keys the maps have not seen: assets and participants repeat constantly.
        if !self.book.contains_key(&o.asset) {
            self.book.insert(o.asset.clone(), BTreeMap::new());
        }
        if let Some(book) = self.book.get_mut(&o.asset) {
            set(book, &o.debtor, debtor);
            set(book, &o.creditor, creditor);
        }
        set(&mut self.totals, &o.asset, (count.saturating_add(1), gross));
        self.ids.insert(o.id.clone());
        Ok(())
    }

    /// A participant's current net position in an asset.
    pub fn position(&self, asset: &str, participant: &str) -> i128 {
        self.book.get(asset).and_then(|a| a.get(participant)).copied().unwrap_or(0)
    }

    /// Obligations recorded so far.
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// Whether nothing has been recorded.
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// Positions, settlement plan and totals for everything recorded so far.
    pub fn netting(&self) -> Result<Netting, Error> {
        self.netting_with(Strategy::ParticipantOrder)
    }

    /// Like [`Netter::netting`], with a chosen plan strategy.
    pub fn netting_with(&self, strategy: Strategy) -> Result<Netting, Error> {
        let mut out = Netting { positions: Vec::new(), transfers: Vec::new(), assets: Vec::new() };
        for (asset, participants) in &self.book {
            let mut debtors: Vec<(&str, i128)> = Vec::new();
            let mut creditors: Vec<(&str, i128)> = Vec::new();
            let mut settled: i128 = 0;
            for (p, n) in participants {
                if *n != 0 {
                    out.positions.push(Position { asset: asset.clone(), participant: p.clone(), net: *n });
                }
                if *n < 0 {
                    let owed = n.checked_neg().ok_or(Error::Overflow)?;
                    debtors.push((p, owed));
                    settled = settled.checked_add(owed).ok_or(Error::Overflow)?;
                } else if *n > 0 {
                    creditors.push((p, *n));
                }
            }
            let first = out.transfers.len();
            if strategy == Strategy::LargestFirst {
                // Descending by amount, then by participant id, so the order is still total.
                debtors.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
                creditors.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
            }
            plan(asset, &mut debtors, &mut creditors, &mut out.transfers)?;
            let (count, gross) = self.totals.get(asset).copied().unwrap_or_default();
            out.assets.push(AssetSummary {
                asset: asset.clone(),
                obligations: count,
                gross,
                settled,
                transfers: out.transfers.len().saturating_sub(first),
            });
        }
        Ok(out)
    }
}

/// Updates `key` in place, cloning it only when it is new.
fn set<V>(map: &mut BTreeMap<String, V>, key: &str, value: V) {
    match map.get_mut(key) {
        Some(slot) => *slot = value,
        None => {
            map.insert(key.into(), value);
        }
    }
}

/// Pairs debtors with creditors in participant order. Each step fully settles
/// at least one side, so an asset with `k` non-zero positions needs at most
/// `k - 1` transfers.
fn plan(
    asset: &str,
    debtors: &mut [(&str, i128)],
    creditors: &mut [(&str, i128)],
    out: &mut Vec<Transfer>,
) -> Result<(), Error> {
    let (mut i, mut j) = (0usize, 0usize);
    while let (Some(d), Some(c)) = (debtors.get(i).copied(), creditors.get(j).copied()) {
        let amount = d.1.min(c.1);
        out.push(Transfer { asset: asset.into(), from: d.0.into(), to: c.0.into(), amount });
        let rest_d = d.1.checked_sub(amount).ok_or(Error::Overflow)?;
        let rest_c = c.1.checked_sub(amount).ok_or(Error::Overflow)?;
        if let Some(slot) = debtors.get_mut(i) {
            slot.1 = rest_d;
        }
        if let Some(slot) = creditors.get_mut(j) {
            slot.1 = rest_c;
        }
        if rest_d == 0 {
            i = i.saturating_add(1);
        }
        if rest_c == 0 {
            j = j.saturating_add(1);
        }
    }
    Ok(())
}

/// Amounts travel as decimal strings: JSON numbers cannot hold an i128.
mod amount {
    use super::*;

    pub fn serialize<S: Serializer>(v: &i128, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(v)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<i128, D::Error> {
        let s = String::deserialize(d)?;
        if s.is_empty() || s.starts_with('+') || (s.len() > 1 && s.trim_start_matches('-').starts_with('0')) {
            return Err(serde::de::Error::custom(format!("not a canonical integer: {s:?}")));
        }
        s.parse().map_err(serde::de::Error::custom)
    }
}
