#include "script_component.hpp"
/*
 * Author: ABE Team
 * ACE3 ballistics override compatibility layer.
 *
 * ACE3's advanced_ballistics module (ace_advanced_ballistics) has TWO entry
 * points that must be neutralized when ABE is active:
 *
 *   1. FIRED HANDLER  — ace_advanced_ballistics_fnc_handleFired
 *      Registered during CBA_settingsInitialized.  Reads ammo/weapon config,
 *      calls `"ace" callExtension ["ballistics:bullet:new", [...]]` to
 *      register the bullet with ACE3's native extension, and adds the bullet
 *      to ace_advanced_ballistics_allBullets.
 *
 *   2. PER-FRAME HANDLER — ace_advanced_ballistics_fnc_handleFirePFH
 *      Iterates ace_advanced_ballistics_allBullets and calls
 *      `"ace" callExtension ["ballistics:bullet:simulate", [...]]` for each
 *      bullet.  Sets velocity on the projectile from the native result.
 *
 * ABE's override strategy (multi-layered):
 *
 *   A) SETTING LOCK — Set `ace_advanced_ballistics_enabled = false` in
 *      missionNamespace.  ACE3's CBA_settingsInitialized handler gates on
 *      `if (!GVAR(enabled)) exitWith {};`.  If this runs BEFORE the
 *      CBA_settingsInitialized event, ACE3 never registers its handlers.
 *      If it runs after, the setting has no effect on already-registered
 *      handlers (but we have layers B and C).
 *
 *   B) HASHMAP REPLACEMENT — Replace
 *      `ace_advanced_ballistics_allBullets` with an empty hashmap.
 *      ACE3's PFH iterates this hashmap every frame.  An empty hashmap
 *      makes the PFH a no-op regardless of whether it is registered.
 *
 *   C) STEP-PURGE — In ABE's per-frame step (fnc_step.sqf), delete any
 *      ABE-tracked bullets from ACE3's hashmap.  This catches late
 *      additions from other addons that call ACE3's fired path.
 *
 * Events consumed/produced:
 *   - Consumes: (none directly; called from ABE_fnc_init)
 *   - Produces: GVAR(ace3Overridden) missionNamespace flag
 *   - Modifies: ace_advanced_ballistics_enabled (missionNamespace)
 *   - Modifies: ace_advanced_ballistics_allBullets (hashmap)
 *
 * The override is reversible — pass _mode = "disable" to restore ACE3's
 * setting (note: already-registered handlers will NOT re-check the setting
 * until the next mission restart).
 *
 * Arguments:
 * 0: mode <STRING> — "enable" (default) or "disable"
 *
 * Return Value:
 * <BOOL> — true if ACE3 advanced ballistics was detected and handled
 *
 * Public: No
 */

params [["_mode", "enable", [""]]];

// Detect ACE3 advanced_ballistics by its CfgPatches entry (not ace_common,
// which is broader).  This tells us the module IS in the addon list, even
// if the mission maker disabled it via CBA settings.
private _aceAB = isClass (configFile >> "CfgPatches" >> "ace_advanced_ballistics");

if (!_aceAB) exitWith {
    diag_log "[ABE] ACE3 Advanced Ballistics not detected — compat skipped";
    false
};

// Defensive default — ensures fnc_step.sqf sees a defined value even if
// this function somehow hasn't run before the PFH fires.
GVAR(ace3Overridden) = false;

// Detect whether ACE3's CBA_settingsInitialized handler has already fired.
// If it has, Layer A (setting lock) had no effect, but Layers B and C
// (hashmap replacement, per-frame purge) still work.
private _aceSettingsInited = missionNamespace getVariable ["ace_advanced_ballistics_settingsInitialized", false];
if (_aceSettingsInited) then {
    diag_log "[ABE] ACE3 Advanced Ballistics settingsInitialized already fired — Layer A ineffective, Layers B and C still active";
};

if (_mode == "enable") then {
    // -----------------------------------------------------------------
    // LAYER A: Disable ACE3's CBA setting.
    // -----------------------------------------------------------------
    // ACE3's ace_advanced_ballistics reads its enabled state from the
    // CBA setting ace_advanced_ballistics_enabled (stored in
    // missionNamespace).  The module checks this in its
    // CBA_settingsInitialized handler:
    //
    //   if (!GVAR(enabled)) exitWith {};
    //
    // where GVAR(enabled) = ace_advanced_ballistics_enabled.
    //
    // Setting this false before CBA_settingsInitialized fires prevents
    // ACE3 from registering fired handlers + PFH entirely.
    // Setting it after CBA_settingsInitialized still prevents late init
    // paths (CBA settings reload, Eden preview, etc.).
    missionNamespace setVariable ["ace_advanced_ballistics_enabled", false, true];
    diag_log "[ABE] ACE3 compat: set ace_advanced_ballistics_enabled = false";

    // -----------------------------------------------------------------
    // LAYER B: Replace ACE3's allBullets hashmap with an empty one.
    // -----------------------------------------------------------------
    // ACE3's per-frame handler iterates:
    //
    //   } forEach GVAR(allBullets);
    //
    // where GVAR(allBullets) = ace_advanced_ballistics_allBullets.
    //
    // By replacing it with an empty hashmap, the forEach iterates zero
    // entries and the PFH becomes a true no-op (no native calls, no
    // velocity writes).  This works even if ACE3's PFH is already
    // registered.
    private _aceAllBullets = missionNamespace getVariable ["ace_advanced_ballistics_allBullets", nil];
    if (!isNil "_aceAllBullets") then {
        // Clear in-place to invalidate any existing references held by
        // the PFH scope.  Assigning a new hashmap is safe here because
        // missionNamespace getVariable is called fresh each frame.
        {
            _aceAllBullets deleteAt _x;
        } forEach (keys _aceAllBullets);
    };

    // -----------------------------------------------------------------
    // Mark the override as active for other ABE subsystems.
    // -----------------------------------------------------------------
    GVAR(ace3Overridden) = true;

    diag_log "[ABE] ACE3 Advanced Ballistics overridden — ABE now handles all trajectory simulation";

    // Post-override sanity check: if ACE3's hashmap is non-empty, something
    // added entries after our Layer B clear (possibly a late-init addon).
    private _acePostCheck = missionNamespace getVariable ["ace_advanced_ballistics_allBullets", nil];
    if (!isNil "_acePostCheck" && {count _acePostCheck > 0}) then {
        diag_log format ["[ABE] WARNING: ACE3 allBullets has %1 entries after override — Layer B incomplete (Layer C step-purge will handle)", count _acePostCheck];
    };

} else {

    // ---- mode == "disable": restore ACE3's setting -------------------
    missionNamespace setVariable ["ace_advanced_ballistics_enabled", true, true];
    GVAR(ace3Overridden) = false;
    diag_log "[ABE] ACE3 Advanced Ballistics setting restored (active at next mission start)";
};

true
