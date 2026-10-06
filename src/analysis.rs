//! What netting saves, and what each participant needs.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::{Error, Obligation, amount, net};

/// Gross, bilateral and multilateral settlement of one asset, side by side.
///
/// Bilateral netting offsets each pair's mutual obligations (what most
/// correspondent arrangements do today). Multilateral netting offsets
/// everyone against everyone. Always `multilateral <= bilateral <= gross`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comparison {
    /// Asset.
    pub asset: String,
    /// Moved if every obligation settles on its own.
    #[serde(with = "amount")]
    pub gross: i128,
    /// Moved if each pair settles only its difference.
    #[serde(with = "amount")]
    pub bilateral: i128,
    /// Moved if every participant settles only its net position.
    #[serde(with = "amount")]
    pub multilateral: i128,
    /// Payments needed bilaterally (pairs with a non-zero difference).
    pub bilateral_transfers: usize,
    /// Payments needed multilaterally.
    pub multilateral_transfers: usize,
}

/// One participant's flows in one asset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParticipantSummary {
    /// Asset.
    pub asset: String,
    /// Participant.
    pub participant: String,
    /// Total it owes others.
    #[serde(with = "amount")]
    pub owes: i128,
    /// Total others owe it.
    #[serde(with = "amount")]
    pub owed: i128,
    /// `owed - owes`; positive receives.
    #[serde(with = "amount")]
    pub net: i128,
    /// Collateral it must post for the window to settle: `max(0, -net)`.
    #[serde(with = "amount")]
    pub collateral: i128,
}

/// Compares settlement styles per asset, sorted by asset.
pub fn compare(obligations: &[Obligation]) -> Result<Vec<Comparison>, Error> {
    let netting = net(obligations)?;
    // asset -> (low, high) -> amount owed low->high minus high->low
    let mut pairs: BTreeMap<&str, BTreeMap<(&str, &str), i128>> = BTreeMap::new();
    for o in obligations {
        let (key, sign) = if o.debtor < o.creditor {
            ((o.debtor.as_str(), o.creditor.as_str()), 1)
        } else {
            ((o.creditor.as_str(), o.debtor.as_str()), -1)
        };
        let e = pairs.entry(&o.asset).or_default().entry(key).or_default();
        *e = e.checked_add(o.amount.checked_mul(sign).ok_or(Error::Overflow)?).ok_or(Error::Overflow)?;
    }
    netting
        .assets
        .iter()
        .map(|a| {
            let pair_nets = pairs.get(a.asset.as_str());
            let bilateral = pair_nets.map_or(Ok(0), |p| {
                p.values().try_fold(0i128, |acc, v| {
                    acc.checked_add(v.checked_abs().ok_or(Error::Overflow)?).ok_or(Error::Overflow)
                })
            })?;
            Ok(Comparison {
                asset: a.asset.clone(),
                gross: a.gross,
                bilateral,
                multilateral: a.settled,
                bilateral_transfers: pair_nets.map_or(0, |p| p.values().filter(|v| **v != 0).count()),
                multilateral_transfers: a.transfers,
            })
        })
        .collect()
}

/// Per-participant flows and collateral needs, sorted by asset then participant.
pub fn participants(obligations: &[Obligation]) -> Result<Vec<ParticipantSummary>, Error> {
    net(obligations)?; // same validation as netting
    let mut book: BTreeMap<(&str, &str), (i128, i128)> = BTreeMap::new();
    for o in obligations {
        let d = book.entry((&o.asset, &o.debtor)).or_default();
        d.0 = d.0.checked_add(o.amount).ok_or(Error::Overflow)?;
        let c = book.entry((&o.asset, &o.creditor)).or_default();
        c.1 = c.1.checked_add(o.amount).ok_or(Error::Overflow)?;
    }
    book.into_iter()
        .map(|((asset, participant), (owes, owed))| {
            let net = owed.checked_sub(owes).ok_or(Error::Overflow)?;
            Ok(ParticipantSummary {
                asset: asset.into(),
                participant: participant.into(),
                owes,
                owed,
                net,
                collateral: net.checked_neg().ok_or(Error::Overflow)?.max(0),
            })
        })
        .collect()
}
