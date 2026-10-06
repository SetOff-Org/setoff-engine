# The netting algorithm

This page states what `net()` computes and why its guarantees hold. Every
guarantee below is also a property test in `tests/engine.rs`.

## Input

A window is a set of obligations `(id, debtor, creditor, asset, amount)` with
unique ids, non-empty fields, `debtor ≠ creditor` and `amount > 0`. Anything
else is rejected before netting starts.

## Positions

For each asset `a` and participant `p`:

```
net(a, p) = Σ amount over obligations with creditor p  −  Σ amount over obligations with debtor p
```

Positive means `p` receives, negative means `p` pays. Arithmetic is checked
`i128`; any overflow rejects the window.

**Conservation.** Each obligation adds `+amount` to one participant and
`−amount` to another in the same asset, so `Σ_p net(a, p) = 0` for every
asset. In particular, total net debits equal total net credits; that common
value is what settlement moves, reported as `settled`.

## The settlement plan

Per asset, list the debtors (`net < 0`) and creditors (`net > 0`), each sorted
by participant id. Then repeat until either list is exhausted:

```
pay = min(remaining debt of debtor i, remaining credit of creditor j)
emit transfer(debtor i → creditor j, pay)
advance i if its debt is now 0; advance j if its credit is now 0
```

**Exact settlement.** Each transfer reduces one debtor's remaining debt and
one creditor's remaining credit by the same amount, and no transfer exceeds
either. Because total debt equals total credit (conservation), both lists run
out at the same step, and every remaining amount is exactly zero. So applying
the plan moves every participant by exactly its net position.

**At most k − 1 transfers.** Every step advances `i`, `j`, or both, because
`pay` equals at least one of the two remaining amounts. With `d` debtors and
`c` creditors there are at most `d + c − 1` steps, since the final step
advances both. With `k = d + c` non-zero positions that is `k − 1` transfers.
Finding the *minimum* number of transfers is NP-hard in general (it contains
subset sum), so the engine takes this bound and determinism over optimality.

**Never worse than bilateral netting.** Let `d(p, q) = owed(q→p) − owed(p→q)`.
Bilateral netting settles each pair's difference, moving
`B = Σ_{pairs} |d(p, q)| = ½ Σ_p Σ_q |d(p, q)|` (each pair appears twice in the
double sum). Since `net(p) = Σ_q d(p, q)`, the triangle inequality gives
`|net(p)| ≤ Σ_q |d(p, q)|`. Multilateral settlement moves
`M = Σ_p max(0, −net(p)) = ½ Σ_p |net(p)|` (by conservation), so
`M ≤ ½ Σ_p Σ_q |d(p, q)| = B`. And `B ≤ gross` because each pair's difference
is at most the pair's total. So `multilateral ≤ bilateral ≤ gross`.

## Determinism

Positions are kept in ordered maps keyed by asset and participant, and the
plan walks both lists in participant-id order (byte order of UTF-8). Nothing
depends on the order obligations arrive, so the output depends only on the
*set* of obligations. That is what lets `setoff-clearing`'s Go implementation
agree with this one byte for byte on every reference vector.

## Collateral

A participant can settle the window if it posts `max(0, −net(a, p))` of each
asset. The sum of required collateral equals `settled`: netting reduces
liquidity needs exactly as far as it reduces movement. `participants()` reports
these figures, and `setoff-contracts` enforces the same rule on chain: no
batch may leave a member's net debit uncovered.

## Cost

`net()` is `O(n log m)` for `n` obligations among `m` participants. On a
laptop it nets 1,000 obligations in about 1.7 ms and 100,000 in about 0.29 s
(`cargo bench`).
