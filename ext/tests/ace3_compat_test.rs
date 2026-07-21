// ABE — ACE3 3-Layer Override Compatibility Test
//
// Validates that the ABE Rust extension behaves correctly when configured
// for ACE3-enhanced mode (ace_present=1).  The 3-layer ACE3 override is
// entirely in SQF (fn_ace3_compat.sqf); the Rust extension stores the
// ace_present flag and all operations must work identically in both
// standalone and ACE3 mode.
//
// What this tests:
//   - init with ace_present=1 succeeds
//   - All commands (fire, step, impact, wound, zeroing, shooter, component)
//     produce valid results in ACE3 mode
//   - Fire→step→impact pipeline in ACE3 mode
//   - Mode switching (ace_present=1 ↔ 0)
//   - Multiple simultaneous bullets in ACE3 mode
//
// What this does NOT test (out of scope):
//   - The SQF compat logic in fn_ace3_compat.sqf (can't be tested from Rust)
//   - ACE3's native extension behavior
//   - ACE3 medical classification (tested in soft_tissue.rs)

use abe_ballistics_ext::{abe_health, abe_init, abe_version, RVExtension, RVExtensionArgs};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

const OUTPUT_BUF_SIZE: usize = 2048;

// ── Test helpers (identical to sqf_compat_test.rs) ────────────────────────

fn rv_ext(func: &str) -> String {
    let mut buf = vec![0u8; OUTPUT_BUF_SIZE];
    let cfunc = CString::new(func).unwrap();
    unsafe {
        RVExtension(
            buf.as_mut_ptr() as *mut c_char,
            OUTPUT_BUF_SIZE as i32,
            cfunc.as_ptr(),
        );
    }
    let end = buf.iter().position(|&b| b == 0).unwrap_or(0);
    std::str::from_utf8(&buf[..end]).unwrap().to_string()
}

fn rv_ext_args(func: &str, args: &[&str]) -> String {
    let mut buf = vec![0u8; OUTPUT_BUF_SIZE];
    let cfunc = CString::new(func).unwrap();
    let c_args: Vec<CString> = args.iter().map(|a| CString::new(*a).unwrap()).collect();
    let ptrs: Vec<*const c_char> = c_args.iter().map(|a| a.as_ptr()).collect();
    unsafe {
        RVExtensionArgs(
            buf.as_mut_ptr() as *mut c_char,
            OUTPUT_BUF_SIZE as i32,
            cfunc.as_ptr(),
            ptrs.as_ptr(),
            args.len() as i32,
        );
    }
    let end = buf.iter().position(|&b| b == 0).unwrap_or(0);
    std::str::from_utf8(&buf[..end]).unwrap().to_string()
}

fn parse_array(s: &str) -> Vec<&str> {
    s.trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .collect()
}

// ── Stateless: ACE3 mode init acceptance ──────────────────────────────────

#[test]
fn ace3_init_accepts_ace_present_flag() {
    // fnc_ace3_compat.sqf calls: _ext callExtension ["init", ["1", "1"]]
    let r = rv_ext_args("init", &["1", "1"]);
    assert_eq!(r, "0", "init with ace_present=1 should succeed");
}

#[test]
fn ace3_init_rejects_wrong_api_version() {
    let r = rv_ext_args("init", &["999", "1"]);
    assert_eq!(
        r, "-1",
        "init with wrong API version should fail even in ACE3 mode"
    );
}

#[test]
fn ace3_init_rejects_non_numeric_version() {
    let r = rv_ext_args("init", &["", "1"]);
    assert_eq!(r, "-1", "init with empty API version should fail");
}

// ── Lifecycle: health + init flow in ACE3 mode ──────────────────────────

#[test]
fn ace3_health_before_init_returns_0() {
    // Must only test if state hasn't been set by another test
    let h = rv_ext("health");
    if h == "1" {
        return; // Another test already initialized — skip
    }
    assert_eq!(h, "0", "health before init should be 0 in ACE3 context");
}

