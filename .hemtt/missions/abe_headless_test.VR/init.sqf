// ABE Headless Test Suite
// Tests the abe_ballistics_ext extension via callExtension
// All output uses [ABE_TEST] prefix for RPT log parsing
// Run with: arma3server_x64 -config=.hemtt/server.cfg -mod=releases/latest

if (!isServer) exitWith {};

private _ext = "abe_ballistics_ext";
private _failures = 0;
private _passes = 0;
private _results = [];

#define ABE_TEST(desc, condition) \
    if (condition) then { \
        _passes = _passes + 1; \
        _results pushBack format ["[ABE_TEST] PASS: %1", desc]; \
    } else { \
        _failures = _failures + 1; \
        _results pushBack format ["[ABE_TEST] FAIL: %1", desc]; \
    }

[] spawn {
    waitUntil { time > 0 };

    diag_log "[ABE_TEST] ========================================";
    diag_log "[ABE_TEST] ABE Headless Test Suite Starting";
    diag_log "[ABE_TEST] ========================================";

    // ── Extension loading ────────────────────────────────────

    private _version = _ext callExtension "version";
    ABE_TEST("Extension loads and returns a version string", _version != "");
    diag_log format ["[ABE_TEST] INFO: Extension version = %1", _version];
    ABE_TEST("Version is 0.1.0", _version == "0.1.0");

    // ── Health before init ───────────────────────────────────

    private _health = _ext callExtension "health";
    ABE_TEST("Health returns 0 before init", _health == "0");

    // ── Init ─────────────────────────────────────────────────

    private _initResult = _ext callExtension ["init", [1, 0]];
    ABE_TEST("Init returns 0 (success)", _initResult == "0");

    // ── Health after init ────────────────────────────────────

    _health = _ext callExtension "health";
    ABE_TEST("Health returns 1 after init", _health == "1");

    // ── Fire command ─────────────────────────────────────────
    // 368mm barrel, 380MPa chamber, 5.56mm caliber, 4.0g projectile, G7 drag

    private _fireResult = _ext callExtension ["fire", ["368", "380", "5.56", "4.0", "g7"]];
    private _fireParsed = parseSimpleArray _fireResult;
    ABE_TEST("Fire returns a parsable array", _fireParsed isEqualType []);
    ABE_TEST("Fire array has 4 elements", count _fireParsed == 4);

    if (count _fireParsed >= 4) then {
        private _mv = _fireParsed select 0;
        private _pressure = _fireParsed select 1;
        private _burnFrac = _fireParsed select 2;
        private _barrelTime = _fireParsed select 3;
        ABE_TEST("Muzzle velocity in valid range (800-1100 m/s)", _mv > 800 && _mv < 1100);
        ABE_TEST("Chamber pressure in valid range (200-500 MPa)", _pressure > 200 && _pressure < 500);
        ABE_TEST("Propellant burn fraction in valid range (0-1)", _burnFrac >= 0 && _burnFrac <= 1);
        ABE_TEST("Barrel time is positive", _barrelTime > 0);
        diag_log format ["[ABE_TEST] DATA: fire=[mv=%1, P=%2, burn=%3, t_ms=%4]", _mv, _pressure, _burnFrac, _barrelTime];
    };

    // ── Fire edge cases ──────────────────────────────────────

    private _fireFail = _ext callExtension ["fire", ["0", "380", "5.56", "4.0", "g7"]];
    ABE_TEST("Fire with zero barrel length returns -1", _fireFail == "-1");

    private _fireNeg = _ext callExtension ["fire", ["368", "-10", "5.56", "4.0", "g7"]];
    ABE_TEST("Fire with negative pressure returns -1", _fireNeg == "-1");

    // ── Longer barrel produces higher MV ─────────────────────

    private _shortFire = _ext callExtension ["fire", ["254", "380", "5.56", "4.0", "g7"]];
    private _longFire = _ext callExtension ["fire", ["508", "380", "5.56", "4.0", "g7"]];
    private _shortParsed = parseSimpleArray _shortFire;
    private _longParsed = parseSimpleArray _longFire;

    if (_shortParsed isEqualType [] && _longParsed isEqualType [] && count _shortParsed >= 1 && count _longParsed >= 1) then {
        private _mvShort = _shortParsed select 0;
        private _mvLong = _longParsed select 0;
        ABE_TEST("Longer barrel (508mm) gives higher MV than shorter (254mm)", _mvLong > _mvShort);
        diag_log format ["[ABE_TEST] DATA: barrel MV comparison: 254mm -> %1 m/s, 508mm -> %2 m/s", _mvShort, _mvLong];
    };

    // ── Step command ─────────────────────────────────────────
    // Bullet at (0,0,0) with v=(900,0,0) m/s, dt=0.01s, G7 BC=0.157

    private _stepResult = _ext callExtension ["step", ["0", "0", "0", "900", "0", "0", "0.01", "0", "0", "0", "1.225", "15", "0", "g7", "0.157", "4.0", "5.56"]];
    private _stepParsed = parseSimpleArray _stepResult;
    ABE_TEST("Step returns a parsable array", _stepParsed isEqualType []);
    ABE_TEST("Step array has 8 elements", count _stepParsed == 8);

    if (count _stepParsed >= 8) then {
        private _stepPosX = _stepParsed select 0;
        private _stepVelX = _stepParsed select 3;
        private _stepMach = _stepParsed select 6;
        ABE_TEST("Bullet moves forward (pos_x > 0)", _stepPosX > 0);
        ABE_TEST("Bullet slows down (vel_x < 900)", _stepVelX < 900);
        ABE_TEST("Mach number is positive", _stepMach > 0);
        diag_log format ["[ABE_TEST] DATA: step=[pos_x=%1, vel_x=%2, mach=%3]", _stepPosX, _stepVelX, _stepMach];
    };

    // ── Step with zero dt ────────────────────────────────────

    private _stepZeroDt = _ext callExtension ["step", ["0", "0", "0", "900", "0", "0", "0", "0", "0", "0", "1.225", "15", "0", "g7", "0.157", "4.0", "5.56"]];
    private _stepZeroParsed = parseSimpleArray _stepZeroDt;
    if (_stepZeroParsed isEqualType [] && count _stepZeroParsed >= 1) then {
        private _stepZeroPos = _stepZeroParsed select 0;
        ABE_TEST("Step with dt=0 has no movement (pos_x ~ 0)", (_stepZeroPos >= -0.001) && (_stepZeroPos <= 0.001));
    };

    // ── Impact command ───────────────────────────────────────
    // 7.62mm ball at 900 m/s vs 5mm RHA at 0°

    private _impactResult = _ext callExtension ["impact", ["900", "0", "0", "9.5", "7.62", "5", "steel_rha", "0", "ball"]];
    private _impactParsed = parseSimpleArray _impactResult;
    ABE_TEST("Impact returns a parsable array", _impactParsed isEqualType []);
    ABE_TEST("Impact array has 9 elements", count _impactParsed == 9);

    if (count _impactParsed >= 9) then {
        private _pen = _impactParsed select 0;
        private _residual = _impactParsed select 1;
        private _energy = _impactParsed select 2;
        private _effThick = _impactParsed select 3;
        private _rico = _impactParsed select 4;
        ABE_TEST("7.62mm ball at 900m/s penetrates 5mm RHA at 0°", _pen == 1);
        ABE_TEST("Residual velocity is positive after penetration", _residual > 0);
        ABE_TEST("Impact energy is positive", _energy > 0);
        ABE_TEST("Effective thickness is positive", _effThick > 0);
        ABE_TEST("No ricochet at 0° impact angle", _rico == 0);
        diag_log format ["[ABE_TEST] DATA: impact=[pen=%1, v_res=%2, E=%3 J, eff_t=%4 mm]", _pen, _residual, _energy, _effThick];
    };

    // ── Impact: thick armor (no penetration) ─────────────────

    private _impactThick = _ext callExtension ["impact", ["900", "0", "0", "9.5", "7.62", "50", "steel_rha", "0", "ball"]];
    private _thickParsed = parseSimpleArray _impactThick;
    if (_thickParsed isEqualType [] && count _thickParsed >= 1) then {
        private _thickPen = _thickParsed select 0;
        ABE_TEST("7.62mm ball does NOT penetrate 50mm RHA", _thickPen == 0);
    };

    // ── Impact: grazing angle (ricochet) ─────────────────────

    private _impactGraze = _ext callExtension ["impact", ["900", "0", "0", "9.5", "7.62", "10", "steel_rha", "85", "ball"]];
    private _grazeParsed = parseSimpleArray _impactGraze;
    if (_grazeParsed isEqualType [] && count _grazeParsed >= 5) then {
        private _grazeRico = _grazeParsed select 4;
        ABE_TEST("Grazing impact at 85° causes ricochet", _grazeRico == 1);
    };

    // ── Impact: AP vs ball comparison ────────────────────────

    private _apImpact = _ext callExtension ["impact", ["880", "0", "0", "9.5", "7.62", "10", "steel_rha", "0", "ap"]];
    private _ballImpact = _ext callExtension ["impact", ["880", "0", "0", "9.5", "7.62", "10", "steel_rha", "0", "ball"]];
    private _apParsed = parseSimpleArray _apImpact;
    private _ballParsed = parseSimpleArray _ballImpact;
    if (_apParsed isEqualType [] && _ballParsed isEqualType [] && count _apParsed >= 1 && count _ballParsed >= 1) then {
        private _apPen = _apParsed select 0;
        private _ballPen = _ballParsed select 0;
        ABE_TEST("AP projectile penetrates as well or better than ball at 880m/s vs 10mm RHA", _apPen >= _ballPen);
    };

    // ── Unknown command ──────────────────────────────────────

    private _unknown = _ext callExtension "nonsense";
    ABE_TEST("Unknown string command returns error message", _unknown == "unknown: nonsense");

    private _unknownArgs = _ext callExtension ["nonsense", ["a"]];
    ABE_TEST("Unknown array command returns error message", _unknownArgs == "unknown: nonsense");

    // ── Summary ──────────────────────────────────────────────

    diag_log "[ABE_TEST] ========================================";
    diag_log format ["[ABE_TEST] RESULTS: %1 passed, %2 failed", _passes, _failures];

    if (_failures == 0) then {
        diag_log "[ABE_TEST] ALL TESTS PASSED";
    } else {
        diag_log "[ABE_TEST] SOME TESTS FAILED";
    };
    diag_log "[ABE_TEST] ========================================";

    // Force mission end so server can exit
    forceEnd;
};
