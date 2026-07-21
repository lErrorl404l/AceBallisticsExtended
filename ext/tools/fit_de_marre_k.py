#!/usr/bin/env python3
"""
Fit De Marre K constants from ARL/BRL reference V50 data.

Solves  K = V50_ref × sqrt(M) / (D^0.75 × T_eff^0.7)

for each reference data point, then aggregates by projectile type
to produce data-driven K constants that minimize V50 prediction error.

Usage:
    python ext/tools/fit_de_marre_k.py

Outputs fitted K values, error reduction from current hand-picked K,
and Rust code snippet to update penetration.rs.
"""

import json
import math
import sys
from pathlib import Path
from collections import defaultdict

REPO_ROOT = Path(__file__).resolve().parent.parent.parent

# ── Current hand-picked K constants (from penetration.rs lines 453-468) ──
CURRENT_K = {
    "ball": 91000.0,
    "ap": 70000.0,
    "apfsds": 50500.0,
    "apcr": 60700.0,
    "heat": 100000.0,
    "soft_point": 95000.0,
}

# ── Classify projectile type from core/jacket material ──
BALL_CORES = {"lead", "lead_antimony", "lead_steel", "cupronickel"}
AP_CORES = {"hardened_steel", "steel_lead", "tungsten_carbide"}


def classify_type(core_material: str, name: str = "") -> str:
    """Classify projectile into ball/AP/other based on core material."""
    cm = core_material.lower().strip()
    if cm in BALL_CORES:
        return "ball"
    if cm in AP_CORES:
        return "ap"
    # Fallback: check name for AP indicators
    nl = name.lower()
    if "ap" in nl or "api" in nl:
        return "ap"
    return "ball"  # default to ball


def solve_k(
    v50: float,
    mass_g: float,
    caliber_mm: float,
    thickness_mm: float,
    obliquity_deg: float = 0.0,
) -> float:
    """
    Solve De Marre formula for K:

        V50 = K × D^0.75 × T_eff^0.7 / sqrt(M)

    =>  K = V50 × sqrt(M) / (D^0.75 × T_eff^0.7)

    All inputs in mm/g/deg.
    """
    D = caliber_mm / 1000.0  # meters
    M = mass_g / 1000.0  # kg
    T = thickness_mm / 1000.0  # meters
    theta = math.radians(obliquity_deg)
    T_eff = T / math.cos(theta)  # line-of-sight thickness

    if D <= 0 or M <= 0 or T_eff <= 0:
        return float("nan")

    return v50 * math.sqrt(M) / (D**0.75 * T_eff**0.7)


def load_arl_data(path: Path) -> list[dict]:
    """Load ARL-TR-4632 reference data."""
    data = json.loads(path.read_text())
    points = []
    for tc in data["test_cases"]:
        core_mat = tc.get("core_material", "lead")
        points.append(
            {
                "id": tc["id"],
                "name": tc["threat"],
                "mass_g": tc["projectile_mass_g"],
                "caliber_mm": tc["caliber_mm"],
                "thickness_mm": tc["target"]["thickness_mm"],
                "obliquity_deg": tc["target"]["obliquity_deg"],
                "v50_ms": tc["v50_ms"],
                "v50_std": tc.get("v50_std_error_ms", 0),
                "core_material": core_mat,
                "source": "ARL-TR-4632",
            }
        )
    return points


def load_common_ammo_data(path: Path) -> list[dict]:
    """Load common ammo calibration reference data."""
    data = json.loads(path.read_text())
    points = []
    for tc in data["test_cases"]:
        core_mat = tc.get("core_material", "lead")
        ptype = classify_type(core_mat, tc.get("name", ""))
        for curve in tc.get("penetration_curves", []):
            if curve.get("target_material") != "steel_rha":
                continue
            for dp in curve.get("data_points", []):
                points.append(
                    {
                        "id": tc["id"],
                        "name": tc["name"],
                        "mass_g": tc["projectile_mass_g"],
                        "caliber_mm": tc["caliber_mm"],
                        "thickness_mm": dp["thickness_mm"],
                        "obliquity_deg": curve["obliquity_deg"],
                        "v50_ms": dp["v50_ms"],
                        "v50_std": 0,
                        "core_material": core_mat,
                        "source": tc.get("reference", "common"),
                    }
                )
    return points