#[test]
fn ace3_init_then_health() {
    // SQF flow: init with [api_version=1, ace_present=1], then health
    let r = rv_ext_args("init", &["1", "1"]);
    assert_eq!(r, "0");
    let h = rv_ext("health");
    assert_eq!(h, "1", "health should be 1 after ACE3 mode init");
}

// ── All commands work in ACE3 mode ───────────────────────────────────────

#[test]
fn ace3_fire_works() {
    rv_ext_args("init", &["1", "1"]);

    // 5.56mm M855 from 368mm barrel
    let r = rv_ext_args("fire", &["368", "380", "5.56", "4.0", "g7"]);
    assert_ne!(r, "-1", "fire should succeed in ACE3 mode");
    let parts = parse_array(&r);
    assert_eq!(parts.len(), 4, "fire result: 4 fields");
    let mv: f64 = parts[0].parse().expect("MV numeric");
    assert!(mv > 600.0 && mv < 750.0, "M855 MV in [600,750]: {mv}");
    assert!(r.starts_with('['), "bracketed array");

    // 7.62mm M80 from 508mm barrel
    let r2 = rv_ext_args("fire", &["508", "380", "7.62", "9.5", "g7"]);
    assert_ne!(r2, "-1");
    let mv2: f64 = parse_array(&r2)[0].parse().expect("MV numeric");
    assert!(mv2 > 500.0 && mv2 < 750.0, "M80 MV in [500,750]: {mv2}");
}

#[test]
fn ace3_fire_success_format() {
    rv_ext_args("init", &["1", "1"]);

    // Result must be parseable by SQF's parseSimpleArray
    let r = rv_ext_args("fire", &["368", "380", "5.56", "4.0", "g7"]);
    assert!(r.starts_with('['), "fire: '[' prefix, got {r}");
    assert!(r.ends_with(']'), "fire: ']' suffix, got {r}");

    // No excess precision (trailing zeros stripped)
    let parts = parse_array(&r);
    for (i, p) in parts.iter().enumerate() {
        assert!(
            !p.contains("000000"),
            "field {i} has no excess precision: {p}"
        );
    }
}

#[test]
fn ace3_fire_edge_cases() {
    rv_ext_args("init", &["1", "1"]);

    // Empty args → fail
    assert_eq!(
        rv_ext_args("fire", &[]),
        "-1",
        "empty fire fails in ACE3 mode"
    );
    // Zero barrel length → fail
    assert_eq!(
        rv_ext_args("fire", &["0", "380", "5.56", "4.0", "g7"]),
        "-1"
    );
    // Negative pressure → fail
    assert_eq!(
        rv_ext_args("fire", &["368", "-10", "5.56", "4.0", "g7"]),
        "-1"
    );
    // Non-numeric → fail
    assert_eq!(
        rv_ext_args("fire", &["abc", "def", "xyz", "ghi", "g7"]),
        "-1"
    );
}

#[test]
fn ace3_longer_barrel_higher_mv() {
    rv_ext_args("init", &["1", "1"]);

    let short = rv_ext_args("fire", &["254", "380", "5.56", "4.0", "g7"]);
    let long = rv_ext_args("fire", &["508", "380", "5.56", "4.0", "g7"]);
    let mv_s: f64 = parse_array(&short)[0].parse().unwrap();
    let mv_l: f64 = parse_array(&long)[0].parse().unwrap();
    assert!(
        mv_l > mv_s,
        "longer barrel → higher MV in ACE3 mode: {mv_l} > {mv_s}"
    );
}

