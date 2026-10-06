# Reference vectors

`tests/vectors/*.json` are the conformance suite for any implementation of
SetOff netting. [setoff-clearing](https://github.com/SetOff-Org/setoff-clearing)
(Go) reproduces all of them; a new implementation should too.

## Format

```json
{
  "name": "bilateral",
  "description": "Two participants owe each other; only the difference moves.",
  "obligations": [
    { "id": "1", "debtor": "A", "creditor": "B", "asset": "USDC", "amount": "100" },
    { "id": "2", "debtor": "B", "creditor": "A", "asset": "USDC", "amount": "60" }
  ],
  "netting": {
    "positions": [
      { "asset": "USDC", "participant": "A", "net": "-40" },
      { "asset": "USDC", "participant": "B", "net": "40" }
    ],
    "transfers": [{ "asset": "USDC", "from": "A", "to": "B", "amount": "40" }],
    "assets": [{ "asset": "USDC", "obligations": 2, "gross": "160", "settled": "40", "transfers": 1 }]
  }
}
```

- Amounts are decimal strings of signed 128-bit integers: no `+`, no leading
  zeros, no `-0`. Counts (`obligations`, `transfers`) are JSON numbers.
- `positions` lists only non-zero positions, sorted by asset, then participant.
- `transfers` are grouped by asset (sorted) and, within an asset, in plan order.
- `assets` is sorted by asset.
- Strings sort by UTF-8 byte order.

## Conformance

An implementation conforms when, for every vector, netting `obligations` with
the default `ParticipantOrder` strategy yields a result equal to `netting`
(compare the decoded structures, or the JSON with a canonical encoder).

The vectors cover a cancelling cycle, a bilateral pair, a two-asset
remittance corridor, a fan-in where netting saves nothing, and 200
pseudo-random obligations among 12 participants. The algorithm they encode is
described in [algorithm.md](algorithm.md).

## Regenerating

```sh
UPDATE_FIXTURES=1 cargo test --test engine reference_vectors_are_current
```

A change to any vector is a breaking change for every implementation: update
setoff-clearing in the same release.
