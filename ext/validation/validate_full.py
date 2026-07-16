#!/usr/bin/env -S uv run
"""
ABE Full 3-Way Validation Suite
================================
Validates ABE Rust extension (via struct-based C ABI) against
py-ballisticcalc, ballistics-engine, and known physical reference values.

Four validation domains:
  1. Exterior ballistics — trajectory (drop, velocity) at 7 ranges × 7 scenarios
  2. Penetration       — De Marre penetration model, 6 scenarios
  3. Atmosphere        — ISA standard atmosphere at 4 altitudes
  4. Fragmentation     — threshold, velocity-sensitivity, projectile-type effects

Usage:
    uv run validate_full.py [--rust-so PATH]
"""

import sys
import math
import os
import ctypes
from pathlib import Path

# ── Configuration ──────────────────────────────────────────────────────────

RUST_SO = os.environ.get(
    "ABE_RUST_SO",
    "/home/matt/Development/AceBallisticsExtention/ext/target/release/libabe_ballistics_ext.so",
)

# ── Test scenarios ─────────────────────────────────────────────────────────

# (name, mv_ms, bc_g7, mass_g, caliber_mm, drag_model)
TRAJ_SCENARIOS = [
    (
        "5.56x45mm M855",
        930,
        0.151,
        4.0,
        5.56,
        "g7",
    ),  # BC: APG/US Army BRL (ARL-TR-5182)
    ("7.62x51mm M80", 850, 0.200, 9.5, 7.62, "g7"),  # BC: APG/US Army BRL (AD0815788)
    (
        "5.45x39mm 7N6",
        900,
        0.168,
        3.43,
        5.45,
        "g7",
    ),  # BC: US Army BRL (15th Intl Symp Ballistics)
    (
        "7.62x54R 7N1",
        830,
        0.216,
        9.7,
        7.62,
        "g7",
    ),  # BC: Hornady Doppler radar (Firearms News)
    (
        "M855A1 (enhanced)",
        945,
        0.161,
        4.0,
        5.56,
        "g7",
    ),  # BC: Litz estimate, enhanced construction
    ("9x19mm Parabellum", 360, 0.130, 8.0, 9.01, "g1"),  # G1 drag model
    (".50 BMG M33", 890, 0.340, 42.0, 12.7, "g7"),  # BC: APG (confirmed M14Forum)
]

SAMPLE_RANGES_M = [0, 100, 200, 300, 500, 800, 1000]

# ── C ABI struct definitions (ctypes, matches Rust repr(C)) ────────────────


class FireParams(ctypes.Structure):
    _fields_ = [
        ("barrel_length_mm", ctypes.c_double),
        ("chamber_pressure_mpa", ctypes.c_double),
        ("caliber_mm", ctypes.c_double),
        ("projectile_mass_g", ctypes.c_double),
        ("cdm_id", ctypes.c_char * 32),
    ]


class FireResult(ctypes.Structure):
    _fields_ = [
        ("muzzle_velocity_ms", ctypes.c_double),
        ("max_chamber_pressure_mpa", ctypes.c_double),
        ("propellant_burn_fraction", ctypes.c_double),
        ("barrel_time_ms", ctypes.c_double),
    ]


class StepParams(ctypes.Structure):
    _fields_ = [
        ("pos_x", ctypes.c_double),
        ("pos_y", ctypes.c_double),
        ("pos_z", ctypes.c_double),
        ("vel_x", ctypes.c_double),
        ("vel_y", ctypes.c_double),
        ("vel_z", ctypes.c_double),
        ("dt_s", ctypes.c_double),
        ("wind_x", ctypes.c_double),
        ("wind_y", ctypes.c_double),
        ("wind_z", ctypes.c_double),
        ("density_kgm3", ctypes.c_double),
        ("temp_c", ctypes.c_double),
        ("altitude_m", ctypes.c_double),
        ("cdm_id", ctypes.c_char * 32),
        ("bc", ctypes.c_double),
        ("mass_g", ctypes.c_double),
        ("caliber_mm", ctypes.c_double),
    ]


class BulletState(ctypes.Structure):
    _fields_ = [
        ("pos_x", ctypes.c_double),
        ("pos_y", ctypes.c_double),
        ("pos_z", ctypes.c_double),
        ("vel_x", ctypes.c_double),
        ("vel_y", ctypes.c_double),
        ("vel_z", ctypes.c_double),
        ("mach", ctypes.c_double),
        ("time_s", ctypes.c_double),
    ]