#[test]
fn ace3_step_works() {
    rv_ext_args("init", &["1", "1"]);

    let r = rv_ext_args(
        "step",
        &[
            "0", "0", "0", // pos
            "900", "0", "0",    // vel
            "0.01", // dt
            "0", "0", "0",     // wind
            "1.225", // density
            "15",    // temp
            "0",     // altitude
            "g7",    // cdm
            "0.157", // bc
            "4.0",   // mass_g
            "5.56",  // caliber_mm
        ],
    );
    assert_ne!(r, "-1", "step should succeed in ACE3 mode");
    let parts = parse_array(&r);
    assert_eq!(parts.len(), 8, "step result: 8 fields");
    assert!(r.starts_with('['), "bracketed array");
    assert!(
        parts[0].parse::<f64>().unwrap() > 0.0,
        "bullet moves forward"
    );
    assert!(
        parts[3].parse::<f64>().unwrap() < 900.0,
        "bullet slows down"
    );
    // Mach field returned
    assert!(
        parts[6].parse::<f64>().unwrap() > 0.0,
        "Mach number reported"
    );
}

#[test]
fn ace3_step_zero_dt() {
    rv_ext_args("init", &["1", "1"]);

    // dt=0 → position unchanged
    let r = rv_ext_args(
        "step",
        &[
            "0", "0", "0", "900", "0", "0", "0", "0", "0", "0", "1.225", "15", "0", "g7", "0.157",
            "4.0", "5.56",
        ],
    );
    assert!(
        parse_array(&r)[0].parse::<f64>().unwrap().abs() < 0.001,
        "dt=0 → pos_x ~0 in ACE3 mode"
    );
}

#[test]
fn ace3_impact_works() {
    rv_ext_args("init", &["1", "1"]);

    // 7.62mm at 900 m/s vs 5mm RHA at 0° → penetrates
    let r = rv_ext_args(
        "impact",
        &[
            "900",
            "0",
            "0",
            "9.5",
            "7.62",
            "5",
            "steel_rha",
            "0",
            "ball",
        ],
    );
    assert_ne!(r, "-1", "impact should succeed in ACE3 mode");
    let parts = parse_array(&r);
    assert_eq!(parts.len(), 9, "impact result: 9 fields");
    assert_eq!(
        parts[0].parse::<i32>().unwrap(),
        1,
        "7.62mm at 900 m/s pens 5mm RHA in ACE3 mode"
    );
    assert!(
        parts[1].parse::<f64>().unwrap() > 0.0,
        "residual velocity > 0"
    );
    assert_eq!(
        parts[4].parse::<i32>().unwrap(),
        0,
        "0° should not ricochet"
    );
    assert!(r.starts_with('['), "bracketed array");

    // All remaining fields finite
    for (i, p) in parts.iter().enumerate().skip(2) {
        assert!(
            p.parse::<f64>().map_or(false, f64::is_finite),
            "field {i} finite: {p}"
        );
    }
}

#[test]
fn ace3_impact_ricochet() {
    rv_ext_args("init", &["1", "1"]);

    // 85° → ricochet
    let r = rv_ext_args(
        "impact",
        &[
            "900",
            "0",
            "0",
            "9.5",
            "7.62",
            "10",
            "steel_rha",
            "85",
            "ball",
        ],
    );
    assert_ne!(r, "-1");
    assert_eq!(
        parse_array(&r)[4].parse::<i32>().unwrap(),
        1,
        "85° → ricochet in ACE3 mode"
    );
}

