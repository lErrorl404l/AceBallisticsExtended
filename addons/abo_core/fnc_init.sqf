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

true
