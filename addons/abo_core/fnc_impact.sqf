#include "script_component.hpp"
/*
 * Author: ABE Team
 * Handles bullet impact with HitPart event. Computes impact angle,
 * queries armor data, calls the extension for pen/ricochet/frag,
 * and applies results to the target.
 *
 * Arguments:
 * 0: target <Object>
 * 1: shooter <Object>
 * 2: projectile <Object>
 * 3: posASL <Array>
 * 4: velocity <Array>
 * 5: speed <Number>
 * 6: surfaceNormal <Array>
 * 7: surfaceType <String>
 * 8: ammoType <String>
 *
 * Return Value:
 * None
 *
 * Public: No
 */

params ["_target", "_shooter", "_projectile", "_posASL", "_vel", "_speed", "_normal", "_surfaceType", "_ammo"];

private _extension = missionNamespace getVariable ["ABE_extension", "abe_ballistics_ext"];

// Impact angle from velocity vs surface normal
// 0° = perpendicular (max pen), 90° = grazing
private _velNorm = vectorNormalized _vel;
private _impactAngleDeg = acos (abs (_velNorm vectorDotProduct _normal));

// Pull tracked bullet state
private _bulletId = _projectile call BIS_fnc_netId;
private _bulletState = GVAR(trackedBullets) get _bulletId;
private _massG = 0.01;
private _caliberMm = 5.56;
if (!isNil "_bulletState") then {
    _massG = _bulletState select 5;
    _caliberMm = _bulletState select 6;
    GVAR(trackedBullets) deleteAt _bulletId;
};

// ── Armor lookup ────────────────────────────────────────────
private _armorThickness = 0.01;
private _armorMaterial = "steel_rha";
private _effectiveThickness = 0.01;

if (_target isKindOf "CAManBase") then {
    _armorThickness = 2.0;  // ~2mm RHA equivalent
    _armorMaterial = "flesh";
    _effectiveThickness = _armorThickness;
} else {
    if (_target isKindOf "LandVehicle" || _target isKindOf "Tank" || _target isKindOf "Car" || _target isKindOf "Air") then {
        private _configArmor = getNumber (configOf _target >> "armor");
        _armorThickness = _configArmor * 0.01;  // scaled to mm-equivalent

        // Multi-hit degradation per vehicle-section
        private _sectionKey = netId _target + str floor ((_target worldToModel _posASL) select 0);
        private _armorState = GVAR(armorState) getOrDefault [_sectionKey, 1.0];
        _effectiveThickness = (_armorThickness / (cos _impactAngleDeg max 0.01)) * _armorState;
        GVAR(armorState) set [_sectionKey, _armorState * 0.85];
    };
};

// ── Call extension ──────────────────────────────────────────
private _impactResult = _extension callExtension [
    "impact",
    [
        _vel select 0, _vel select 1, _vel select 2,
        _massG,
        _caliberMm,
        _effectiveThickness,
        _armorMaterial,
        _impactAngleDeg,
        getText (configFile >> "CfgAmmo" >> _ammo >> "ABO_projectileType")
    ]
];

// ── Apply results ───────────────────────────────────────────
if (count _impactResult >= 4) then {
    private _penetrated = parseNumber (_impactResult select 0);
    private _residualVel = parseNumber (_impactResult select 1);
    private _ricochet = parseNumber (_impactResult select 4);
    private _fragments = parseNumber (_impactResult select 7);

    if (_penetrated > 0 && _target isKindOf "CAManBase") then {
        _target setHitIndex [0, ((_residualVel / 900) * 0.5 * (_caliberMm / 5.56) min 1), false, _projectile];
    };

    if (_ricochet > 0 && !isNull _projectile) then {
        private _reflectAngle = _ricochet * 10;
        private _outDir = vectorNormalized (
            _vel vectorAdd (_normal vectorMultiply (2 * (_velNorm vectorDotProduct _normal)))
        );
        _projectile setVelocity [
            _outDir select 0 * _residualVel,
            _outDir select 1 * _residualVel,
            _outDir select 2 * _residualVel
        ];
    };

    if (_fragments > 0 && _target isKindOf "CAManBase") then {
        private _fragDamage = (_residualVel / 900) * 0.5 * (_caliberMm / 5.56);
        _target setHitIndex [0, ((_fragDamage + _fragments * 0.1) min 1), false, _projectile];
    };
};