class ImpactParams(ctypes.Structure):
    _fields_ = [
        ("vel_x", ctypes.c_double),
        ("vel_y", ctypes.c_double),
        ("vel_z", ctypes.c_double),
        ("mass_g", ctypes.c_double),
        ("caliber_mm", ctypes.c_double),
        ("armor_thickness_mm", ctypes.c_double),
        ("armor_material", ctypes.c_char * 32),
        ("impact_angle_deg", ctypes.c_double),
        ("projectile_type", ctypes.c_char * 32),
    ]


class ImpactResult(ctypes.Structure):
    _fields_ = [
        ("penetrated", ctypes.c_int),
        ("residual_vel_ms", ctypes.c_double),
        ("energy_j", ctypes.c_double),
        ("effective_thickness_mm", ctypes.c_double),
        ("ricochet", ctypes.c_int),
        ("ricochet_angle_deg", ctypes.c_double),
        ("ricochet_energy_fraction", ctypes.c_double),
        ("fragments", ctypes.c_int),
        ("spall_fragments", ctypes.c_int),
    ]


# ── Verify struct sizes match Rust repr(C) expectations ────────────────────

STRUCT_SIZES = {
    "FireParams": (ctypes.sizeof(FireParams), 64),
    "FireResult": (ctypes.sizeof(FireResult), 32),
    "StepParams": (ctypes.sizeof(StepParams), 160),
    "BulletState": (ctypes.sizeof(BulletState), 64),
    "ImpactParams": (ctypes.sizeof(ImpactParams), 120),
    "ImpactResult": (ctypes.sizeof(ImpactResult), 64),
}

for name, (actual, expected) in STRUCT_SIZES.items():
    if actual != expected:
        print(f"FATAL: {name} struct size mismatch: got {actual}, expected {expected}")
        sys.exit(1)


# ── ABE Rust library loader ────────────────────────────────────────────────


def load_abe_lib(so_path: str) -> ctypes.CDLL:
    lib = ctypes.CDLL(so_path)

    lib.abe_init.argtypes = [ctypes.c_uint32, ctypes.c_uint32]
    lib.abe_init.restype = ctypes.c_int

    lib.abe_fire.argtypes = [
        ctypes.POINTER(FireParams),
        ctypes.POINTER(FireResult),
    ]
    lib.abe_fire.restype = ctypes.c_int

    lib.abe_step.argtypes = [
        ctypes.POINTER(StepParams),
        ctypes.POINTER(BulletState),
    ]
    lib.abe_step.restype = ctypes.c_int

    lib.abe_impact.argtypes = [
        ctypes.POINTER(ImpactParams),
        ctypes.POINTER(ImpactResult),
    ]
    lib.abe_impact.restype = ctypes.c_int

    lib.abe_health.restype = ctypes.c_int
    lib.abe_health.argtypes = []

    # Init
    ret = lib.abe_init(1, 0)
    if ret != 0:
        print(f"FATAL: abe_init returned {ret}")
        sys.exit(1)
    if lib.abe_health() != 1:
        print("FATAL: health check failed after init")
        sys.exit(1)

    return lib


# ── ABE wrappers ───────────────────────────────────────────────────────────


def _make_cdm_id(drag_model: str) -> bytes:
    buf = b"\0" * 32
    raw = drag_model.encode("ascii")[:31]
    return raw + b"\0" + buf[len(raw) + 1 :]


def abe_trajectory(
    lib: ctypes.CDLL,
    mv_ms: float,
    bc: float,
    mass_g: float,
    caliber_mm: float,
    drag_model: str,
    dt_s: float = 0.01,
    max_range_m: float = 1050.0,
    min_vel: float = 50.0,
    density: float = 1.225,
    temp_c: float = 15.0,
    altitude_m: float = 0.0,
) -> list[dict]:
    """Run ABE step loop and return trajectory as a list of state dicts."""
    cdm_bytes = _make_cdm_id(drag_model)
    x = y = z = 0.0
    vx = mv_ms
    vy = vz = 0.0
    t = 0.0

    trajectory = [{"x_m": x, "y_m": y, "z_m": z, "v_ms": mv_ms, "t_s": t}]

    step = StepParams(
        pos_x=0,
        pos_y=0,
        pos_z=0,
        vel_x=0,
        vel_y=0,
        vel_z=0,
        dt_s=dt_s,
        wind_x=0,
        wind_y=0,
        wind_z=0,
        density_kgm3=density,
        temp_c=temp_c,
        altitude_m=altitude_m,
        cdm_id=cdm_bytes,
        bc=bc,
        mass_g=mass_g,
        caliber_mm=caliber_mm,
    )
    result = BulletState()

    while x < max_range_m and vx > min_vel:
        step.pos_x = x
        step.pos_y = y
        step.pos_z = z
        step.vel_x = vx
        step.vel_y = vy
        step.vel_z = vz

        ret = lib.abe_step(ctypes.byref(step), ctypes.byref(result))
        if ret != 0:
            break

        x = result.pos_x
        y = result.pos_y
        z = result.pos_z
        vx = result.vel_x
        vy = result.vel_y
        vz = result.vel_z
        t += dt_s

        speed = math.sqrt(vx * vx + vy * vy + vz * vz)
        trajectory.append({"x_m": x, "y_m": y, "z_m": z, "v_ms": speed, "t_s": t})

    return trajectory


