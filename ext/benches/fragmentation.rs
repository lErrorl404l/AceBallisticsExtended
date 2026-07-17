// ABE — Fragmentation Benchmarks
//
// Measures:
//   - fragment_sample_10000: 200× evaluate() calls generating ~10000 fragment mass samples
//   - fragment_distribution_15steps: M193-like bullet at 15 velocity steps (400→1100 m/s)
//
// Run:  cargo bench --bench fragmentation

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use abe_ballistics_ext::fragmentation;

/// Generate ~10000 fragment mass samples by calling evaluate() 200 times.
///
/// Each evaluate() call with FMJ at high velocity produces up to 50 fragments;
/// 200 × 50 = 10000 total fragment mass samples processed.
fn bench_fragment_sample(c: &mut Criterion) {
    c.bench_function("fragmentation/fragment_sample_10000", |b| {
        b.iter(|| {
            let mut total_mass = 0.0f64;
            for _ in 0..200 {
                let result = fragmentation::evaluate(
                    black_box(1500.0), // well above threshold → max fragments
                    black_box(10.0),   // 10g projectile
                    black_box("fmj"),
                    black_box(762.0),
                    None,
                );
                for f in &result.fragments {
                    total_mass += f.mass_g;
                }
            }
            black_box(total_mass)
        })
    });
}

/// Compute the fragment distribution for an M193-like bullet (3.6 g, FMJ)
/// across 15 velocity steps from 400 m/s to 1100 m/s.
///
/// Velocities span subsonic (no fragmentation), transonic, and supersonic
/// regimes to capture both the early-return and full-computation paths.
fn bench_fragment_distribution(c: &mut Criterion) {
    let velocities: [f64; 15] = [
        400.0, 450.0, 500.0, 550.0, 600.0, 650.0, 700.0, 750.0, 800.0, 850.0, 900.0, 950.0, 1000.0,
        1050.0, 1100.0,
    ];

    c.bench_function("fragmentation/fragment_distribution_15steps", |b| {
        b.iter(|| {
            let mut total_frags = 0i32;
            for &v in &velocities {
                let result = fragmentation::evaluate(
                    black_box(v),
                    black_box(3.6), // M193 projectile mass (55 gr → 3.6 g)
                    black_box("fmj"),
                    black_box(762.0), // fragmentation threshold (sonic)
                    None,
                );
                total_frags += result.num_fragments;
            }
            black_box(total_frags)
        })
    });
}

criterion_group!(benches, bench_fragment_sample, bench_fragment_distribution);
criterion_main!(benches);