#[test]
fn ace3_impact_material_matrix() {
    rv_ext_args("init", &["1", "1"]);

    macro_rules! impact_mat {
        ($mat:expr) => {{
            let r = rv_ext_args(
                "impact",
                &["880", "0", "0", "9.5", "7.62", "20", $mat, "0", "ball"],
            );
            assert_ne!(r, "-1", "impact({}) should succeed in ACE3 mode", $mat);
            assert!(r.starts_with('['), "{}: '[' prefix", $mat);
            let p = parse_array(&r);
            assert_eq!(p.len(), 9, "{}: 9 fields", $mat);
            p.iter().map(|s| s.to_string()).collect::<Vec<_>>()
        }};
    }

    let rha = impact_mat!("steel_rha");
    let hha = impact_mat!("steel_hha");
    let al5083 = impact_mat!("aluminum_5083");
    let ceramic = impact_mat!("ceramic_al2o3");
    let wood = impact_mat!("wood");

    let rha_pen: i32 = rha[0].parse().unwrap();
    let hha_pen: i32 = hha[0].parse().unwrap();
    let al_pen: i32 = al5083[0].parse().unwrap();
    let ceramic_pen: i32 = ceramic[0].parse().unwrap();
    let wood_pen: i32 = wood[0].parse().unwrap();

    // All residual velocities >= 0
    for (name, p) in [
        ("rha", &rha),
        ("hha", &hha),
        ("al5083", &al5083),
        ("ceramic", &ceramic),
        ("wood", &wood),
    ] {
        assert!(
            p[1].parse::<f64>().unwrap() >= 0.0,
            "{name}: residual vel >= 0 in ACE3 mode"
        );
    }

    // Material ordering
    assert!(
        al_pen >= rha_pen,
        "Al5083 pens easier than RHA in ACE3 mode"
    );
    assert!(
        wood_pen >= rha_pen,
        "Wood pens easier than RHA in ACE3 mode"
    );
    assert!(
        hha_pen <= rha_pen,
        "HHA blocks better than RHA in ACE3 mode"
    );
    assert!(
        ceramic_pen <= rha_pen,
        "Ceramic blocks better than RHA in ACE3 mode"
    );

    // Absolute thresholds at 20mm/880m/s/7.62mm ball (K=56509 after ARL calibration)
    assert_eq!(al_pen, 1, "20mm Al5083 PENs in ACE3 mode");
    assert_eq!(wood_pen, 1, "20mm wood PENs in ACE3 mode");
    assert_eq!(rha_pen, 0, "20mm RHA does NOT PEN in ACE3 mode");
    assert_eq!(ceramic_pen, 0, "20mm ceramic does NOT PEN in ACE3 mode");
}

#[test]
fn ace3_wound_works() {
    rv_ext_args("init", &["1", "1"]);

    // 7.62mm at 850 m/s into soft tissue
    let r = rv_ext_args("wound", &["850", "0", "0", "9.5", "7.62", "ball"]);
    assert!(
        r.starts_with('['),
        "wound result should be array in ACE3 mode: {r}"
    );
    let parts: Vec<&str> = r
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .collect();
    assert_eq!(parts.len(), 5, "wound result: 5 fields");
    let pen_mm: f64 = parts[0].parse().expect("pen depth numeric");
    assert!(
        pen_mm > 50.0,
        "rifle round penetrates deeply in ACE3 mode: {pen_mm}mm"
    );
}

#[test]
fn ace3_zeroing_works() {
    rv_ext_args("init", &["1", "1"]);

    // 63 mm sight height, 100 m zero, 948 m/s MV
    let r = rv_ext_args("zeroing", &["63", "100", "948"]);
    assert!(
        r.starts_with('['),
        "zeroing result should be array in ACE3 mode: {r}"
    );
    let inner = r.trim_start_matches('[').trim_end_matches(']');
    let moa: f64 = inner.parse().expect("zeroing result should be numeric MOA");
    assert!(
        moa > 0.0 && moa < 20.0,
        "zero MOA in plausible range: {moa}"
    );
}

#[test]
fn ace3_shooter_works() {
    rv_ext_args("init", &["1", "1"]);

    let r = rv_ext_args(
        "shooter",
        &["2.0", "prone", "bipod", "72", "0", "advanced", "300"],
    );
    assert!(
        r.starts_with('['),
        "shooter result should be array in ACE3 mode: {r}"
    );
    let parts: Vec<&str> = r
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .collect();
    assert_eq!(parts.len(), 3, "shooter result: 3 fields");
    let moa: f64 = parts[0].parse().expect("first field MOA");
    assert!(
        moa > 0.0 && moa < 10.0,
        "shooter MOA plausible in ACE3 mode: {moa}"
    );
}

