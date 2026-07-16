#!/usr/bin/env -S uv run
"""
ABE Exterior Ballistics Cross-Validation
==========================================
Compares ABE's Rust trajectory integration against two Python reference
libraries: py-ballisticcalc (3DoF point-mass) and ballistics-engine.

Usage:
    uv run validate_exterior.py [--rust-so PATH]
"""

import sys
import math
import json
import os

# ── Test scenarios (from ABE data configs) ──────────────────
# Each: (name, mv_ms, bc_g7, mass_g, caliber_mm, cdm_id)
SCENARIOS = [
    ("5.56x45mm M855", 930, 0.157, 4.0, 5.56, "g7"),
    ("7.62x51mm M80", 850, 0.200, 9.5, 7.62, "g7"),
    ("5.45x39mm 7N6", 900, 0.145, 3.4, 5.45, "g7"),
    ("7.62x54R LPS", 830, 0.215, 9.6, 7.62, "g7"),
    ("M855A1 (enhanced)", 945, 0.161, 4.0, 5.56, "g7"),
]

SAMPLE_RANGES_M = [0, 100, 200, 300, 500, 800, 1000]


# ── ballistics-engine wrapper ───────────────────────────────
def _traj_be(scenario):
    """Compute trajectory with ballistics-engine (Rust Python bindings)."""
    import ballistics_engine as be

    name, mv, bc, mass, cal, cdm = scenario

    mv_fps = mv * 3.28084
    mass_gr = mass * 15.4324
    cal_in = cal / 25.4

    inputs = be.BallisticInputs.from_dict(
        {
            "muzzle_velocity_fps": mv_fps,
            "bullet_weight_grains": mass_gr,
            "bullet_diameter_inches": cal_in,
            "bc": bc,
            "drag_model": cdm,
            "sight_height_inches": 2.0,
            "zero_distance_yards": 300,
            "use_rk4": True,
            "sample_interval_m": 5.0,
            "muzzle_height_inches": 0.0,
            "use_adaptive_rk45": False,
            "target_height_inches": 12.0,
        }
    )
    solver = be.TrajectorySolver(inputs)
    result = solver.solve()

    raw = []
    for p in result.points:
        raw.append(
            {
                "x_m": p.x * 0.9144,
                "y_m": p.y * 0.9144,
                "v_ms": p.velocity_fps * 0.3048,
                "t_s": p.time,
            }
        )
    return _sample_points(raw, SAMPLE_RANGES_M)


# ── py-ballisticcalc wrapper ────────────────────────────────
def _traj_pbc(scenario):
    """Compute trajectory with py-ballisticcalc (pure Python 3DoF)."""
    import py_ballisticcalc as pb

    name, mv, bc, mass, cal, cdm = scenario
    pb.loadMetricUnits()

    dm = pb.DragModel(
        bc,
        pb.TableG7,
        pb.Weight(mass, pb.Unit.Gram),
        pb.Distance(cal, pb.Unit.Millimeter),
    )
    ammo = pb.Ammo(dm, pb.Velocity(mv, pb.Unit.MPS))
    weapon = pb.Weapon(sight_height=pb.Distance(0.05, pb.Unit.Meter))
    atmo = pb.Atmo(altitude=0, pressure=1013.25, temperature=15)
    shot = pb.Shot(ammo=ammo, weapon=weapon, atmo=atmo)

    calc = pb.Calculator()
    calc.set_weapon_zero(shot, pb.Distance(100, pb.Unit.Meter))

    max_r = max(SAMPLE_RANGES_M) + 50
    hit = calc.fire(
        shot,
        trajectory_range=pb.Distance(max_r, pb.Unit.Meter),
        trajectory_step=pb.Distance(5, pb.Unit.Meter),
    )

    raw = []
    for tp in hit.trajectory:
        x = tp.x.get_in(pb.Unit.Meter)
        y = tp.y.get_in(pb.Unit.Meter)
        v = tp.velocity.get_in(pb.Unit.MPS)
        raw.append({"x_m": x, "y_m": y, "v_ms": v, "t_s": tp.time})

    return _sample_points(raw, SAMPLE_RANGES_M)


# ── Helpers ─────────────────────────────────────────────────
def _sample_points(raw, ranges):
    sampled = []
    for r in ranges:
        best = None
        best_dx = float("inf")
        for p in raw:
            dx = abs(p["x_m"] - r)
            if dx < best_dx:
                best_dx = dx
                best = p
        if best and best_dx < 5.0:
            sampled.append(best)
        else:
            sampled.append(None)
    return sampled


# ── Main ────────────────────────────────────────────────────
def main():
    sepa = "─" * 100
    print("    ABE Exterior Ballistics — Cross-Validation Report\n")

    for scenario in SCENARIOS:
        name, mv, bc, mass, cal, cdm = scenario
        print(sepa)
        print(f"  {name}")
        print(f"  MV={mv} m/s  BC(G7)={bc}  Mass={mass}g  Caliber={cal}mm  Drag={cdm}")
        print(sepa)

        traj_pbc = _traj_pbc(scenario)
        traj_be = _traj_be(scenario)

        print(
            f"  {'Rng(m)':>6}  {'PyBC Drop(m)':>13}  {'PyBC Vel':>9}  "
            f"{'BE Drop(m)':>12}  {'BE Vel':>9}  {'Δ Drop':>8}"
        )
        print(f"  {'-' * 62}")

        for i, r in enumerate(SAMPLE_RANGES_M):
            pbc = traj_pbc[i]
            be_pt = traj_be[i]

            pbc_str = f"{pbc['y_m']:>8.3f}" if pbc else f"{'N/A':>8}"
            pbc_v = f"{pbc['v_ms']:>7.1f}" if pbc else f"{'N/A':>7}"
            be_str = f"{be_pt['y_m']:>8.3f}" if be_pt else f"{'N/A':>8}"
            be_v = f"{be_pt['v_ms']:>7.1f}" if be_pt else f"{'N/A':>7}"

            if pbc and be_pt:
                delta = pbc["y_m"] - be_pt["y_m"]
                delta_str = f"{delta:>+8.3f}"
            else:
                delta_str = f"{'N/A':>8}"

            print(
                f"  {r:>6}  {pbc_str:>13}  {pbc_v:>9}  "
                f"{be_str:>12}  {be_v:>9}  {delta_str:>8}"
            )

        print()

    print(sepa)
    print("  References:")
    print("    py-ballisticcalc  v2.2.10  — 3DoF point-mass, RK4 integration")
    print("    ballistics-engine v0.25.0  — Rust kernel, Python bindings")
    print()
    print("  Next: wire Rust ABE extension output for three-way comparison")


if __name__ == "__main__":
    main()
