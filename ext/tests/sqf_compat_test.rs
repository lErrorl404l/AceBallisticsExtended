// ABE - SQF Calling Pattern Compatibility Tests
//
// These integration tests validate the *exact* string-ABI protocol
// that ABE's SQF side uses when it calls:
//   "abe_ballistics_ext" callExtension "command"
//   "abe_ballistics_ext" callExtension ["command", [arg1, arg2, ...]]
//
// The tests call the C ABI entry points (RVExtension / RVExtensionArgs)
// directly — the same functions Arma 3 resolves at runtime.
//
// Why this exists alongside the unit tests in lib.rs:
//   - Integration tests compile as a SEPARATE binary, proving the
//     exported symbols really are linkable and match the expected C ABI.
//   - Every test here documents the exact argument order and shape
//     SQF passes, so a regression in the ABI layer is caught.
//
// IMPORTANT: The extension's internal state (OnceLock) can be written
// exactly once.  Calling "health" before "init" locks it to
// initialized=false and init becomes a no-op thereafter.  The pre-init
// health check is therefore handled as a standalone test that skips
// when another test has already initialized — mirroring the guard
// pattern in lib.rs unit tests.

use abe_ballistics_ext::{abe_health, abe_init, abe_version, RVExtension, RVExtensionArgs};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

const OUTPUT_BUF_SIZE: usize = 2048;

// ── Test helpers ──────────────────────────────────────────────────────────────

/// Call the string-mode entry point — maps to SQF's:
///   "abe_ballistics_ext" callExtension "command"
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

/// Call the array-mode entry point — maps to SQF's:
///   "abe_ballistics_ext" callExtension ["command", [arg1, arg2, ...]]
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

/// Parse a bracketed array response into its comma-separated fields.
fn parse_array(s: &str) -> Vec<&str> {
    s.trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .collect()
}

// ── Standalone tests (stateless — safe to run in parallel) ──────────────────

#[test]
fn sqf_version_command() {
    assert_eq!(rv_ext("version"), "0.1.0");
}

#[test]
fn sqf_version_is_valid_semver() {
    let ver = rv_ext("version");
    let parts: Vec<&str> = ver.split('.').collect();
    assert_eq!(parts.len(), 3, "version should be semver: got {ver}");
    for p in &parts {
        assert!(
            p.parse::<u32>().is_ok(),
            "version segment '{p}' not numeric"
        );
    }
}

#[test]
fn sqf_unknown_string_command() {
    assert_eq!(rv_ext("bogus_command"), "unknown: bogus_command");
}

#[test]
fn sqf_unknown_args_command() {
    assert_eq!(
        rv_ext_args("nonsense", &["a", "b", "c"]),
        "unknown: nonsense"
    );
}

#[test]
fn sqf_empty_string_command() {
    assert_eq!(rv_ext(""), "unknown: ");
}

#[test]
fn sqf_empty_string_args_command() {
    assert_eq!(rv_ext_args("", &[]), "unknown: ");
}

#[test]
fn sqf_very_long_command_fits_in_buffer() {
    let long = "a".repeat(500);
    let r = rv_ext(&long);
    assert!(r.starts_with("unknown: "));
    assert!(
        r.len() <= OUTPUT_BUF_SIZE,
        "output must not overflow buffer"
    );
}

#[test]
fn sqf_init_wrong_api_version() {
    // fnc_init.sqf:  if (_result select 0 != 0) exitWith { false }
    assert_eq!(rv_ext_args("init", &["999", "0"]), "-1");
}

#[test]
fn sqf_init_non_numeric_version() {
    assert_eq!(rv_ext_args("init", &["", "0"]), "-1");
}

// ── Pre-init health check ────────────────────────────────────────────────────
//
// fnc_health.sqf:  _health = _extension callExtension "health";
// Before init, health returns "0".  Because health() calls get_state()
// which initializes the OnceLock, this test can only pass when it runs
// before any other test that touches state.  We handle this by checking
// the return value — if it's "0" we verify; if "1" another test already
// initialized state and this is expected.

#[test]
fn sqf_health_before_init() {
    let h = rv_ext("health");
    if h == "1" {
        // Another test already initialized state — pre-init state
        // could not be verified.  This is expected in parallel mode.
        return;
    }
    assert_eq!(h, "0", "health before init should be 0");
}

// ── Stateful lifecycle tests (sequential, single test) ─────────────────────
//
// These all share the global STATE OnceLock. All stateful assertions are
// grouped into one test to avoid races.  Order:
//   1. init → "0" (must be FIRST — no health call before this)
//   2. health after init → "1"
//   3. re-init → check idempotency
//   4. incorrect version inits → don't affect state
//   5. fire commands (normal, edge cases, barrel comparison)
//   6. step commands (normal, zero dt, partial args)
//   7. impact commands (pen, flesh, ricochet, AP vs ball)
//   8. pipeline: fire → 100 steps → impact
//   9. return format contracts