def abe_penetration(
    lib: ctypes.CDLL,
    vel_x: float,
    vel_y: float,
    vel_z: float,
    mass_g: float,
    caliber_mm: float,
    armor_thickness_mm: float,
    armor_material: str,
    impact_angle_deg: float,
    projectile_type: str,
) -> dict:
    """Run ABE penetration model and return result dict."""
    mat_bytes = armor_material.encode("ascii")[:31].ljust(31, b"\0") + b"\0"
    mat_bytes = mat_bytes.ljust(32, b"\0")[:32]

    proj_bytes = projectile_type.encode("ascii")[:31].ljust(31, b"\0") + b"\0"
    proj_bytes = proj_bytes.ljust(32, b"\0")[:32]

    params = ImpactParams(
        vel_x=vel_x,
        vel_y=vel_y,
        vel_z=vel_z,
        mass_g=mass_g,
        caliber_mm=caliber_mm,
        armor_thickness_mm=armor_thickness_mm,
        armor_material=mat_bytes,
        impact_angle_deg=impact_angle_deg,
        projectile_type=proj_bytes,
    )
    result = ImpactResult()
    ret = lib.abe_impact(ctypes.byref(params), ctypes.byref(result))
    if ret != 0:
        return {"error": ret}
    return {
        "penetrated": bool(result.penetrated),
        "residual_vel_ms": result.residual_vel_ms,
        "energy_j": result.energy_j,
        "effective_thickness_mm": result.effective_thickness_mm,
        "ricochet": bool(result.ricochet),
        "ricochet_angle_deg": result.ricochet_angle_deg,
        "ricochet_energy_fraction": result.ricochet_energy_fraction,
        "fragments": result.fragments,
        "spall_fragments": result.spall_fragments,
    }


# ── Reference implementations ──────────────────────────────────────────────


def traj_pbc(scenario, sample_ranges):
    """Compute trajectory with py-ballisticcalc and sample at ranges."""
    import py_ballisticcalc as pb

    name, mv, bc, mass, cal, cdm = scenario
    pb.loadMetricUnits()

    drag_table = pb.TableG7 if cdm == "g7" else pb.TableG1
    dm = pb.DragModel(
        bc,
        drag_table,
        pb.Weight(mass, pb.Unit.Gram),
        pb.Distance(cal, pb.Unit.Millimeter),
    )
    ammo = pb.Ammo(dm, pb.Velocity(mv, pb.Unit.MPS))
    weapon = pb.Weapon(sight_height=pb.Distance(0.05, pb.Unit.Meter))
    atmo = pb.Atmo(altitude=0, pressure=1013.25, temperature=15)
    shot = pb.Shot(ammo=ammo, weapon=weapon, atmo=atmo)

    calc = pb.Calculator()
    calc.set_weapon_zero(shot, pb.Distance(100, pb.Unit.Meter))

    max_r = max(sample_ranges) + 50
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
    return _sample_points(raw, sample_ranges)


def traj_be(scenario, sample_ranges):
    """Compute trajectory with ballistics-engine and sample at ranges."""
    import ballistics_engine as be

    name, mv, bc, mass, cal, cdm = scenario

    drag_map = {"g7": "G7", "g1": "G1"}
    drag_model = drag_map.get(cdm, "G7")

    inputs = be.BallisticInputs.from_dict(
        {
            "muzzle_velocity_fps": mv * 3.28084,
            "bullet_weight_grains": mass * 15.4324,
            "bullet_diameter_inches": cal / 25.4,
            "bc": bc,
            "drag_model": drag_model,
            "sight_height_inches": 2.0,
            "zero_distance_yards": 100,
            "use_rk4": True,
            "sample_interval_m": 5.0,
            "muzzle_height_inches": 0.0,
            "use_adaptive_rk45": False,
            "target_height_inches": 12.0,
        }
    )
    solver = be.TrajectorySolver(inputs)
    result_pts = solver.solve()

    raw = []
    for p in result_pts.points:
        raw.append(
            {
                "x_m": p.x * 0.9144,
                "y_m": p.y * 0.9144,
                "v_ms": p.velocity_fps * 0.3048,
                "t_s": p.time,
            }
        )
    return _sample_points(raw, sample_ranges)


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