#[test]
fn ace3_component_works() {
    rv_ext_args("init", &["1", "1"]);

    // MBT front hit, 120mm APFSDS at 1600 m/s
    let r = rv_ext_args(
        "component",
        &[
            "mbt", "front", "120", "4600", "1600", "apfsds", "0", "900", "1",
        ],
    );
    assert!(
        r.starts_with('['),
        "component result array in ACE3 mode: {r}"
    );
    let parts: Vec<&str> = r
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .collect();
    assert_eq!(parts.len(), 3, "component result: 3 fields");
    let mob: f64 = parts[0].parse().expect("mobility kill numeric");
    assert!(mob >= 0.0 && mob <= 1.0, "mobility kill in [0,1]: {mob}");
    assert!(mob > 0.3, "APFSDS vs MBT front mobility kill > 0.3: {mob}");
}

// ── Pipeline: fire → step → impact in ACE3 mode ─────────────────────────

#[test]
fn ace3_pipeline() {
    rv_ext_args("init", &["1", "1"]);

    // Fire M855
    let fire = rv_ext_args("fire", &["368", "380", "5.56", "4.0", "g7"]);
    assert_ne!(fire, "-1");
    let mv: f64 = parse_array(&fire)[0].parse().unwrap();

    // Step × 100 (1 s flight)
    let (mut x, mut z) = (0.0_f64, 0.0_f64);
    let (mut vx, mut vz) = (mv, 0.0);

    for _ in 0..100 {
        let s = format!("{x},0,{z},{vx},0,{vz},0.01,0,0,0,1.225,15,0,g7,0.157,4.0,5.56");
        let args: Vec<&str> = s.split(',').collect();
        let r = rv_ext_args("step", &args);
        assert_ne!(r, "-1", "step in ACE3 pipeline should succeed");
        let p = parse_array(&r);
        x = p[0].parse().unwrap();
        z = p[2].parse().unwrap();
        vx = p[3].parse().unwrap();
        vz = p[5].parse().unwrap();
    }

    assert!(x > 400.0, "bullet travels in ACE3 mode: x={x:.1}");
    assert!(z > 0.0, "bullet drops in ACE3 mode: z={z:.1}");
    assert!(vx < mv, "bullet slows in ACE3 mode: {vx:.1} < {mv:.1}");

    // Impact against 3mm RHA — valid 9-field result
    let impact = rv_ext_args(
        "impact",
        &[
            &format!("{vx:.1}"),
            "0",
            &format!("{vz:.1}"),
            "4.0",
            "5.56",
            "3",
            "steel_rha",
            "0",
            "ball",
        ],
    );
    let ip = parse_array(&impact);
    assert_eq!(ip.len(), 9, "impact result: 9 fields in ACE3 pipeline");
    assert!(
        ip[1].parse::<f64>().unwrap() >= 0.0,
        "residual velocity >= 0"
    );
}

// ── Multi-bullet interleaving in ACE3 mode ──────────────────────────────
//
// SQF tracks multiple bullet states in a hashmap and calls step for each
// per frame.  The extension must produce deterministic results regardless
// of interleaving order.

