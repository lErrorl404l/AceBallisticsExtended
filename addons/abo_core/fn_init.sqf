#include "script_component.hpp"

params [["_extensionName", "abe_ballistics_ext", [""]]];

// Extension version check
private _version = _extensionName callExtension "version";
diag_log format ["[ABE] Initializing, extension: %1 version: %2", _extensionName, _version];

if (_version == "") exitWith {
    diag_log "[ABE] ERROR: Extension not found or failed to load";
    false
};

// Initialize extension
private _apiVersion = 1;
private _acePresent = isClass (configFile >> "CfgPatches" >> "ace_common");
private _result = _extensionName callExtension ["init", [_apiVersion, [false, _acePresent] select isNil {_acePresent}]];

if (_result select 0 != 0) exitWith {
    diag_log format ["[ABE] ERROR: Extension init failed (api: %1, ace: %2)", _apiVersion, _acePresent];
    false
};

// Store extension reference
missionNamespace setVariable ["ABE_extension", _extensionName];
missionNamespace setVariable ["ABE_aceMode", _acePresent];

// Initialize tracking hashmaps
GVAR(trackedBullets) = createHashMap;
GVAR(armorState) = createHashMap;

// Pre-register ACE3 weapon config values as overrides for the IRL lookup
[{
    params ["_extension"];
    private _weaponClasses = "getText (_x >> 'ACE_barrelLength') != ''" configClasses (configFile >> "CfgWeapons");
    {
        private _cls = configName _x;
        private _barrel = getNumber (_x >> "ACE_barrelLength");
        private _twist = getNumber (_x >> "ACE_barrelTwist");
        if (_barrel > 0) then {
            _extension callExtension ["register_override", [
                _cls, 0, _barrel, _twist, 0, 0
            ]];
        };
    } forEach _weaponClasses;
    diag_log format ["[ABE] Registered %1 ACE3 weapon overrides", count _weaponClasses];
}, [_extensionName], 0.5] call CBA_fnc_waitAndExecute;

diag_log format ["[ABE] Initialized successfully (ACE3 mode: %1)", _acePresent];

    // Register EH for firing
    if (_acePresent) then {
        // ACE3 mode: hook into ACE3's handleFire
        ["ace_firedPlayer", {
            params ["_unit", "_weapon", "_muzzle", "_mode", "_ammo", "_magazine", "_projectile"];
            [_unit, _weapon, _muzzle, _ammo, _projectile] call FUNC(fire);
        }] call CBA_fnc_addEventHandler;

        ["ace_firedNonPlayer", {
            params ["_unit", "_weapon", "_muzzle", "_mode", "_ammo", "_magazine", "_projectile"];
            [_unit, _weapon, _muzzle, _ammo, _projectile] call FUNC(fire);
        }] call CBA_fnc_addEventHandler;

        // Override ACE3's built-in ballistics module so ABE controls
        // trajectory simulation instead.  This disables ACE3's
        // advanced_ballistics setting, replaces its tracking hashmap,
        // and arms the per-frame purge in fnc_step.sqf.
        // See fnc_ace3_compat.sqf for the override strategy (layers A-C).
        call FUNC(ace3_compat);
    } else {
        // Standalone mode: hook into vanilla fired event
        ["CBA_fired", {
            params ["_unit", "_weapon", "_muzzle", "_mode", "_ammo", "_magazine", "_projectile"];
            [_unit, _weapon, _muzzle, _ammo, _projectile] call FUNC(fire);
        }] call CBA_fnc_addEventHandler;
    };

    // Start per-frame handler for bullet tracking
    [{
        call FUNC(step);
        call FUNC(health);
    }, 0.0] call CBA_fnc_addPerFrameHandler;

    // Register HitPart handler on all vehicles (existing + future spawns)
    ["AllVehicles", "init", {
        params ["_vehicle"];
        _vehicle addEventHandler ["HitPart", {
            params ["_target", "_shooter", "_projectile", "_posASL", "_vel", "_speed", "_normal", "_surfaceType", "_ammo"];
            [_target, _shooter, _projectile, _posASL, _vel, _speed, _normal, _surfaceType, _ammo] call FUNC(impact);
        }];
    }] call CBA_fnc_addEventHandler;

    // Mission-end cleanup: restore ACE3's setting so other missions are
    // not affected.  Note: missionNamespace resets on mission unload, so
    // this is defensive.  Uses vanilla Arma 3 mission event handler.
    if (_acePresent) then {
        addMissionEventHandler ["Ended", {
            ["disable"] call FUNC(ace3_compat);
        }];
    };

    true