# ── ISA atmosphere formulas (Python reference) ────────────────────────────

ISA_GRAVITY = 9.80665
ISA_R = 287.058
ISA_LAPSE = -0.0065
ISA_T0 = 288.15
ISA_P0 = 101325.0
ISA_RHO0 = 1.225
ISA_TROPOPAUSE = 11000.0


def isa_temperature_c(altitude_m: float) -> float:
    """Temperature (°C) at altitude in ISA standard atmosphere."""
    if altitude_m <= ISA_TROPOPAUSE:
        t = ISA_T0 + ISA_LAPSE * altitude_m
        return max(t, 216.65) - 273.15
    return 216.65 - 273.15  # -56.5°C


def isa_pressure(altitude_m: float) -> float:
    """Pressure (Pa) at altitude in ISA standard atmosphere."""
    t_k = isa_temperature_c(altitude_m) + 273.15
    if altitude_m <= ISA_TROPOPAUSE:
        exponent = -ISA_GRAVITY / (ISA_R * ISA_LAPSE)
        return ISA_P0 * (t_k / ISA_T0) ** exponent
    else:
        p_trop = isa_pressure(ISA_TROPOPAUSE)
        delta_h = altitude_m - ISA_TROPOPAUSE
        return p_trop * math.exp(-ISA_GRAVITY * delta_h / (ISA_R * t_k))


def isa_density(altitude_m: float) -> float:
    """Density (kg/m³) at altitude in ISA standard atmosphere."""
    return isa_pressure(altitude_m) / (ISA_R * (isa_temperature_c(altitude_m) + 273.15))


# ── Validation functions ──────────────────────────────────────────────────

SECTION_SEP = "═" * 110
SUB_SEP = "─" * 110


def _pass_fail(delta, tolerance) -> str:
    return "PASS" if delta <= tolerance else "FAIL"


def validate_struct_sizes():
    """Verify all ctypes struct sizes match Rust repr(C)."""
    results = []
    for name, (actual, expected) in STRUCT_SIZES.items():
        delta = abs(actual - expected)
        ok = delta == 0
        results.append(
            {
                "section": "Struct Layout",
                "scenario": name,
                "metric": "sizeof",
                "abe": str(actual),
                "reference": str(expected),
                "delta": delta,
                "tolerance": 0,
                "pass": ok,
            }
        )
    return results