#[test]
fn ace3_multi_bullet_interleaving() {
    rv_ext_args("init", &["1", "1"]);

    // Fire M855 (5.56mm) and M80 (7.62mm)
    let r_a = rv_ext_args("fire", &["368", "380", "5.56", "4.0", "g7"]);
    let r_b = rv_ext_args("fire", &["630", "380", "7.62", "9.5", "g7"]);
    assert_ne!(r_a, "-1");
    assert_ne!(r_b, "-1");

    let mv_a: f64 = parse_array(&r_a)[0].parse().unwrap();
    let mv_b: f64 = parse_array(&r_b)[0].parse().unwrap();
    assert!(
        (mv_a - mv_b).abs() > 10.0,
        "M855 and M80 have different MVs in ACE3 mode: A={mv_a}, B={mv_b}"
    );

    // Bullet A state
    let (mut ax, mut ay, mut az) = (0.0, 0.0, 0.0);
    let (mut avx, mut avy, mut avz) = (mv_a, 0.0, 0.0);

    // Bullet B state
    let (mut bx, mut by, mut bz) = (0.0, 0.0, 0.0);
    let (mut bvx, mut bvy, mut bvz) = (mv_b, 0.0, 0.0);

    // Step A 5 times, step B 5 times, then alternate 200 each
    let step_a = |x, y, z, vx, vy, vz| -> (f64, f64, f64, f64, f64, f64) {
        let s = format!("{x},{y},{z},{vx},{vy},{vz},0.01,0,0,0,1.225,15,0,g7,0.157,4.0,5.56");
        let args: Vec<&str> = s.split(',').collect();
        let r = rv_ext_args("step", &args);
        assert_ne!(r, "-1", "step A in ACE3 mode should succeed");
        let p = parse_array(&r);
        (
            p[0].parse().unwrap(),
            p[1].parse().unwrap(),
            p[2].parse().unwrap(),
            p[3].parse().unwrap(),
            p[4].parse().unwrap(),
            p[5].parse().unwrap(),
        )
    };

    let step_b = |x, y, z, vx, vy, vz| -> (f64, f64, f64, f64, f64, f64) {
        let s = format!("{x},{y},{z},{vx},{vy},{vz},0.01,0,0,0,1.225,15,0,g7,0.200,9.5,7.62");
        let args: Vec<&str> = s.split(',').collect();
        let r = rv_ext_args("step", &args);
        assert_ne!(r, "-1", "step B in ACE3 mode should succeed");
        let p = parse_array(&r);
        (
            p[0].parse().unwrap(),
            p[1].parse().unwrap(),
            p[2].parse().unwrap(),
            p[3].parse().unwrap(),
            p[4].parse().unwrap(),
            p[5].parse().unwrap(),
        )
    };

    for _ in 0..5 {
        (ax, ay, az, avx, avy, avz) = step_a(ax, ay, az, avx, avy, avz);
    }
    assert!(ax > 0.0, "Bullet A moved in ACE3 mode: {ax}");
    assert!(avx < mv_a, "Bullet A slowed in ACE3 mode: {avx} < {mv_a}");
    assert!(az > 0.0, "Bullet A dropped in ACE3 mode: {az}");

    for _ in 0..5 {
        (bx, by, bz, bvx, bvy, bvz) = step_b(bx, by, bz, bvx, bvy, bvz);
    }
    assert!(bx > 0.0, "Bullet B moved in ACE3 mode: {bx}");
    assert!(bvx < mv_b, "Bullet B slowed in ACE3 mode: {bvx} < {mv_b}");
    assert!(bz > 0.0, "Bullet B dropped in ACE3 mode: {bz}");

    // Alternate 200 each
    for _ in 0..200 {
        (ax, ay, az, avx, avy, avz) = step_a(ax, ay, az, avx, avy, avz);
        (bx, by, bz, bvx, bvy, bvz) = step_b(bx, by, bz, bvx, bvy, bvz);
    }

    // M855 (BC=0.157) has higher drag than M80 (BC=0.200) → M80 travels farther
    assert!(
        ax < bx,
        "M855 travels less than M80 in ACE3 mode: A.x={ax:.1} < B.x={bx:.1}"
    );

    // Physical plausibility
    assert!(avx > 0.0, "Bullet A forward velocity positive: {avx}");
    assert!(bvx > 0.0, "Bullet B forward velocity positive: {bvx}");
    assert!(az > 0.0, "Bullet A dropped: {az}");
    assert!(bz > 0.0, "Bullet B dropped: {bz}");

    // Stateless: interleaving does not corrupt A's trajectory
    let (mut ref_x, _, mut ref_z) = (0.0, 0.0, 0.0);
    let (mut ref_vx, _, mut ref_vz) = (mv_a, 0.0, 0.0);
    for _ in 0..205 {
        (ref_x, _, ref_z, ref_vx, _, ref_vz) = step_a(ref_x, 0.0, ref_z, ref_vx, 0.0, ref_vz);
    }
    assert!(
        (ax - ref_x).abs() < 0.01,
        "A's x matches reference (stateless) in ACE3 mode: {ax} vs {ref_x}"
    );
}

