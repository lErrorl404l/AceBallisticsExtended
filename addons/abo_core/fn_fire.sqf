#include "script_component.hpp"

params ["_unit", "_weapon", "_muzzle", "_ammo", "_projectile"];

private _extension = missionNamespace getVariable ["ABE_extension", "abe_ballistics_ext"];

// Read weapon config (ABO/ACE3 config entries take priority)
private _barrelLength = getNumber (configFile >> "CfgWeapons" >> _weapon >> "ABO_barrelLength");
private _chamberPressure = getNumber (configFile >> "CfgWeapons" >> _weapon >> "ABO_chamberPressure");
private _caliber = getNumber (configFile >> "CfgAmmo" >> _ammo >> "caliber");
private _projectileMass = getNumber (configFile >> "CfgAmmo" >> _ammo >> "ABO_projectileMass");
private _cdmId = getText (configFile >> "CfgAmmo" >> _ammo >> "ABO_cdmId");

// IRL database fallback: resolve weapon via PHF map when config fields are absent
if (_barrelLength <= 0 || _chamberPressure <= 0 || _caliber <= 0) then {
    private _resolve = _extension callExtension ["resolve_weapon", [_weapon]];
    if (_resolve select 0 != "0") then {
        private _parts = parseSimpleArray (_resolve select 0);
        if (count _parts >= 3) then {
            if (_caliber <= 0) then { _caliber = _parts select 0; };
            if (_barrelLength <= 0) then { _barrelLength = _parts select 1; };
            if (_chamberPressure <= 0) then {
                _chamberPressure = _parts select 3;
            };
        };
    };
};

// Fallback: read from weapon config
if (_projectileMass <= 0) then {
    _projectileMass = getNumber (configFile >> "CfgWeapons" >> _weapon >> "ABO_projectileMass");
};
if (_barrelLength <= 0) then {
    _barrelLength = getNumber (configFile >> "CfgWeapons" >> _weapon >> "ABO_barrelLength");
};

// Call extension to calculate interior ballistics
private _fireResult = _extension callExtension [
    "fire",
    [
        _barrelLength,
        _chamberPressure,
        _caliber,
        _projectileMass,
        _cdmId
    ]
];

// Store bullet state for per-frame tracking
if !(isNull _projectile) then {
    private _bulletId = _projectile call BIS_fnc_netId;
    private _state = [
        getPosASL _projectile,                          // pos
        velocity _projectile,                            // vel
        diag_tickTime,                                   // fire time
        _cdmId,                                          // drag model
        getNumber (configFile >> "CfgAmmo" >> _ammo >> "ABO_bcG7"),  // BC
        _projectileMass,                                 // mass (g)
        _caliber                                         // caliber (mm)
    ];
    private _tracked = missionNamespace getVariable ["abe_abo_core_trackedBullets", createHashMap];
    _tracked set [_bulletId, _state];
};