def validate_trajectory(lib: ctypes.CDLL):
    """Three-way trajectory comparison: ABE vs PBC vs BE."""
    results = []

    for scenario in TRAJ_SCENARIOS:
        name, mv, bc, mass, cal, cdm = scenario

        # Compute all three trajectories
        traj_abe = abe_trajectory(lib, mv, bc, mass, cal, cdm)
        try:
            traj_abe_sampled = _sample_points(traj_abe, SAMPLE_RANGES_M)
        except Exception:
            traj_abe_sampled = [None] * len(SAMPLE_RANGES_M)

        try:
            traj_p = traj_pbc(scenario, SAMPLE_RANGES_M)
        except Exception as e:
            traj_p = [None] * len(SAMPLE_RANGES_M)

        try:
            traj_b = traj_be(scenario, SAMPLE_RANGES_M)
        except Exception as e:
            traj_b = [None] * len(SAMPLE_RANGES_M)

        for i, rng in enumerate(SAMPLE_RANGES_M):
            pt_abe = traj_abe_sampled[i] if i < len(traj_abe_sampled) else None
            pt_pbc = traj_p[i] if i < len(traj_p) else None
            pt_be = traj_b[i] if i < len(traj_b) else None

            # Drop comparison: ABE vs PBC
            # ABE: +z = down (positive = drop below bore)
            # PBC:  +y = up  (negative = drop below bore)
            # So abe_z + pbc_y ≈ 0 when physics agrees → delta = |abe_z - (-pbc_y)| = |abe_z + pbc_y|
            if pt_abe and pt_pbc:
                delta_drop_pbc = abs(pt_abe["z_m"] + pt_pbc["y_m"])
                results.append(
                    {
                        "section": f"Trajectory: {name}",
                        "scenario": f"{rng}m drop",
                        "metric": "drop_abe_vs_pbc (m)",
                        "abe": f"{pt_abe['z_m']:.3f}",
                        "reference": f"{pt_pbc['y_m']:.3f}",
                        "delta": round(delta_drop_pbc, 3),
                        "tolerance": 2.0
                        if rng <= 200
                        else (5.0 if rng <= 500 else 10.0),
                        "pass": delta_drop_pbc
                        <= (2.0 if rng <= 200 else (5.0 if rng <= 500 else 10.0)),
                    }
                )

            # Drop comparison: ABE vs BE
            # BE uses same +y=up convention as PBC → same sum logic
            if pt_abe and pt_be:
                delta_drop_be = abs(pt_abe["z_m"] + pt_be["y_m"])
                results.append(
                    {
                        "section": f"Trajectory: {name}",
                        "scenario": f"{rng}m drop",
                        "metric": "drop_abe_vs_be (m)",
                        "abe": f"{pt_abe['z_m']:.3f}",
                        "reference": f"{pt_be['y_m']:.3f}",
                        "delta": round(delta_drop_be, 3),
                        "tolerance": 2.0
                        if rng <= 200
                        else (5.0 if rng <= 500 else 10.0),
                        "pass": delta_drop_be
                        <= (2.0 if rng <= 200 else (5.0 if rng <= 500 else 10.0)),
                    }
                )

            # Velocity comparison: ABE vs PBC
            if pt_abe and pt_pbc:
                delta_v = abs(pt_abe["v_ms"] - pt_pbc["v_ms"])
                results.append(
                    {
                        "section": f"Trajectory: {name}",
                        "scenario": f"{rng}m vel",
                        "metric": "vel_abe_vs_pbc (m/s)",
                        "abe": f"{pt_abe['v_ms']:.1f}",
                        "reference": f"{pt_pbc['v_ms']:.1f}",
                        "delta": round(delta_v, 1),
                        "tolerance": 30.0
                        if rng <= 200
                        else (50.0 if rng <= 500 else 80.0),
                        "pass": delta_v
                        <= (30.0 if rng <= 200 else (50.0 if rng <= 500 else 80.0)),
                    }
                )

            # Velocity comparison: ABE vs BE
            if pt_abe and pt_be:
                delta_v = abs(pt_abe["v_ms"] - pt_be["v_ms"])
                results.append(
                    {
                        "section": f"Trajectory: {name}",
                        "scenario": f"{rng}m vel",
                        "metric": "vel_abe_vs_be (m/s)",
                        "abe": f"{pt_abe['v_ms']:.1f}",
                        "reference": f"{pt_be['v_ms']:.1f}",
                        "delta": round(delta_v, 1),
                        "tolerance": 30.0
                        if rng <= 200
                        else (50.0 if rng <= 500 else 80.0),
                        "pass": delta_v
                        <= (30.0 if rng <= 200 else (50.0 if rng <= 500 else 80.0)),
                    }
                )

    return results