def main():
    arl_path = (
        REPO_ROOT
        / "data"
        / "calibration"
        / "test_cases"
        / "arl_bullet_penetration.json"
    )
    common_path = (
        REPO_ROOT
        / "data"
        / "calibration"
        / "test_cases"
        / "common_ammo_calibration.json"
    )

    if not arl_path.exists():
        print(f"ERROR: {arl_path} not found", file=sys.stderr)
        sys.exit(1)
    if not common_path.exists():
        print(f"ERROR: {common_path} not found", file=sys.stderr)
        sys.exit(1)

    arl_points = load_arl_data(arl_path)
    common_points = load_common_ammo_data(common_path)

    all_points = arl_points + common_points
    print(
        f"Loaded {len(arl_points)} ARL + {len(common_points)} common = "
        f"{len(all_points)} total reference points\n"
    )

    # ── Compute K per point ──
    by_type = defaultdict(list)
    for pt in all_points:
        ptype = classify_type(pt["core_material"], pt.get("name", ""))
        k = solve_k(
            pt["v50_ms"],
            pt["mass_g"],
            pt["caliber_mm"],
            pt["thickness_mm"],
            pt["obliquity_deg"],
        )
        if math.isnan(k) or k <= 0:
            continue
        by_type[ptype].append(
            {
                **pt,
                "k_fitted": k,
                "k_current": CURRENT_K.get(ptype, 91000),
                "v50_pred_current": (
                    CURRENT_K.get(ptype, 91000)
                    * (pt["caliber_mm"] / 1000) ** 0.75
                    * (
                        pt["thickness_mm"]
                        / 1000
                        / max(math.cos(math.radians(pt["obliquity_deg"])), 0.01)
                    )
                    ** 0.7
                    / math.sqrt(pt["mass_g"] / 1000)
                ),
            }
        )

    # ── Sub-classification for refined fit ──
    # Ball: separate sub-9mm (5.56, 7.62) from heavy (9mm, .50 BMG)
    # AP: separate WC core (M995) from steel core
    SUB_CLASS = {
        "ball_small": lambda p: (
            p["caliber_mm"] < 9.0 and classify_type(p["core_material"]) == "ball"
        ),
        "ball_heavy": lambda p: (
            p["caliber_mm"] >= 9.0 and classify_type(p["core_material"]) == "ball"
        ),
        "ap_steel": lambda p: (
            p["core_material"] == "hardened_steel"
            or (
                classify_type(p["core_material"]) == "ap"
                and "tungsten" not in p["core_material"]
            )
        ),
        "ap_wc": lambda p: "tungsten" in p["core_material"],
    }
    by_sub = defaultdict(list)
    for pt in all_points:
        k = solve_k(
            pt["v50_ms"],
            pt["mass_g"],
            pt["caliber_mm"],
            pt["thickness_mm"],
            pt["obliquity_deg"],
        )
        if math.isnan(k) or k <= 0:
            continue
        for sub_name, pred in SUB_CLASS.items():
            if pred(pt):
                by_sub[sub_name].append({**pt, "k_fitted": k})
                break  # first match only

    # ── Report per type ──
    print(
        f"{'Type':<12} {'Points':>6} {'Current K':>10} {'Fitted K':>10} "
        f"{'StdDev':>8} {'RMSE(curr)':>10} {'RMSE(fit)':>10} {'Improv':>8}"
    )
    print("-" * 80)

    for ptype in ["ball", "ap"]:
        pts = by_type.get(ptype, [])
        if not pts:
            continue
        k_values = [p["k_fitted"] for p in pts]
        fitted_k = sum(k_values) / len(k_values)
        std_k = math.sqrt(sum((k - fitted_k) ** 2 for k in k_values) / len(k_values))
        current_k = CURRENT_K.get(ptype, 91000)

        rmse_current = math.sqrt(
            sum((p["v50_ms"] - p["v50_pred_current"]) ** 2 for p in pts) / len(pts)
        )

        rmse_fitted = math.sqrt(
            sum(
                (
                    p["v50_ms"]
                    - (
                        fitted_k
                        * (p["caliber_mm"] / 1000) ** 0.75
                        * (
                            p["thickness_mm"]
                            / 1000
                            / max(math.cos(math.radians(p["obliquity_deg"])), 0.01)
                        )
                        ** 0.7
                        / math.sqrt(p["mass_g"] / 1000)
                    )
                )
                ** 2
                for p in pts
            )
            / len(pts)
        )

        improvement = (1 - rmse_fitted / rmse_current) * 100 if rmse_current > 0 else 0

        print(
            f"{ptype:<12} {len(pts):>6} {current_k:>10.0f} {fitted_k:>10.0f} "
            f"{std_k:>8.0f} {rmse_current:>10.0f} {rmse_fitted:>10.0f} "
            f"{improvement:>+7.1f}%"
        )

    print()

    # ── Detailed per-point report ──
    for ptype in ["ball", "ap"]:
        pts = by_type.get(ptype, [])
        if not pts:
            continue
        fitted_k = sum(p["k_fitted"] for p in pts) / len(pts)

        print(f"\n=== {ptype.upper()} — detailed ({len(pts)} points) ===")
        print(
            f"{'ID':<35} {'T(mm)':>6} {'Angle':>5} {'V50':>5} "
            f"{'K_fit':>8} {'K_curr':>8} {'V_curr':>7} {'V_fit':>7} {'Err_c%':>7}"
        )
        print("-" * 95)
        for p in sorted(pts, key=lambda x: x["id"]):
            fitted_v50 = (
                fitted_k
                * (p["caliber_mm"] / 1000) ** 0.75
                * (
                    p["thickness_mm"]
                    / 1000
                    / max(math.cos(math.radians(p["obliquity_deg"])), 0.01)
                )
                ** 0.7
                / math.sqrt(p["mass_g"] / 1000)
            )
            err_current = (p["v50_pred_current"] - p["v50_ms"]) / p["v50_ms"] * 100
            err_fitted = (fitted_v50 - p["v50_ms"]) / p["v50_ms"] * 100
            short_id = p["id"][:34]
            print(
                f"{short_id:<35} {p['thickness_mm']:>6.1f} "
                f"{p['obliquity_deg']:>5.0f} {p['v50_ms']:>5.0f} "
                f"{p['k_fitted']:>8.0f} {p['k_current']:>8.0f} "
                f"{p['v50_pred_current']:>7.0f} {fitted_v50:>7.0f} "
                f"{err_current:>+6.1f}%"
            )

    # ── Generate Rust code snippet ──
    print("\n\n=== Rust code snippet for penetration.rs ===")
    for ptype in ["ball", "ap"]:
        pts = by_type.get(ptype, [])
        if not pts:
            continue
        fitted_k = sum(p["k_fitted"] for p in pts) / len(pts)
        current_k = CURRENT_K.get(ptype, 91000)
        rmse_current = math.sqrt(
            sum((p["v50_ms"] - p["v50_pred_current"]) ** 2 for p in pts) / len(pts)
        )
        rmse_fitted = math.sqrt(
            sum(
                (
                    p["v50_ms"]
                    - (
                        fitted_k
                        * (p["caliber_mm"] / 1000) ** 0.75
                        * (
                            p["thickness_mm"]
                            / 1000
                            / max(math.cos(math.radians(p["obliquity_deg"])), 0.01)
                        )
                        ** 0.7
                        / math.sqrt(p["mass_g"] / 1000)
                    )
                )
                ** 2
                for p in pts
            )
            / len(pts)
        )

        print(
            f"\n// {ptype.upper()}: K = {fitted_k:.0f} "
            f"(was {current_k:.0f}, "
            f"RMSE {rmse_current:.0f}→{rmse_fitted:.0f} m/s "
            f"({(1 - rmse_fitted / rmse_current) * 100:.0f}% improvement)"
        )
        print(
            f"// Fitted from {len(pts)} reference data points "
            f"(ARL-TR-4632 + common ammo calibration)"
        )
        print(f"ProjectileType::{ptype} => {fitted_k:.0f},")

    # ── Summary ──
    print("\n\n=== Summary ===")
    for ptype in ["ball", "ap"]:
        pts = by_type.get(ptype, [])
        if not pts:
            continue
        fitted_k = sum(p["k_fitted"] for p in pts) / len(pts)
        k_values = [p["k_fitted"] for p in pts]
        std_k = math.sqrt(sum((k - fitted_k) ** 2 for k in k_values) / len(k_values))
        print(f"\n{ptype}:")
        print(f"  Current K: {CURRENT_K.get(ptype, 0):.0f}")
        print(f"  Fitted K:  {fitted_k:.0f} ± {std_k:.0f}")
        print(f"  Data pts:  {len(pts)}")
        print(f"  Reduction: {(1 - fitted_k / CURRENT_K.get(ptype, 1)) * 100:.1f}%")


if __name__ == "__main__":
    main()