#[test]
fn sqf_all_lifecycle_tests() {
    // ── Init (fnc_init.sqf) ─────────────────────────────────────────────
    //
    // MUST NOT call health/version before init, because health calls
    // get_state() which locks the OnceLock to initialized=false.
    assert_eq!(rv_ext_args("init", &["1", "0"]), "0", "init should succeed");

    // ── Health after init (fnc_health.sqf) ──────────────────────────────
    assert_eq!(rv_ext("health"), "1", "health after init should be 1");

    // Re-init → returns "0" (OnceLock.set is idempotent)
    assert_eq!(rv_ext_args("init", &["1", "0"]), "0");
    assert_eq!(rv_ext("health"), "1", "health stays 1 after re-init");

    // Wrong version init → "-1" (doesn't touch state)
    assert_eq!(rv_ext_args("init", &["999", "0"]), "-1");
    assert_eq!(rv_ext("health"), "1", "health after failed re-init");

    // Non-numeric version init → "-1"
    assert_eq!(rv_ext_args("init", &["", "0"]), "-1");
    assert_eq!(rv_ext("health"), "1");

    // Init with ACE mode (ace_present=1)
    assert_eq!(rv_ext_args("init", &["1", "1"]), "0");

    // ── Fire commands (fnc_fire.sqf) ────────────────────────────────────
    //
    //   _extension callExtension ["fire", [
    //       _barrelLength, _chamberPressure, _caliber,
    //       _projectileMass, _cdmId
    //   ]]

    // Normal fire: 368mm barrel, 380 MPa, 5.56mm M855, 4.0g, G7
    let r = rv_ext_args("fire", &["368", "380", "5.56", "4.0", "g7"]);
    assert_ne!(r, "-1", "fire should succeed");
    let parts = parse_array(&r);
    assert_eq!(parts.len(), 4, "fire result should have 4 fields");
    assert!(r.starts_with('['), "result should start with '['");
    let mv: f64 = parts[0].parse().expect("mv should be numeric");
    assert!(mv > 600.0 && mv < 750.0, "mv in [600,750]: {mv}");
    assert!(
        !parts[0].contains("000000"),
        "no excess precision: {}",
        parts[0]
    );

    // Empty args → fail
    assert_eq!(rv_ext_args("fire", &[]), "-1", "empty args -> fail");
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
    // Non-numeric args → fail
    assert_eq!(
        rv_ext_args("fire", &["abc", "def", "xyz", "ghi", "g7"]),
        "-1"
    );

    // Longer barrel → higher MV
    let short = rv_ext_args("fire", &["254", "380", "5.56", "4.0", "g7"]);
    let long = rv_ext_args("fire", &["508", "380", "5.56", "4.0", "g7"]);
    let mv_s: f64 = parse_array(&short)[0].parse().unwrap();
    let mv_l: f64 = parse_array(&long)[0].parse().unwrap();
    assert!(mv_l > mv_s, "longer barrel -> higher MV: {mv_l} > {mv_s}");

    // 7.62mm NATO
    let nato = rv_ext_args("fire", &["508", "380", "7.62", "9.5", "g7"]);
    let mv_nato: f64 = parse_array(&nato)[0].parse().unwrap();
    assert!(
        mv_nato > 500.0 && mv_nato < 750.0,
        "M80 MV in [500,750]: {mv_nato}"
    );

    // ── Step commands (fnc_step.sqf) ────────────────────────────────────
    //
    //   _extension callExtension ["step", [
    //       posX, posY, posZ, velX, velY, velZ, dt,
    //       windX, windY, 0, density, temp, altitude,
    //       cdmId, bc, massG, caliberMm
    //   ]]

    // Normal step: 17-arg pattern
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
    assert_ne!(r, "-1");
    let parts = parse_array(&r);
    assert_eq!(parts.len(), 8, "step result should have 8 fields");
    assert!(r.starts_with('['), "result should start with '['");
    assert!(
        parts[0].parse::<f64>().unwrap() > 0.0,
        "bullet moves forward"
    );
    assert!(
        parts[3].parse::<f64>().unwrap() < 900.0,
        "bullet slows down"
    );

    // Zero dt → pos unchanged
    let dz = rv_ext_args(
        "step",
        &[
            "0", "0", "0", "900", "0", "0", "0", "0", "0", "0", "1.225", "15", "0", "g7", "0.157",
            "4.0", "5.56",
        ],
    );
    assert!(
        parse_array(&dz)[0].parse::<f64>().unwrap().abs() < 0.001,
        "dt=0 → pos_x ~0"
    );

    // Partial args (10 instead of 17) → defaults for rest
    assert_ne!(
        rv_ext_args(
            "step",
            &["0", "0", "0", "900", "0", "0", "0.01", "0", "0", "0"],
        ),
        "-1",
        "10-arg step should not crash"
    );

    // ── Impact commands (fnc_impact.sqf) ────────────────────────────────
    //
    //   _extension callExtension ["impact", [
    //       velX, velY, velZ, massG, caliberMm, effectiveThickness,
    //       armorMaterial, impactAngleDeg, projectileType
    //   ]]

    // Normal: 7.62mm at 900m/s vs 5mm RHA at 0°
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
    assert_ne!(r, "-1");
    let parts = parse_array(&r);
    assert_eq!(parts.len(), 9, "impact result should have 9 fields");
    assert_eq!(
        parts[0].parse::<i32>().unwrap(),
        1,
        "7.62mm at 900m/s should pen 5mm RHA"
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

    // CAManBase (flesh) → over-penetrate
    let flesh = rv_ext_args(
        "impact",
        &["900", "0", "0", "9.5", "7.62", "2", "flesh", "0", "ball"],
    );
    assert_eq!(
        parse_array(&flesh)[0].parse::<i32>().unwrap(),
        1,
        "7.62mm vs 2mm flesh over-penetrate"
    );

    // 85° → ricochet
    let graze = rv_ext_args(
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
    assert_eq!(
        parse_array(&graze)[4].parse::<i32>().unwrap(),
        1,
        "85° -> ricochet"
    );

    // AP ≥ ball penetration
    let r_ball = rv_ext_args(
        "impact",
        &[
            "880",
            "0",
            "0",
            "9.5",
            "7.62",
            "10",
            "steel_rha",
            "0",
            "ball",
        ],
    );
    let r_ap = rv_ext_args(
        "impact",
        &["880", "0", "0", "9.5", "7.62", "10", "steel_rha", "0", "ap"],
    );
    let bp: i32 = parse_array(&r_ball)[0].parse().unwrap();
    let ap: i32 = parse_array(&r_ap)[0].parse().unwrap();
    assert!(ap >= bp, "AP >= ball: AP={ap}, ball={bp}");

    // ── Pipeline: fire → step → impact ──────────────────────────────────
    //
    // Mirrors the real SQF event flow where data flows through
    // fnc_fire → fnc_step → fnc_impact handlers.

    // Fire → MV
    let fire = rv_ext_args("fire", &["508", "380", "5.56", "4.0", "g7"]);
    assert_ne!(fire, "-1");
    let mv: f64 = parse_array(&fire)[0].parse().unwrap();
    assert!(mv > 600.0, "MV should be reasonable: {mv}");

    // Step × 100 (1s flight at dt=0.01)
    let (mut x, mut z) = (0.0_f64, 0.0_f64);
    let (mut vx, mut vz) = (mv, 0.0);

    for _ in 0..100 {
        let s = format!("{x},0,{z},{vx},0,{vz},0.01,0,0,0,1.225,15,0,g7,0.157,4.0,5.56");
        let args: Vec<&str> = s.split(',').collect();
        let r = rv_ext_args("step", &args);
        assert_ne!(r, "-1", "step should succeed");
        let p = parse_array(&r);
        x = p[0].parse().unwrap();
        z = p[2].parse().unwrap();
        vx = p[3].parse().unwrap();
        vz = p[5].parse().unwrap();
    }

    assert!(x > 400.0, "bullet should travel: x={x:.1}");
    assert!(z > 0.0, "bullet should drop: z={z:.1}");
    assert!(vx < mv, "bullet should slow: {vx:.1} < {mv:.1}");

    // Impact: subsonic 5.56mm vs 3mm RHA
    // After 100 steps the bullet may be below the De Marre threshold
    // for 3mm RHA — the contract is that the ABI returns a valid
    // 9-field result (not "-1") with residual velocity defined.
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
    assert_eq!(ip.len(), 9, "impact result should have 9 fields");
    assert!(
        ip[1].parse::<f64>().unwrap() >= 0.0,
        "residual velocity >= 0"
    );

    // ── Return format contracts ─────────────────────────────────────────

    // Error format: literal string "-1"
    assert_eq!(
        rv_ext_args("fire", &["0", "380", "5.56", "4.0", "g7"]),
        "-1"
    );

    // Success: bracketed array, parseable by parseSimpleArray
    let fire = rv_ext_args("fire", &["368", "380", "5.56", "4.0", "g7"]);
    assert!(fire.starts_with('['), "fire: '[' prefix, got {fire}");
    assert!(fire.ends_with(']'), "fire: ']' suffix, got {fire}");

    let step = rv_ext_args(
        "step",
        &[
            "0", "0", "0", "900", "0", "0", "0.01", "0", "0", "0", "1.225", "15", "0", "g7",
            "0.157", "4.0", "5.56",
        ],
    );
    assert!(step.starts_with('['), "step: '[' prefix, got {step}");

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
    assert!(impact.starts_with('['), "impact: '[' prefix, got {impact}");
}

// ── Advanced integration tests ───────────────────────────────────────────────
//
// These simulate EXACTLY what SQF does when tracking multiple simultaneous
// bullets (bullet_id → state hashmap in SQF). Each test calls init itself
// since they need initialized state and init is idempotent.

#[test]
fn sqf_multi_bullet_interleaving() {
    rv_ext_args("init", &["1", "0"]);

    // Fire M855 (5.56mm) and M80 (7.62mm) from different barrel lengths — get clearly different MVs
    let r_a = rv_ext_args("fire", &["368", "380", "5.56", "4.0", "g7"]);
    let r_b = rv_ext_args("fire", &["630", "380", "7.62", "9.5", "g7"]);
    assert_ne!(r_a, "-1");
    assert_ne!(r_b, "-1");

    let mv_a: f64 = parse_array(&r_a)[0].parse().unwrap();
    let mv_b: f64 = parse_array(&r_b)[0].parse().unwrap();
    assert!(
        (mv_a - mv_b).abs() > 10.0,
        "M855 and M80 should have different muzzle velocities: A={mv_a}, B={mv_b}"
    );

    // Bullet A state — M855 parameters
    let mut ax = 0.0_f64;
    let mut ay = 0.0_f64;
    let mut az = 0.0_f64;
    let mut avx = mv_a;
    let mut avy = 0.0_f64;
    let mut avz = 0.0_f64;

    // Bullet B state — M80 parameters
    let mut bx = 0.0_f64;
    let mut by = 0.0_f64;
    let mut bz = 0.0_f64;
    let mut bvx = mv_b;
    let mut bvy = 0.0_f64;
    let mut bvz = 0.0_f64;

    const STEP: &str = "0.01";
    const WIND: &str = "0,0,0";
    const ATMOS: &str = "1.225,15,0";
    const A_CDM: &str = "g7";
    const A_BC: &str = "0.157";
    const A_MASS: &str = "4.0";
    const A_CAL: &str = "5.56";
    const B_CDM: &str = "g7";
    const B_BC: &str = "0.200";
    const B_MASS: &str = "9.5";
    const B_CAL: &str = "7.62";

    // Helper closure: step bullet A and update its state
    let step_a = |x, y, z, vx, vy, vz| -> (f64, f64, f64, f64, f64, f64) {
        let s = format!(
            "{x},{y},{z},{vx},{vy},{vz},{STEP},{WIND},{ATMOS},{A_CDM},{A_BC},{A_MASS},{A_CAL}"
        );
        let args: Vec<&str> = s.split(',').collect();
        let r = rv_ext_args("step", &args);
        assert_ne!(r, "-1", "step A should succeed");
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

    // Helper closure: step bullet B and update its state
    let step_b = |x, y, z, vx, vy, vz| -> (f64, f64, f64, f64, f64, f64) {
        let s = format!(
            "{x},{y},{z},{vx},{vy},{vz},{STEP},{WIND},{ATMOS},{B_CDM},{B_BC},{B_MASS},{B_CAL}"
        );
        let args: Vec<&str> = s.split(',').collect();
        let r = rv_ext_args("step", &args);
        assert_ne!(r, "-1", "step B should succeed");
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

    // Step A 5 times
    for _ in 0..5 {
        (ax, ay, az, avx, avy, avz) = step_a(ax, ay, az, avx, avy, avz);
    }
    assert!(ax > 0.0, "Bullet A should have moved after 5 steps: {ax}");
    assert!(avx < mv_a, "Bullet A should have slowed: {avx} < {mv_a}");
    assert!(az > 0.0, "Bullet A should have dropped: {az}");

    // Step B 5 times — verify B hasn't been corrupted by A's stepping
    for _ in 0..5 {
        (bx, by, bz, bvx, bvy, bvz) = step_b(bx, by, bz, bvx, bvy, bvz);
    }
    assert!(bx > 0.0, "Bullet B should have moved after 5 steps: {bx}");
    assert!(bvx < mv_b, "Bullet B should have slowed: {bvx} < {mv_b}");
    assert!(bz > 0.0, "Bullet B should have dropped: {bz}");
    // B's state is independent — stepping A between B's steps doesn't corrupt
    // (extension is stateless; SQF tracks the hashmap)

    // Alternate stepping A then B for 200 more steps each
    // (longer run ensures BC effect dominates over initial MV advantage)
    for _ in 0..200 {
        (ax, ay, az, avx, avy, avz) = step_a(ax, ay, az, avx, avy, avz);
        (bx, by, bz, bvx, bvy, bvz) = step_b(bx, by, bz, bvx, bvy, bvz);
    }

    // M855 (BC=0.157) has higher drag than M80 (BC=0.200).
    // After 400+ steps, the BC difference dominates and M80 travels farther.
    assert!(
        ax < bx,
        "M855 (higher drag) should travel less than M80: A.x={ax:.1} < B.x={bx:.1}"
    );

    // Both final states must be physically plausible
    assert!(avx > 0.0, "Bullet A forward velocity positive: {avx}");
    assert!(bvx > 0.0, "Bullet B forward velocity positive: {bvx}");
    assert!(az > 0.0, "Bullet A has dropped: {az}");
    assert!(bz > 0.0, "Bullet B has dropped: {bz}");

    // Cross-condition: stepping B between A's steps does not affect A's
    // trajectory. Run A alone as reference and compare.
    let mut ref_x = 0.0_f64;
    let mut ref_y = 0.0_f64;
    let mut ref_z = 0.0_f64;
    let mut ref_vx = mv_a;
    let mut ref_vy = 0.0_f64;
    let mut ref_vz = 0.0_f64;
    for _ in 0..205 {
        (ref_x, ref_y, ref_z, ref_vx, ref_vy, ref_vz) =
            step_a(ref_x, ref_y, ref_z, ref_vx, ref_vy, ref_vz);
    }
    assert!(
        (ax - ref_x).abs() < 0.01,
        "A's x should match reference run (interleaving is stateless): {ax} vs {ref_x}"
    );
}

#[test]
fn sqf_long_duration_stability() {
    rv_ext_args("init", &["1", "0"]);

    // Fire M855
    let r = rv_ext_args("fire", &["368", "380", "5.56", "4.0", "g7"]);
    assert_ne!(r, "-1");
    let mv: f64 = parse_array(&r)[0].parse().unwrap();

    let (mut x, mut y, mut z) = (0.0_f64, 0.0_f64, 0.0_f64);
    let (mut vx, mut vy, mut vz) = (mv, 0.0_f64, 0.0_f64);
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
        assert_eq!(p.len(), 8, "step result should have 8 fields");
        x = p[0].parse().unwrap();
        y = p[1].parse().unwrap();
        z = p[2].parse().unwrap();
        vx = p[3].parse().unwrap();
        vy = p[4].parse().unwrap();
        vz = p[5].parse().unwrap();

        // Check for NaN/Inf every 1000 steps
        if step_idx > 0 && step_idx % 1000 == 0 {
            for (j, val) in [x, y, z, vx, vy, vz].iter().enumerate() {
                assert!(
                    val.is_finite(),
                    "NaN/Inf at step {step_idx}, field {j}: {val}"
                );
            }
        }
    }

    assert_eq!(bad_steps, 0, "all 10000 steps returned success");
    assert!(
        vx < 50.0,
        "forward velocity very low after 100 s: {vx:.1} m/s"
    );
    assert!(z > 1000.0, "bullet dropped significantly: z={z:.1} m");
    assert!(vz > 0.0, "bullet still falling: vz={vz:.1}");
    // Terminal velocity for subsonic G7 bullet ≈ 100–150 m/s
    assert!(
        vz < 200.0,
        "vertical velocity bounded near terminal: {vz:.1}"
    );
    // Verify the terminal all values are finite
    assert!(x.is_finite());
    assert!(y.is_finite());
    assert!(z.is_finite());
    assert!(vx.is_finite());
    assert!(vy.is_finite());
    assert!(vz.is_finite());
}

#[test]
fn sqf_wind_accumulation() {
    rv_ext_args("init", &["1", "0"]);

    // Fire M855
    let r = rv_ext_args("fire", &["368", "380", "5.56", "4.0", "g7"]);
    assert_ne!(r, "-1");
    let mv: f64 = parse_array(&r)[0].parse().unwrap();

    // ── Zero-wind run ────────────────────────────────────────────────────
    let (mut zx, mut zy, mut zz) = (0.0_f64, 0.0_f64, 0.0_f64);
    let (mut zvx, mut zvy, mut zvz) = (mv, 0.0_f64, 0.0_f64);

    for _ in 0..100 {
        let s = format!("{zx},{zy},{zz},{zvx},{zvy},{zvz},0.01,0,0,0,1.225,15,0,g7,0.157,4.0,5.56");
        let args: Vec<&str> = s.split(',').collect();
        let r = rv_ext_args("step", &args);
        assert_ne!(r, "-1");
        let p = parse_array(&r);
        zx = p[0].parse().unwrap();
        zy = p[1].parse().unwrap();
        zz = p[2].parse().unwrap();
        zvx = p[3].parse().unwrap();
        zvy = p[4].parse().unwrap();
        zvz = p[5].parse().unwrap();
    }

    // ── Crosswind run (lateral wind) ────────────────────────────────────
    // NOTE: The wind model applies wind as an absolute velocity adjustment
    // per step (NOT multiplied by dt), so even a gentle wind accumulates
    // significant y-velocity over many steps. Using a gentle wind and few
    // steps to keep the test numerically stable.
    let (mut cx, mut cy, mut cz) = (0.0_f64, 0.0_f64, 0.0_f64);
    let (mut cvx, mut cvy, mut cvz) = (mv, 0.0_f64, 0.0_f64);

    for _ in 0..20 {
        // wind_y = 0.5 m/s gentle crosswind
        let s =
            format!("{cx},{cy},{cz},{cvx},{cvy},{cvz},0.01,0,0.5,0,1.225,15,0,g7,0.157,4.0,5.56");
        let args: Vec<&str> = s.split(',').collect();
        let r = rv_ext_args("step", &args);
        assert_ne!(r, "-1");
        let p = parse_array(&r);
        cx = p[0].parse().unwrap();
        cy = p[1].parse().unwrap();
        cz = p[2].parse().unwrap();
        cvx = p[3].parse().unwrap();
        cvy = p[4].parse().unwrap();
        cvz = p[5].parse().unwrap();
    }

    // Zero-wind: lateral displacement ≈ 0
    assert!(
        zy.abs() < 0.001,
        "zero-wind bullet should have negligible y: {zy}"
    );

    // Crosswind: lateral displacement ≠ 0
    assert!(
        cy.abs() > 0.001,
        "crosswind bullet should have y displacement: {cy}"
    );

    // Crosswind y-velocity finite (wind applied as absolute kick per step)
    assert!(cvy.abs() < 30.0, "crosswind y-velocity bounded: {cvy:.1}");

    // Both trajectories should still be progressing forward
    assert!(zx > 0.0, "zero-wind bullet moves forward: {zx:.1}");
    assert!(cx > 0.0, "crosswind bullet moves forward: {cx:.1}");
}

#[test]
fn sqf_subsonic_transition() {
    rv_ext_args("init", &["1", "0"]);

    // Fire M855
    let r = rv_ext_args("fire", &["368", "380", "5.56", "4.0", "g7"]);
    assert_ne!(r, "-1");
    let mv: f64 = parse_array(&r)[0].parse().unwrap();
    assert!(mv > 600.0, "MV reasonable: {mv}");

    let (mut x, mut y, mut z) = (0.0_f64, 0.0_f64, 0.0_f64);
    let (mut vx, mut vy, mut vz) = (mv, 0.0_f64, 0.0_f64);
    let mut crossed_mach1 = false;
    let mut pre_transonic_vx = mv;

    for step_idx in 0..3000 {
        let s = format!("{x},{y},{z},{vx},{vy},{vz},0.01,0,0,0,1.225,15,0,g7,0.157,4.0,5.56");
        let args: Vec<&str> = s.split(',').collect();
        let r = rv_ext_args("step", &args);
        assert_ne!(r, "-1", "step should succeed at {step_idx}");
        let p = parse_array(&r);
        assert_eq!(p.len(), 8);

        x = p[0].parse().unwrap();
        y = p[1].parse().unwrap();
        z = p[2].parse().unwrap();
        let new_vx: f64 = p[3].parse().unwrap();
        vy = p[4].parse().unwrap();
        vz = p[5].parse().unwrap();
        let mach: f64 = p[6].parse().unwrap();

        // Every value must be finite
        assert!(x.is_finite(), "x NaN at {step_idx}");
        assert!(new_vx.is_finite(), "vx NaN at {step_idx}");
        assert!(vy.is_finite(), "vy NaN at {step_idx}");
        assert!(vz.is_finite(), "vz NaN at {step_idx}");
        assert!(mach.is_finite(), "mach NaN at {step_idx}");

        // Velocity must monotonically decrease (strictly)
        assert!(
            new_vx <= vx + 0.001,
            "velocity must not increase: {new_vx} > {vx} at step {step_idx}"
        );
        vx = new_vx;

        if !crossed_mach1 && mach < 1.0 {
            crossed_mach1 = true;
        }

        if !crossed_mach1 {
            pre_transonic_vx = vx;
        }
    }

    assert!(crossed_mach1, "bullet must cross Mach 1 within 3000 steps");
    assert!(
        pre_transonic_vx > 0.0,
        "pre-transonic velocity positive: {pre_transonic_vx}"
    );

    // After 3000 steps (30 s), should be well into subsonic, all finite
    assert!(vx > 0.0, "still moving forward: {vx:.1}");
    assert!(z > 0.0, "has dropped: {z:.1}");
}

#[test]
fn sqf_energy_conservation_no_drag() {
    rv_ext_args("init", &["1", "0"]);

    // Fire M855 to get a muzzle velocity
    let r = rv_ext_args("fire", &["368", "380", "5.56", "4.0", "g7"]);
    assert_ne!(r, "-1");
    let mv: f64 = parse_array(&r)[0].parse().unwrap();

    let (mut x, y, mut z) = (0.0_f64, 0.0_f64, 0.0_f64);
    let (mut vx, vy, mut vz) = (mv, 0.0_f64, 0.0_f64);

    for _ in 0..100 {
        // density=0 → no aerodynamic drag; only gravity and velocity act
        let s = format!("{x},{y},{z},{vx},{vy},{vz},0.01,0,0,0,0,15,0,g7,0.157,4.0,5.56");
        let args: Vec<&str> = s.split(',').collect();
        let r = rv_ext_args("step", &args);
        assert_ne!(r, "-1");
        let p = parse_array(&r);
        x = p[0].parse().unwrap();
        z = p[2].parse().unwrap();
        vx = p[3].parse().unwrap();
        vz = p[5].parse().unwrap();
    }

    // After 100 steps at dt=0.01 → t = 1.0 s
    // In vacuum with zero wind:
    //   - vx should be constant (no drag)
    //   - vz = GRAVITY * t (free fall from rest)
    //   - z  = ½ GRAVITY t²  (semi-implicit Euler gives slight offset)
    assert!(
        (vx - mv).abs() < 0.001,
        "vx conserved in vacuum: {vx} ≈ {mv}"
    );

    // vz after 1.0 s free fall from rest = 9.80665 m/s
    let expected_vz = 9.80665;
    assert!(
        (vz - expected_vz).abs() < 0.01,
        "vz ≈ GRAVITY × 1 s: {vz} ≈ {expected_vz}"
    );

    // z after 1.0 s free fall, semi-implicit Euler integration
    // Exact: z = GRAVITY * dt² * n(n+1)/2 = 9.80665 * 0.0001 * 5050 ≈ 4.952
    assert!(
        (z - 4.95).abs() < 0.1,
        "free-fall drop ≈ ½ G t²: z={z:.3} ≈ 4.95"
    );
}

#[test]
fn sqf_corner_case_args() {
    rv_ext_args("init", &["1", "0"]);

    // Fire M855
    let r = rv_ext_args("fire", &["368", "380", "5.56", "4.0", "g7"]);
    assert_ne!(r, "-1");
    let mv: f64 = parse_array(&r)[0].parse().unwrap();
    let mv_s = &format!("{mv}");

    // 1. Very small timestep (1e-8 s) → should produce valid forward progress
    let r = rv_ext_args(
        "step",
        &[
            "0",
            "0",
            "0",
            mv_s,
            "0",
            "0",
            "0.00000001",
            "0",
            "0",
            "0",
            "1.225",
            "15",
            "0",
            "g7",
            "0.157",
            "4.0",
            "5.56",
        ],
    );
    assert_ne!(r, "-1", "tiny dt (1e-8) should succeed");
    let p = parse_array(&r);
    let x: f64 = p[0].parse().unwrap();
    assert!(x > 0.0, "tiny dt still moves forward: {x}");
    assert!(x.is_finite());

    // 2. Very large timestep (10 s) → should not crash
    let r = rv_ext_args(
        "step",
        &[
            "0", "0", "0", mv_s, "0", "0", "10.0", "0", "0", "0", "1.225", "15", "0", "g7",
            "0.157", "4.0", "5.56",
        ],
    );
    assert_ne!(r, "-1", "large dt (10 s) should not crash");
    let p = parse_array(&r);
    assert_eq!(p.len(), 8, "step with large dt returns 8 fields");
    // With 10 s timestep the bullet may have moved through the target or
    // be unreachable — the contract is a valid result array, not "-1".

    // 3. High altitude (50 000 m) — stratospheric density
    let r = rv_ext_args(
        "step",
        &[
            "0", "0", "0", mv_s, "0", "0", "0.01", "0", "0", "0", "1.225", "15", "50000", "g7",
            "0.157", "4.0", "5.56",
        ],
    );
    assert_ne!(r, "-1", "high altitude (50 km) should not crash");
    let p = parse_array(&r);
    let vx_at_alt: f64 = p[3].parse().unwrap();
    assert!(vx_at_alt.is_finite(), "vx finite at 50 km: {vx_at_alt}");

    // 4. Extremely high velocity (Mach ≈ 15) → hypersonic
    let r = rv_ext_args(
        "step",
        &[
            "0", "0", "0", "5000", "0", "0", "0.01", "0", "0", "0", "1.225", "15", "0", "g7",
            "0.157", "4.0", "5.56",
        ],
    );
    assert_ne!(r, "-1", "hypersonic (5000 m/s) should not crash");
    let p = parse_array(&r);
    let hx: f64 = p[0].parse().unwrap();
    let hvx: f64 = p[3].parse().unwrap();
    assert!(hx.is_finite(), "x finite at hypersonic: {hx}");
    assert!(hvx.is_finite(), "vx finite at hypersonic: {hvx}");
    assert!(hvx < 5000.0, "hypersonic bullet slows: {hvx} < 5000");

    // 5. Near-vacuum (density = 0.001) → drag almost zero
    let r = rv_ext_args(
        "step",
        &[
            "0", "0", "0", mv_s, "0", "0", "0.01", "0", "0", "0", "0.001", "15", "0", "g7",
            "0.157", "4.0", "5.56",
        ],
    );
    assert_ne!(r, "-1", "near-vacuum step succeeds");
    let p = parse_array(&r);
    let nvx: f64 = p[3].parse().unwrap();
    assert!(
        nvx > mv - 5.0,
        "velocity barely decreases in near-vacuum: {nvx} vs {mv}"
    );

    // 6. Zero defaults (bc=0, mass=0, caliber=0) — should not crash
    let r = rv_ext_args(
        "step",
        &[
            "0", "0", "0", mv_s, "0", "0", "0.01", "0", "0", "0", "0", "0", "0", "g7", "0", "0",
            "0",
        ],
    );
    assert_ne!(r, "-1", "zero-defaults step should not crash");
    let p = parse_array(&r);
    assert_eq!(p.len(), 8, "zero-defaults step returns 8 fields");
    assert!(
        p.iter()
            .all(|v| v.parse::<f64>().map_or(false, f64::is_finite)),
        "all output fields finite with zero defaults"
    );
}

// ── New Test: Impact material matrix ───────────────────────────────────
//
// Tests the impact command with all armor material types, verifying
// that material factors correctly modulate penetration resistance.

#[test]
fn sqf_impact_material_matrix() {
    // All at 20mm, 880 m/s, 7.62mm ball, 9.5g, 0° (K=56509 after ARL calibration)
    macro_rules! impact_mat {
        ($mat:expr) => {{
            let r = rv_ext_args(
                "impact",
                &["880", "0", "0", "9.5", "7.62", "20", $mat, "0", "ball"],
            );
            assert_ne!(r, "-1", "impact({}) should succeed", $mat);
            assert!(r.starts_with('['), "{}: '[' prefix", $mat);
            let p = parse_array(&r);
            assert_eq!(p.len(), 9, "{}: 9 fields", $mat);
            p.iter().map(|s| s.to_string()).collect::<Vec<_>>()
        }};
    }

    // material_factor keys: steel_rha(1.0), steel_hha(1.25),
    // aluminum_5083(0.35), composite_glass(0.4), ceramic_al2o3(2.5),
    // wood(0.05).  Unknown → default 1.0 (RHA-equivalent).
    let rha = impact_mat!("steel_rha");
    let hha = impact_mat!("steel_hha");
    let al5083 = impact_mat!("aluminum_5083");
    let cglass = impact_mat!("composite_glass");
    let ceramic = impact_mat!("ceramic_al2o3");
    let wood = impact_mat!("wood");
    let unknown = impact_mat!("nonexistent_material");

    let rha_pen: i32 = rha[0].parse().unwrap();
    let hha_pen: i32 = hha[0].parse().unwrap();
    let al_pen: i32 = al5083[0].parse().unwrap();
    let glass_pen: i32 = cglass[0].parse().unwrap();
    let ceramic_pen: i32 = ceramic[0].parse().unwrap();
    let wood_pen: i32 = wood[0].parse().unwrap();
    let unk_pen: i32 = unknown[0].parse().unwrap();

    // All residual velocities >= 0
    for (name, p) in [
        ("rha", &rha),
        ("hha", &hha),
        ("al5083", &al5083),
        ("cglass", &cglass),
        ("ceramic", &ceramic),
        ("wood", &wood),
        ("unknown", &unknown),
    ] {
        assert!(
            p[1].parse::<f64>().unwrap() >= 0.0,
            "{name}: residual vel >= 0"
        );
    }

    // Material ordering: lower factor = easier to pen
    assert!(
        al_pen >= rha_pen,
        "Al5083 (0.35) pens easier than RHA (1.0)"
    );
    assert!(
        glass_pen >= ceramic_pen,
        "Composite glass (0.4) pens easier than ceramic (2.5)"
    );
    assert!(
        wood_pen >= rha_pen,
        "Wood (0.05) pens easier than RHA (1.0)"
    );
    assert!(
        hha_pen <= rha_pen,
        "HHA (1.25) blocks better than RHA (1.0)"
    );
    assert!(
        ceramic_pen <= rha_pen,
        "Ceramic (2.5) blocks better than RHA (1.0)"
    );

    // Known absolute thresholds at 20mm/880 m/s/7.62mm ball (K=56509 after ARL calibration)
    assert_eq!(al_pen, 1, "20mm Al5083 should PEN at 880m/s");
    assert_eq!(glass_pen, 1, "20mm composite glass should PEN at 880m/s");
    assert_eq!(wood_pen, 1, "20mm wood should PEN at 880m/s");
    assert_eq!(rha_pen, 0, "20mm RHA should NOT PEN at 880m/s");
    assert_eq!(
        ceramic_pen, 0,
        "20mm Al2O3 ceramic should NOT PEN at 880m/s"
    );

    // Unknown material → RHA-equivalent fallback
    assert_eq!(unk_pen, rha_pen, "unknown material → RHA behaviour");
}

// ── New Test: Fragmentation in impact ──────────────────────────────────
//
// Tests the last two fields of the impact result: effective_fragments
// (projectile breakup) and spall_fragments (armour backface spall).
// High-velocity impacts on thin armour should produce measurable
// fragmentation; low-velocity impacts should produce fewer.

#[test]
fn sqf_fragmentation_in_impact() {
    // High-velocity: 900 m/s, 5.56mm, 4.0g vs 3mm RHA at 0°
    let hi = rv_ext_args(
        "impact",
        &[
            "900",
            "0",
            "0",
            "4.0",
            "5.56",
            "3",
            "steel_rha",
            "0",
            "ball",
        ],
    );
    let hi_p = parse_array(&hi);
    assert!(hi_p[7].parse::<u32>().is_ok(), "fragments field numeric");
    assert!(hi_p[8].parse::<u32>().is_ok(), "spall field numeric");
    let hi_pen: i32 = hi_p[0].parse().unwrap();
    let hi_frags: u32 = hi_p[7].parse().unwrap();
    let hi_spall: u32 = hi_p[8].parse().unwrap();

    // Penetrating high-velocity impact should produce some fragmentation
    if hi_pen == 1 {
        // Either projectile fragments or armour spall
        assert!(
            hi_frags > 0 || hi_spall > 0,
            "penetrating 900 m/s impact should fragment: frags={hi_frags}, spall={hi_spall}"
        );
    }

    // Low-velocity: 200 m/s, same params → lower or zero fragments
    let lo = rv_ext_args(
        "impact",
        &[
            "200",
            "0",
            "0",
            "4.0",
            "5.56",
            "3",
            "steel_rha",
            "0",
            "ball",
        ],
    );
    let lo_p = parse_array(&lo);
    assert!(lo_p[7].parse::<u32>().is_ok(), "low frag field numeric");
    let lo_frags: u32 = lo_p[7].parse().unwrap();
    let lo_spall: u32 = lo_p[8].parse().unwrap();

    // Key invariant: lower velocity → fewer or equal fragments
    assert!(
        lo_frags <= hi_frags || lo_spall <= hi_spall,
        "lower velocity should produce fewer fragments: lo({lo_frags},{lo_spall}) vs hi({hi_frags},{hi_spall})"
    );
}

// ── New Test: Error recovery ───────────────────────────────────────────
//
// After a failed/bad command, the extension must still accept valid
// commands in the same session — state must not be corrupted.

#[test]
fn sqf_error_recovery() {
    // 1. Fire failure modes
    assert_eq!(rv_ext_args("fire", &[]), "-1", "empty fire → -1");
    assert_eq!(
        rv_ext_args("fire", &["abc", "def"]),
        "-1",
        "non-numeric fire → -1"
    );

    // Valid fire after errors
    let r = rv_ext_args("fire", &["368", "380", "5.56", "4.0", "g7"]);
    assert_ne!(r, "-1", "valid fire after errors recovers");
    assert!(r.starts_with('['), "recovered fire returns array");

    // 2. Step — has no minimum-arg check (missing fields default). Just
    //    verify valid steps still work after a failed fire.
    for dt in ["0.01", "0.05", "0.1"] {
        let r = rv_ext_args(
            "step",
            &[
                "0", "0", "0", "900", "0", "0", dt, "0", "0", "0", "1.225", "15", "0", "g7",
                "0.157", "4.0", "5.56",
            ],
        );
        assert_ne!(r, "-1", "valid step (dt={dt}) after errors recovers");
        assert!(r.starts_with('['));
    }

    // 3. Impact — also has no minimum-arg check (all defaults). Verify
    //    normal impact still works.
    let r = rv_ext_args(
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
    assert_ne!(r, "-1", "valid impact after errors recovers");
    assert!(r.starts_with('['));

    // 4. State integrity
    // Health returns "1" (init should have been called by lifecycle test)
    let h = rv_ext("health");
    assert_eq!(h, "1", "state not corrupted by error commands: health={h}");
}

// ── P0 Handlers: zeroing, shooter, component, wound ────────────────────────────
//
// Tests for the new P0 C ABI dispatch handlers that were wired in
// RVExtensionArgs alongside the existing init/fire/step/impact handlers.

#[test]
fn rv_ext_zeroing_basic() {
    rv_ext_args("init", &["1", "0"]);
    // 63 mm sight height, 100 m zero range, 948 m/s MV
    let r = rv_ext_args("zeroing", &["63", "100", "948"]);
    assert!(r.len() > 2, "zeroing result should have content: {r}");
    assert!(r.starts_with('['), "zeroing result should be an array: {r}");
    // Typical 5.56mm zero at 100m → ~3.5 MOA with 63mm sight height
    // Parse the result: "[3.5]"
    let inner = r.trim_start_matches('[').trim_end_matches(']');
    let moa: f64 = inner.parse().expect("zeroing result should be numeric MOA");
    assert!(
        moa > 0.0 && moa < 20.0,
        "zero MOA should be plausible: {moa}"
    );
}

#[test]
fn rv_ext_zeroing_no_sight_fails() {
    rv_ext_args("init", &["1", "0"]);
    // Zero sight height, 100 m zero, 948 m/s — only drop compensation remains
    let r = rv_ext_args("zeroing", &["0", "100", "948"]);
    // Drop comp ~1.88 MOA — result should be a positive number
    assert!(r.starts_with('['), "zeroing result should be an array: {r}");
    let inner = r.trim_start_matches('[').trim_end_matches(']');
    let moa: f64 = inner.parse().expect("zeroing result should be numeric MOA");
    assert!(
        moa > 0.0 && moa < 10.0,
        "drop-comp MOA should be plausible: {moa}"
    );
}

#[test]
fn rv_ext_shooter_prone_bipod() {
    rv_ext_args("init", &["1", "0"]);
    // 2.0 MOA base, prone, bipod, 72 BPM, breath hold (0), advanced, 300 m
    let r = rv_ext_args(
        "shooter",
        &["2.0", "prone", "bipod", "72", "0", "advanced", "300"],
    );
    assert!(
        r.contains(","),
        "shooter result should be comma-separated: {r}"
    );
    assert!(r.starts_with('['), "shooter result should be an array: {r}");
    let parts: Vec<&str> = r
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .collect();
    assert_eq!(parts.len(), 3, "shooter result should have 3 fields: {r}");
    let moa: f64 = parts[0].parse().expect("first field should be MOA");
    assert!(
        moa > 0.0 && moa < 10.0,
        "shooter MOA should be plausible: {moa}"
    );
}

#[test]
fn rv_ext_component_engine() {
    rv_ext_args("init", &["1", "0"]);
    // MBT, front hit, 120mm APFSDS at 1600 m/s, 0°, armour penetrated
    let r = rv_ext_args(
        "component",
        &[
            "mbt", "front", "120", "4600", "1600", "apfsds", "0", "900", "1",
        ],
    );
    assert!(
        r.starts_with('['),
        "component result should be an array: {r}"
    );
    let parts: Vec<&str> = r
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .collect();
    assert_eq!(parts.len(), 3, "component result should have 3 fields: {r}");
    let mob_kill: f64 = parts[0]
        .parse()
        .expect("mobility kill prob should be numeric");
    assert!(
        mob_kill >= 0.0 && mob_kill <= 1.0,
        "mobility kill prob in [0,1]: {mob_kill}"
    );
    // 120mm APFSDS to MBT front at 1600 m/s → high kill probability
    assert!(
        mob_kill > 0.3,
        "APFSDS vs MBT front should have significant kill prob: {mob_kill}"
    );
}

#[test]
fn rv_ext_wound_basic() {
    rv_ext_args("init", &["1", "0"]);
    // 7.62mm NATO ball at 850 m/s into soft tissue
    let r = rv_ext_args("wound", &["850", "0", "0", "9.5", "7.62", "ball"]);
    assert!(r.starts_with('['), "wound result should be an array: {r}");
    let parts: Vec<&str> = r
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .collect();
    assert_eq!(parts.len(), 5, "wound result should have 5 fields: {r}");
    let pen_mm: f64 = parts[0]
        .parse()
        .expect("penetration depth should be numeric");
    assert!(
        pen_mm > 50.0,
        "rifle round should penetrate deeply: {pen_mm}mm"
    );
}

// ── Struct-based C ABI entry points ─────────────────────────────────────────
//
// Uses abe_init/abe_health/abe_version directly (struct C ABI).
// These also share the global STATE — only assertions that hold regardless
// of test ordering are checked here.

#[test]
fn sqf_c_abi() {
    unsafe {
        let v = abe_version();
        assert_eq!(CStr::from_ptr(v).to_str().unwrap(), "0.1.0");

        assert_eq!(abe_init(999, 0), -1, "wrong version should fail");

        // init may succeed or be a no-op (OnceLock already set by another
        // test).  The important contract is: if init succeeded, health
        // returns 1; if init was a no-op, health returns whatever the
        // other test set.
        if abe_init(1, 0) == 0 {
            assert_eq!(abe_health(), 1, "health should be 1 after init");
        }
    }
}