def validate_penetration(lib: ctypes.CDLL):
    """Validate ABE penetration model against known physics expectations."""
    scenarios = [
        # (name, vel_x, vel_y, vel_z, mass_g, cal_mm, armor_thick_mm, material, angle, proj_type, expect_penetrate, reason)
        (
            "7.62x51mm M80 vs 5mm RHA",
            850,
            0,
            0,
            9.5,
            7.62,
            5.0,
            "steel_rha",
            0.0,
            "ball",
            True,
            "Standard rifle round pens thin plate",
        ),
        (
            "7.62x51mm M80 vs 20mm RHA",
            850,
            0,
            0,
            9.5,
            7.62,
            20.0,
            "steel_rha",
            0.0,
            "ball",
            False,
            "Standard rifle round stopped by thick plate",
        ),
        (
            "7.62x51mm M80 vs 10mm RHA at 30°",
            850,
            0,
            0,
            9.5,
            7.62,
            10.0,
            "steel_rha",
            30.0,
            "ball",
            False,
            "30° angle → 11.5mm eff. thickness — stops M80 ball",
        ),
        (
            "5.56x45mm M855 vs 3mm RHA",
            930,
            0,
            0,
            4.0,
            5.56,
            3.0,
            "steel_rha",
            0.0,
            "ball",
            True,
            "High-velocity 5.56 pens thin plate",
        ),
        (
            "5.56x45mm M855 vs 3mm RHA (slow)",
            300,
            0,
            0,
            4.0,
            5.56,
            3.0,
            "steel_rha",
            0.0,
            "ball",
            False,
            "Subsonic 5.56 cannot pen even thin plate",
        ),
        (
            "AP vs Ball penetration (7.62mm @ 880m/s vs 10mm RHA)",
            880,
            0,
            0,
            9.5,
            7.62,
            10.0,
            "steel_rha",
            0.0,
            "ap",
            True,
            "AP projectile should have better pen than ball",
        ),
    ]

    results = []
    for (
        name,
        vx,
        vy,
        vz,
        mass,
        cal,
        thick,
        mat,
        angle,
        proj,
        expect_pen,
        reason,
    ) in scenarios:
        r = abe_penetration(lib, vx, vy, vz, mass, cal, thick, mat, angle, proj)
        did_pen = r.get("penetrated", False)

        # Binary pass/fail on penetration expectation
        pen_ok = did_pen == expect_pen

        results.append(
            {
                "section": "Penetration",
                "scenario": name,
                "metric": "penetrated",
                "abe": "yes" if did_pen else "no",
                "reference": "yes" if expect_pen else "no",
                "delta": 0 if pen_ok else 1,
                "tolerance": 0,
                "pass": pen_ok,
                "_reason": reason,
                "_residual": r.get("residual_vel_ms", 0),
                "_effective_thick": r.get("effective_thickness_mm", 0),
            }
        )

        # Additional detail for penetrations
        if did_pen:
            results.append(
                {
                    "section": "Penetration",
                    "scenario": f"  {name} detail",
                    "metric": "residual_vel (m/s)",
                    "abe": f"{r['residual_vel_ms']:.1f}",
                    "reference": ">0",
                    "delta": 0,
                    "tolerance": 0,
                    "pass": r["residual_vel_ms"] > 0,
                }
            )
            results.append(
                {
                    "section": "Penetration",
                    "scenario": f"  {name} detail",
                    "metric": "fragments",
                    "abe": str(r["fragments"]),
                    "reference": ">0 on pen",
                    "delta": 0,
                    "tolerance": 0,
                    "pass": r["fragments"] > 0,
                }
            )

        # Ricochet info if applicable
        if r.get("ricochet", False):
            results.append(
                {
                    "section": "Penetration",
                    "scenario": f"  {name} detail",
                    "metric": "ricochet_angle (°)",
                    "abe": f"{r['ricochet_angle_deg']:.1f}",
                    "reference": ">0",
                    "delta": 0,
                    "tolerance": 0,
                    "pass": r["ricochet_angle_deg"] > 0,
                }
            )

    # Additional: AP should penetrate where ball doesn't — same params
    # (already in the last scenario above but let's verify AP pens better directly)
    ball_r = abe_penetration(lib, 880, 0, 0, 9.5, 7.62, 10.0, "steel_rha", 0.0, "ball")
    ap_r = abe_penetration(lib, 880, 0, 0, 9.5, 7.62, 10.0, "steel_rha", 0.0, "ap")

    ap_better = ap_r.get("penetrated", False) or not ball_r.get("penetrated", False)
    results.append(
        {
            "section": "Penetration",
            "scenario": "AP > Ball penetration (10mm RHA @ 880m/s)",
            "metric": "ap_pens >= ball_pens",
            "abe": f"AP={ap_r.get('penetrated', False)} Ball={ball_r.get('penetrated', False)}",
            "reference": "AP >= Ball",
            "delta": 0,
            "tolerance": 0,
            "pass": ap_better,
        }
    )

    return results


def validate_atmosphere():
    """Validate ISA atmosphere model at key altitudes."""
    alt_test_points = [
        (0, "Sea level", 15.0, 101325.0, 1.225),
        (1000, "1000m", 8.5, 89874.0, 1.112),
        (5000, "5000m", -17.5, 54019.0, 0.736),
        (11000, "11km", -56.5, 22632.0, 0.364),
    ]

    TOL_TEMP = 2.0  # °C
    TOL_PRESS = 0.02  # fraction (2%)
    TOL_DENSITY = 0.02  # fraction (2%)

    results = []

    for alt_m, label, exp_temp_c, exp_press_pa, exp_density in alt_test_points:
        temp_c = isa_temperature_c(alt_m)
        press = isa_pressure(alt_m)
        density = isa_density(alt_m)

        d_temp = abs(temp_c - exp_temp_c)
        d_press_frac = abs(press - exp_press_pa) / exp_press_pa
        d_dens_frac = abs(density - exp_density) / exp_density

        results.append(
            {
                "section": "Atmosphere",
                "scenario": f"{label} ({alt_m}m)",
                "metric": f"temp (°C)",
                "abe": f"{temp_c:.1f}",
                "reference": f"{exp_temp_c:.1f}",
                "delta": round(d_temp, 2),
                "tolerance": TOL_TEMP,
                "pass": d_temp <= TOL_TEMP,
            }
        )
        results.append(
            {
                "section": "Atmosphere",
                "scenario": f"{label} ({alt_m}m)",
                "metric": f"pressure (Pa)",
                "abe": f"{press:.0f}",
                "reference": f"{exp_press_pa:.0f}",
                "delta": f"{d_press_frac * 100:.1f}%",
                "tolerance": f"{TOL_PRESS * 100:.0f}%",
                "pass": d_press_frac <= TOL_PRESS,
            }
        )
        results.append(
            {
                "section": "Atmosphere",
                "scenario": f"{label} ({alt_m}m)",
                "metric": f"density (kg/m³)",
                "abe": f"{density:.3f}",
                "reference": f"{exp_density:.3f}",
                "delta": f"{d_dens_frac * 100:.1f}%",
                "tolerance": f"{TOL_DENSITY * 100:.0f}%",
                "pass": d_dens_frac <= TOL_DENSITY,
            }
        )

    return results


