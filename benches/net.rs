//! Netting throughput on pseudo-random windows.

#![allow(clippy::unwrap_used, clippy::arithmetic_side_effects, missing_docs)]

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use setoff_engine::{Obligation, net};

/// `n` obligations among `participants`, two assets, from a fixed seed.
fn window(n: usize, participants: usize) -> Vec<Obligation> {
    let mut x: u64 = 7;
    let mut next = || {
        x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (x >> 33) as usize
    };
    (0..n)
        .map(|i| {
            let d = next() % participants;
            let c = (d + 1 + next() % (participants - 1)) % participants;
            let asset = if next() % 4 == 0 { "EURC" } else { "USDC" };
            Obligation::new(
                &format!("{i}"),
                &format!("P{d}"),
                &format!("P{c}"),
                asset,
                (1 + next() % 1_000_000) as i128,
            )
        })
        .collect()
}

fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("net");
    for (n, p) in [(1_000, 20), (10_000, 100), (100_000, 500)] {
        let obs = window(n, p);
        group.bench_with_input(BenchmarkId::new("obligations", n), &obs, |b, obs| b.iter(|| net(obs).unwrap()));
    }
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