// ── Mode switching ──────────────────────────────────────────────────────
//
// The SQF compat layer can re-init with a different ace_present value
// (re-init is idempotent — OnceLock keeps first write).  These tests
// validate that the extension handles both values gracefully.

#[test]
fn ace3_mode_switch_standalone_to_ace3() {
    // Init as standalone first
    let r = rv_ext_args("init", &["1", "0"]);
    assert_eq!(r, "0", "standalone init succeeds");

    // Fire works in standalone
    let fire = rv_ext_args("fire", &["368", "380", "5.56", "4.0", "g7"]);
    assert_ne!(fire, "-1", "fire works in standalone");

    // Re-init as ACE3 mode (idempotent — initial state preserved)
    let r2 = rv_ext_args("init", &["1", "1"]);
    assert_eq!(r2, "0", "re-init to ACE3 mode succeeds");

    // All commands still work after re-init
    let fire2 = rv_ext_args("fire", &["508", "380", "7.62", "9.5", "g7"]);
    assert_ne!(fire2, "-1", "fire works after switching to ACE3 mode");
    assert!(fire2.starts_with('['));

    let step = rv_ext_args(
        "step",
        &[
            "0", "0", "0", "900", "0", "0", "0.01", "0", "0", "0", "1.225", "15", "0", "g7",
            "0.200", "9.5", "7.62",
        ],
    );
    assert_ne!(step, "-1", "step works after switching to ACE3 mode");

    let impact = rv_ext_args(
        "impact",
        &[
            "900",
            "0",
            "0",
            "9.5",
            "7.62",
            "5",
            "steel_rha",
            "0",
            "ball",
        ],
    );
    assert_ne!(impact, "-1", "impact works after switching to ACE3 mode");

    // Health still 1
    assert_eq!(rv_ext("health"), "1", "health stays 1 after mode switch");

    // Wound, zeroing, shooter, component still work
    let wound = rv_ext_args("wound", &["850", "0", "0", "9.5", "7.62", "ball"]);
    assert!(wound.starts_with('['), "wound works after mode switch");

    let zeroing = rv_ext_args("zeroing", &["63", "100", "948"]);
    assert!(zeroing.starts_with('['), "zeroing works after mode switch");

    let shooter = rv_ext_args(
        "shooter",
        &["2.0", "prone", "bipod", "72", "0", "advanced", "300"],
    );
    assert!(shooter.starts_with('['), "shooter works after mode switch");

    let component = rv_ext_args(
        "component",
        &[
            "mbt", "front", "120", "4600", "1600", "apfsds", "0", "900", "1",
        ],
    );
    assert!(
        component.starts_with('['),
        "component works after mode switch"
    );
}

// ── Long-duration stability in ACE3 mode ────────────────────────────────
//
// Guarantee no NaN/Inf creep after 10 000 integration steps