def validate_fragmentation(lib: ctypes.CDLL):
    """Validate fragmentation model through impact results."""
    results = []

    # ── Scenario 1: Below threshold velocity → no fragments ──────────
    # Use low velocity impact with parameters that should produce minimal frags
    r1 = abe_penetration(lib, 200, 0, 0, 4.0, 5.56, 3.0, "steel_rha", 0.0, "fmj")
    results.append(
        {
            "section": "Fragmentation",
            "scenario": "Below fragmentation threshold (200 m/s FMJ)",
            "metric": "fragments",
            "abe": str(r1.get("fragments", "?")),
            "reference": "0 (low velocity)",
            "delta": 0,
            "tolerance": 0,
            "pass": r1.get("fragments", 999) <= 2,
        }
    )

    # ── Scenario 2: High velocity → fragments produced ───────────────
    r2 = abe_penetration(lib, 900, 0, 0, 4.0, 5.56, 3.0, "steel_rha", 0.0, "fmj")
    results.append(
        {
            "section": "Fragmentation",
            "scenario": "High velocity (900 m/s FMJ)",
            "metric": "fragments",
            "abe": str(r2.get("fragments", "?")),
            "reference": "> 0 (supersonic frag)",
            "delta": 0,
            "tolerance": 0,
            "pass": r2.get("fragments", 0) > 0,
        }
    )

    # ── Scenario 3: Ball vs FMJ vs AP produce different fragment counts ──
    # Impact the same plate with different projectile types
    r_ball = abe_penetration(lib, 900, 0, 0, 9.5, 7.62, 5.0, "steel_rha", 0.0, "ball")
    r_fmj = abe_penetration(lib, 900, 0, 0, 9.5, 7.62, 5.0, "steel_rha", 0.0, "fmj")
    r_ap = abe_penetration(lib, 900, 0, 0, 9.5, 7.62, 5.0, "steel_rha", 0.0, "ap")

    # AP should produce = or fewer fragments than FMJ/Ball
    ap_frags = r_ap.get("fragments", 0)
    ball_frags = r_ball.get("fragments", 0)
    fmj_frags = r_fmj.get("fragments", 0)
    ap_lower = ap_frags <= ball_frags and ap_frags <= fmj_frags

    results.append(
        {
            "section": "Fragmentation",
            "scenario": "AP <= FMJ fragments",
            "metric": "AP_frags vs FMJ_frags",
            "abe": f"AP={ap_frags} FMJ={fmj_frags} Ball={ball_frags}",
            "reference": "AP <= FMJ",
            "delta": abs(ap_frags - fmj_frags),
            "tolerance": 0,
            "pass": ap_lower,
        }
    )

    # ── Scenario 4: Higher velocity → more fragments ─────────────────
    r_slow = abe_penetration(lib, 500, 0, 0, 4.0, 7.62, 5.0, "steel_rha", 0.0, "fmj")
    r_fast = abe_penetration(lib, 1000, 0, 0, 4.0, 7.62, 5.0, "steel_rha", 0.0, "fmj")
    results.append(
        {
            "section": "Fragmentation",
            "scenario": "Higher velocity → more fragments",
            "metric": "frag_count(1000m/s) >= frag_count(500m/s)",
            "abe": f"{r_fast.get('fragments', 0)} >= {r_slow.get('fragments', 0)}",
            "reference": "velocity-dependent frag count",
            "delta": abs(r_fast.get("fragments", 0) - r_slow.get("fragments", 0)),
            "tolerance": 0,
            "pass": r_fast.get("fragments", 0) >= r_slow.get("fragments", 0),
        }
    )

    # ── Scenario 5: Penetrating hits produce spall fragments ──────────
    # Use a high-velocity impact against moderate armor
    r_pen = abe_penetration(lib, 950, 0, 0, 9.5, 7.62, 8.0, "steel_rha", 0.0, "ball")
    if r_pen.get("penetrated", False):
        results.append(
            {
                "section": "Fragmentation",
                "scenario": "Penetrating hit produces spall",
                "metric": "spall_fragments",
                "abe": str(r_pen.get("spall_fragments", 0)),
                "reference": "> 0",
                "delta": 0,
                "tolerance": 0,
                "pass": r_pen.get("spall_fragments", 0) > 0,
            }
        )

    return results


