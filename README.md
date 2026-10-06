<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/logo-dark.svg">
    <img src="assets/logo.svg" alt="SetOff" height="64">
  </picture>
</p>

<p align="center"><b>Deterministic multilateral netting. Obligations in; net positions and a settlement plan with at most n−1 transfers out.</b></p>

<p align="center">
  <a href="https://github.com/SetOff-Org/setoff-engine/actions/workflows/ci.yml"><img src="https://github.com/SetOff-Org/setoff-engine/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue" alt="License"></a>
</p>

---

```rust
use setoff_engine::{Obligation, net};

let ob = |id: &str, from: &str, to: &str, amount| Obligation::new(id, from, to, "USDC", amount);
let n = net(&[ob("1", "A", "B", 100), ob("2", "B", "C", 100), ob("3", "C", "A", 100)])?;
assert!(n.transfers.is_empty()); // a perfect cycle settles with nothing moving
```

What the reference cases in [`tests/vectors`](tests/vectors) show:

| Window | Obligations | Gross → settled | Saved | Transfers |
|---|---|---|---|---|
| Three-way cycle | 3 | 300 → 0 | 100% | 0 |
| Remittance corridor, USDC | 5 | 1,145.0 → 290.0 | 74.7% | 2 |
| Remittance corridor, EURC | 2 | 135.0 → 5.0 | 96.3% | 1 |
| 200 pseudo-random, USDC | 145 | 715.48 → 141.83 | 80.2% | 11 |
| Fan-in to one merchant | 5 | 150 → 150 | 0% | 5 |

## Guarantees

Each is a property test over randomly generated windows:

- Net positions per asset sum to zero.
- The plan settles every position exactly, with at most `k − 1` transfers for
  `k` non-zero positions.
- The output depends only on the *set* of obligations: shuffling the input
  never changes a byte.
- Arithmetic is checked over `i128`; amounts travel as canonical decimal
  strings, because JSON numbers can't hold an `i128`.

Order independence is what lets a second implementation agree exactly.
[setoff-clearing](https://github.com/SetOff-Org/setoff-clearing) nets in Go and
must reproduce every vector here. CI checks both sides.

## Updating the vectors

```sh
UPDATE_FIXTURES=1 cargo test
```

Copy any changed vector to `setoff-clearing/testdata/vectors` in the same
change. If the Go implementation then disagrees, one of the two has a bug.

## Related

- [setoff-contracts](https://github.com/SetOff-Org/setoff-contracts): on-chain
  settlement with guaranteed finality.
- [setoff-clearing](https://github.com/SetOff-Org/setoff-clearing): the clearing
  service and `setoff` CLI.

## License

[Apache-2.0](LICENSE)