#[test]
fn ace3_long_duration_stability() {
    rv_ext_args("init", &["1", "1"]);

    let r = rv_ext_args("fire", &["368", "380", "5.56", "4.0", "g7"]);
    assert_ne!(r, "-1");
    let mv: f64 = parse_array(&r)[0].parse().unwrap();

    let (mut x, mut y, mut z) = (0.0, 0.0, 0.0);
    let (mut vx, mut vy, mut vz) = (mv, 0.0, 0.0);
    let mut bad_steps: u32 = 0;

    for step_idx in 0..10000 {
        let s = format!("{x},{y},{z},{vx},{vy},{vz},0.01,0,0,0,1.225,15,0,g7,0.157,4.0,5.56");
        let args: Vec<&str> = s.split(',').collect();
        let r = rv_ext_args("step", &args);

        if r == "-1" {
            bad_steps += 1;
            continue;
        }

        let p = parse_array(&r);
        assert_eq!(p.len(), 8, "step result: 8 fields in ACE3 mode");
        x = p[0].parse().unwrap();
        y = p[1].parse().unwrap();
        z = p[2].parse().unwrap();
        vx = p[3].parse().unwrap();
        vy = p[4].parse().unwrap();
        vz = p[5].parse().unwrap();

        if step_idx > 0 && step_idx % 1000 == 0 {
            for (j, val) in [x, y, z, vx, vy, vz].iter().enumerate() {
                assert!(
                    val.is_finite(),
                    "NaN/Inf at step {step_idx}, field {j}: {val} in ACE3 mode"
                );
            }
        }
    }

    assert_eq!(
        bad_steps, 0,
        "all 10000 steps returned success in ACE3 mode"
    );
    assert!(
        vx < 50.0,
        "forward velocity very low after 100s in ACE3 mode: {vx:.1}"
    );
    assert!(
        z > 1000.0,
        "bullet dropped significantly in ACE3 mode: z={z:.1}"
    );
    assert!(vz > 0.0, "bullet still falling in ACE3 mode: {vz:.1}");
    assert!(
        vz < 200.0,
        "vertical velocity bounded in ACE3 mode: {vz:.1}"
    );

    for v in [x, y, z, vx, vy, vz] {
        assert!(v.is_finite(), "all values finite at end of ACE3 run");
    }
}

// ── Struct C ABI with ACE3 mode ─────────────────────────────────────────
//
// The low-level C ABI entry points must also accept the ace_present flag.

#[test]
fn ace3_c_abi_init() {
    unsafe {
        let v = abe_version();
        assert_eq!(CStr::from_ptr(v).to_str().unwrap(), "0.1.0");

        // Wrong version → -1 regardless of ace_present
        assert_eq!(abe_init(999, 1), -1, "wrong API version fails in ACE3 mode");

        // Correct init with ace_present=1
        if abe_init(1, 1) == 0 {
            assert_eq!(abe_health(), 1, "health after ACE3 mode C ABI init");
        }
    }
}

// ── Error recovery in ACE3 mode ─────────────────────────────────────────
//
// After failed commands, valid commands must still work

#[test]
fn ace3_error_recovery() {
    rv_ext_args("init", &["1", "1"]);

    // Fire with bad args
    assert_eq!(
        rv_ext_args("fire", &[]),
        "-1",
        "empty fire → -1 in ACE3 mode"
    );
    assert_eq!(
        rv_ext_args("fire", &["abc", "def"]),
        "-1",
        "non-numeric fire → -1 in ACE3 mode"
    );

    // Valid fire after errors
    let r = rv_ext_args("fire", &["368", "380", "5.56", "4.0", "g7"]);
    assert_ne!(r, "-1", "valid fire recovers in ACE3 mode");
    assert!(r.starts_with('['));

    // Valid step after errors
    let s = rv_ext_args(
        "step",
        &[
            "0", "0", "0", "900", "0", "0", "0.01", "0", "0", "0", "1.225", "15", "0", "g7",
            "0.157", "4.0", "5.56",
        ],
    );
    assert_ne!(s, "-1", "valid step recovers in ACE3 mode");

    // Valid impact after errors
    let i = rv_ext_args(
        "impact",
        &[
            "880",
            "0",
            "0",
            "9.5",
            "7.62",
            "5",
            "steel_rha",
            "0",
            "ball",
        ],
    );
    assert_ne!(i, "-1", "valid impact recovers in ACE3 mode");

    // State integrity
    assert_eq!(
        rv_ext("health"),
        "1",
        "state not corrupted by errors in ACE3 mode"
    );
}

// ── Unknown command handling in ACE3 mode ───────────────────────────────

#[test]
fn ace3_unknown_string_command() {
    rv_ext_args("init", &["1", "1"]);
    assert_eq!(rv_ext("bogus"), "unknown: bogus");
}

#[test]
fn ace3_unknown_args_command() {
    rv_ext_args("init", &["1", "1"]);
    assert_eq!(rv_ext_args("nonsense", &["a", "b"]), "unknown: nonsense");
}