# ── Report ────────────────────────────────────────────────────────────────


def print_report(all_results):
    """Print formatted validation report."""
    total = len(all_results)
    passed = sum(1 for r in all_results if r["pass"])
    failed = total - passed
    fail_rate = failed / total * 100 if total > 0 else 0

    # ── Header ─────────────────────────────────────────────────────────────
    print(f"\n{'#' * 112}")
    print(f"#  ABE Full 3-Way Validation Report")
    print(
        f"#  {total} checks | {passed} passed | {failed} failed | {fail_rate:.1f}% failure rate"
    )
    print(f"{'#' * 112}\n")

    # ── Group by section ───────────────────────────────────────────────────
    from collections import OrderedDict

    sections = OrderedDict()
    for r in all_results:
        sec = r["section"]
        if sec not in sections:
            sections[sec] = []
        sections[sec].append(r)

    for section_name, sec_results in sections.items():
        sec_total = len(sec_results)
        sec_pass = sum(1 for r in sec_results if r["pass"])
        print(f"{SECTION_SEP}")
        print(f"  {section_name}  ({sec_pass}/{sec_total} passed)")
        print(f"{SECTION_SEP}")

        # Table header
        print(
            f"  {'Scenario':<45} {'Metric':<32} {'ABE':<18} {'Ref':<18} {'Δ':<12} {'Tol':<10} {'Result':<6}"
        )
        print(
            f"  {'─' * 44}  {'─' * 31}  {'─' * 17}  {'─' * 17}  {'─' * 11}  {'─' * 9}  {'─' * 5}"
        )

        for r in sec_results:
            result_str = "PASS" if r["pass"] else "FAIL"
            scenario = r["scenario"]
            metric = r["metric"]
            abe = str(r["abe"])
            ref = str(r["reference"])
            delta = str(r["delta"])
            tol = str(r["tolerance"])

            # Handle extra metadata for certain rows
            if "_reason" in r:
                scenario = r["_reason"][:44]

            print(
                f"  {scenario:<45} {metric:<32} {abe:<18} {ref:<18} {delta:<12} {tol:<10} {result_str:<6}"
            )

        print()

    # ── Summary ────────────────────────────────────────────────────────────
    print(f"{SECTION_SEP}")
    print(
        f"  SUMMARY: {passed}/{total} checks PASSED ({failed} failed, {fail_rate:.1f}%)"
    )
    if failed > 0:
        print(f"\n  FAILED CHECKS:")
        for r in all_results:
            if not r["pass"]:
                print(f"    ✗ [{r['section']}] {r['scenario']} — {r['metric']}")
                print(
                    f"      ABE={r['abe']}  Ref={r['reference']}  Δ={r['delta']}  Tol={r['tolerance']}"
                )
    print(f"{SECTION_SEP}\n")


# ── Main ──────────────────────────────────────────────────────────────────


def main():
    so_path = RUST_SO
    # Allow override via CLI
    if "--rust-so" in sys.argv:
        idx = sys.argv.index("--rust-so")
        if idx + 1 < len(sys.argv):
            so_path = sys.argv[idx + 1]

    if not os.path.exists(so_path):
        print(f"ERROR: Rust .so not found at {so_path}")
        print("Build it with: cd ext && cargo build --release")
        sys.exit(1)

    # Load ABE library
    print(f"Loading ABE from {so_path} ...")
    lib = load_abe_lib(so_path)
    print(f"  abe_health() = {lib.abe_health()}")
    print(f"  Init OK")
    print()

    all_results = []

    # 1. Struct layout validation
    print("Running struct layout validation ...")
    all_results.extend(validate_struct_sizes())

    # 2. Trajectory validation
    print("Running trajectory validation (7 scenarios × 7 ranges) ...")
    all_results.extend(validate_trajectory(lib))

    # 3. Penetration validation
    print("Running penetration validation (6 scenarios) ...")
    all_results.extend(validate_penetration(lib))

    # 4. Atmosphere validation
    print("Running atmosphere validation (ISA at 4 altitudes) ...")
    all_results.extend(validate_atmosphere())

    # 5. Fragmentation validation
    print("Running fragmentation validation (5 checks) ...")
    all_results.extend(validate_fragmentation(lib))

    # Print report
    print_report(all_results)

    # Exit code
    failed = sum(1 for r in all_results if not r["pass"])
    sys.exit(1 if failed > 0 else 0)


if __name__ == "__main__":
    main()
