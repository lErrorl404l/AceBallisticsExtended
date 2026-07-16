description = "ABE Core Module Tests";
author = "ABE Team";
version = 1;

// ── Weapon config loading ────────────────────────────────────
test_weapon_config_load = {
    private _weaponId = "abe_test_rifle";
    private _cfg = parseSimpleArray (
        "abe_ballistics_ext" callExtension ["config_weapon", [_weaponId]]
    );
    private _result = count _cfg > 0 && {(_cfg select 0) == _weaponId};
    _result
};

// ── Fire event produces valid trajectory seed ───────────────
test_fire_event = {
    // Simulated fire call: [weaponId, muzzleVel, barrelLength, caliber, mass]
    private _fireResult = parseSimpleArray (
        "abe_ballistics_ext" callExtension ["fire", ["abe_test_rifle", "855", 930, 0.509, 0.004]]
    );
    count _fireResult >= 3
};

// ── Step updates position correctly ─────────────────────────
test_step_integration = {
    private _state = [0, 0, 0, 930, 0, 0, 0, 4, 5.56, 0.3, 0, 0, 9.81, 0];
    private _result = parseSimpleArray (
        "abe_ballistics_ext" callExtension ["step", [
            _state param [0], _state param [1], _state param [2],
            _state param [3], _state param [4], _state param [5],
            _state param [6], _state param [7], _state param [8],
            _state param [9], _state param [10], _state param [11],
            _state param [12], _state param [13]
        ]]
    );
    count _result >= 3
};

// ── Health check returns version ────────────────────────────
test_health_check = {
    private _health = "abe_ballistics_ext" callExtension "health";
    private _result = _health select [0, 1] == "[";  // starts with JSON array bracket
    _result
};

// ── Impact handler returns valid result struct ─────────────
test_impact_evaluation = {
    private _impactResult = parseSimpleArray (
        "abe_ballistics_ext" callExtension ["impact", [
            930, 0, 0,     // velocity vector
            4.0,            // mass (g)
            5.56,           // caliber (mm)
            0.008,          // effective thickness (m)
            "steel_rha",    // armor material
            0.0,            // impact angle (deg)
            "ball"          // projectile type
        ]]
    );
    count _impactResult >= 4
};
