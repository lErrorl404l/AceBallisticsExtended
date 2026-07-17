# ABE Ballistics Extension — Performance Benchmarks

> Run: `cargo bench` from `ext/`  
> Quick check: `cargo bench -- --test` (dry-run, no measurement)  
> Generated: 2026-07-17  
> Machine: Linux, AMD64  
> Toolchain: Rust (release, opt-level=3, LTO, 1 codegen-unit)

## Overview

11 criterion groups with 19 individual benchmarks covering all major code paths:

| Category | Benchmarks | What's tested |
|---|---|---|
| `fire/*` | 5 | Interior ballistics (muzzle velocity for 4 calibers + 1 existing) |
| `step/*` | 2 | Exterior ballistics (RK4-like semi-implicit Euler, struct + string ABI) |
| `impact/*` | 5 | Terminal ballistics (penetration, ricochet, material variants) |
| `pipeline/*` | 2 | Full fire → 500-step → impact cycle (simple + realistic) |
| `multi_bullet/*` | 2 | Multi-bullet interleaved stepping (10×30 and 100×30) |
| `init/*` | 3 | Thread-safe state init via `OnceLock` (health, version, re-init) |

## Results

### 1. Fire — Interior Ballistics

| Benchmark | Mean time | Iterations/s (approx) |
|---|---|---|
| `fire/struct_abi` | **33.3 ns** | 30 M/s |
| `fire/5.56mm_m855` | **33.3 ns** | 30 M/s |
| `fire/7.62mm_m80` | **33.4 ns** | 30 M/s |
| `fire/9mm_parabellum` | **33.3 ns** | 30 M/s |
| `fire/338_lapua` | **33.4 ns** | 30 M/s |

**Interpretation:** All fire benchmarks converge to the same time (~33 ns) because the muzzle velocity calculation is a single formula (pressure curve integral → KE → velocity). Caliber differences only change input values, not the computation graph. This is the fastest kernel — pure math, no branching, no lookups.

---

### 2. Step — Exterior Ballistics

| Benchmark | Mean time | Iterations/s (approx) |
|---|---|---|
| `step/struct_abi` | **54.6 ns** | 18 M/s |
| `step/string_abi` | **1.51 µs** | 663 k/s |

**Interpretation:** struct ABI at 55 ns is the drag-table lookup plus semi-implicit Euler position/velocity update. The string ABI (1.5 µs) is **28× slower** — the overhead of parsing 17 string arguments (17× `f64::from_str()`) dominates. This is the cost SQF callers pay; for any batch stepping the struct ABI path is strongly preferred.

---

### 3. Impact — Terminal Ballistics

| Benchmark | Mean time | Iterations/s (approx) |
|---|---|---|
| `impact/struct_abi` | **1.19 µs** | 843 k/s |
| `impact/ap_vs_10mm_rha` | **631 ns** | 1.58 M/s |
| `impact/ball_vs_aluminum_thin` | **849 ns** | 1.18 M/s |
| `impact/ricochet_at_80deg` | **75.3 ns** | 13.3 M/s |
| `impact/apds_vs_ceramic` | **185 ns** | 5.41 M/s |

**Interpretation:** Impact is the most variable kernel. The struct_abi benchmark (1.19 µs) hits the full De Marre + fragmentation path. Ricochet (75 ns) is the fastest because it returns early from `evaluate()` before the heavy math. AP vs RHA (631 ns) is a mid-range penetration with fragmentation. APDS vs ceramic (185 ns) is fast because ceramic's high material factor prevents penetration, skipping residual velocity math.

---

### 4. Pipeline — Full Cycle

| Benchmark | Mean time | Iterations/s (approx) |
|---|---|---|
| `pipeline/fire_500step_impact` | **31.0 µs** | 32 k/s |
| `pipeline/fire_500step_impact_realistic` | **47.1 µs** | 21 k/s |

**Interpretation:** The realistic pipeline (with 5 m/s crosswind at 500 m altitude) is ~52% slower than the simple version. The additional cost comes from the ICAO atmosphere model (`density_from_altitude`), wind shear factor, and the altitude-based density path which triggers inside `abe_step` for every step. Both are well under 100 µs for a complete 500m trajectory — fast enough for real-time simulation of hundreds of simultaneous bullets.

---

### 5. Multi-Bullet

| Benchmark | Mean time | Iterations/s (approx) |
|---|---|---|
| `multi_bullet/10x30_interleaved` | **19.6 µs** | 51 k/s |
| `multi_bullet/100x30_interleaved` | **194 µs** | 5.2 k/s |

**Interpretation:** Scaling is linear — 10× the bullets = 9.9× the time (within noise). At 194 µs for 100 bullets × 30 steps each, you can simulate **~5,000 full salvos per second**. For typical ARMA 3 engagements (10–50 active projectiles), this is comfortably real-time.

---

### 6. Init — OnceLock State Management

| Benchmark | Mean time |
|---|---|
| `init/health_check` | **0.61 ns** (609 ps) |
| `init/version_string` | **0.62 ns** (615 ps) |
| `init/full_init` | **1.01 ns** |

**Interpretation:** The `OnceLock::get()` check on the hot path (every ABI function calls `get_state().initialized`) is **sub-nanosecond** — essentially free. The version string `OnceLock<CString>` is the same after first initialization. `init/full_init` measures `OnceLock::set()` on an already-initialized state (the `Err` path). All three are negligible and well within measurement noise.

## Key Takeaways

1. **Fire is free** (~33 ns) — pure math, no optimization needed.
2. **Step is the hot path** (55 ns struct, 1.5 µs string) — the 28× string-ABI overhead argues for a batch-stepping API for SQF callers.
3. **Impact costs vary by path** (75 ns ricochet → 1.2 µs full penetration) — the fragmentation model is the main cost.
4. **Pipeline throughput**: 21,000–32,000 full trajectories/second — fine for 50-bullet ARMA 3 engagements.
5. **Multi-bullet scales linearly** — no contention in the stateless design.
6. **OnceLock overhead is immeasurable** — 0.6 ns is in the noise floor of modern CPUs.

## CI Baseline

These numbers were established on:  
- CPU: AMD64 (unknown gen, Linux)  
- Rust profile: release (opt-level=3, LTO, 1 codegen-unit)  
- Criterion version: 0.5 with html_reports

To compare against a different machine, run:
```bash
cargo bench --target-dir /tmp/abe-bench
```

CI regression alert thresholds (suggested):
- `step/struct_abi`: alert if > 70 ns (current 55 ns + 25% headroom)
- `impact/struct_abi`: alert if > 1.5 µs
- `pipeline/fire_500step_impact`: alert if > 45 µs
