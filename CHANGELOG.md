# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- `setoff-engine-wasm` (`bindings/wasm`): the netting engine for JavaScript,
  with per-row validation errors and both plan strategies.
- A [netting simulator](https://setoff-org.github.io/setoff-engine/) on
  GitHub Pages, with examples, live editing and shareable scenarios.

## [0.1.0] - 2026-10-07

### Added

- `net()`: positions, a settlement plan with at most k−1 transfers per asset,
  and per-asset totals, independent of input order.
- `Netter`: incremental netting with atomic, all-or-nothing `add`.
- `net_with()` and `Strategy::LargestFirst` as an alternative plan.
- `compare()`: gross vs. bilateral vs. multilateral settlement per asset.
- `participants()`: each party's flows and required collateral.
- `validate()`: every invalid obligation in one pass.
- `read_csv()` and `write_plan_csv()` (`csv` feature): obligations in, the
  settlement plan out as `asset,from,to,amount`; and a public canonical
  `parse_amount()`.
- `Netting::for_participant()` and collecting obligations into a `Netter`.
- `no_std` + `alloc` builds (default features off), checked for `wasm32v1-none`.
- Reference vectors with a written format (`docs/vectors.md`) and an algorithm
  write-up with proofs (`docs/algorithm.md`).
- Benchmarks: 100,000 obligations in about 0.29 s.

[Unreleased]: https://github.com/SetOff-Org/setoff-engine/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/SetOff-Org/setoff-engine/releases/tag/v0.1.0
