# Contributing

The netting engine is SetOff's reference implementation: the clearing service,
the simulator and the reference vectors all follow it. It takes part in the
[Stellar Wave](https://www.drips.network/wave/stellar) program; Wave issues are
labeled with their complexity. The ground rules for every SetOff repository are
in the [organization guide](https://github.com/SetOff-Org/.github/blob/main/CONTRIBUTING.md).

## Setup

```sh
git clone https://github.com/SetOff-Org/setoff-engine && cd setoff-engine
cargo test --all-features
```

`rust-toolchain.toml` pins the toolchain and its targets; the minimum supported
Rust version is 1.88. The crate is `no_std` + `alloc`.

## Before you open a PR

```sh
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --no-default-features --target wasm32v1-none
```

CI also runs cargo-deny, the MSRV check, and the reference vectors through the
WebAssembly build in Node ([`bindings/wasm/test`](bindings/wasm/test/)).

## Reference vectors

`tests/vectors/*.json` are the contract between implementations. If netting
output changes, regenerate them with `UPDATE_FIXTURES=1 cargo test`, explain why
in the PR, and expect a follow-up in
[setoff-clearing](https://github.com/SetOff-Org/setoff-clearing), whose CI
requires identical vectors.

## The simulator

`site/` is a static page over `bindings/wasm`. To run it locally:

```sh
cargo build --release -p setoff-engine-wasm --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir site/pkg target/wasm32-unknown-unknown/release/setoff_engine_wasm.wasm
python -m http.server -d site
```

## Commit messages

[Conventional Commits](https://www.conventionalcommits.org): `feat(engine): …`,
`fix(wasm): …`, `docs: …`.
