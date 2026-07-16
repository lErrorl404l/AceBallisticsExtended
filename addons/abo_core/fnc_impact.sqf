#include "script_component.hpp"

params ["_projectile", "_target", "_posASL", "_vel", "_massG", "_caliberMm", "_ammo"];

private _extension = missionNamespace getVariable ["ABE_extension", "abe_ballistics_ext"];

// Determine armor properties at impact point
private _armorThickness = 0;
private _armorMaterial = "steel_rha";
private _impactAngle = 0;

if (_target isKindOf "LandVehicle" || _target isKindOf "Tank" || _target isKindOf "Car") then {
    // Read vehicle armor config
    private _hitPoint = _target worldToModel _posASL;
    _armorThickness = getNumber (configOf _target >> "armor") * 0.005; // crude: armor→mm
    _armorMaterial = "steel_rha";
    _impactAngle = 0; // TODO: compute from surface normal
};

private _speed = vectorMagnitude _vel;

private _impactResult = _extension callExtension [
    "impact",
    [
        _vel select 0, _vel select 1, _vel select 2,
        _massG,
        _caliberMm,
        _armorThickness,
        _armorMaterial,
        _impactAngle,
        getText (configFile >> "CfgAmmo" >> _ammo >> "ABO_projectileType")
    ]
];

// Apply damage based on penetration result
if (count _impactResult >= 4) then {
    private _penetrated = _impactResult select 0;
    private _residualVel = _impactResult select 1;
    private _ricochet = _impactResult select 4;
    private _fragments = _impactResult select 7;

    if (_penetrated > 0 && _target isKindOf "CAManBase") then {
        // Apply penetration damage to units
        private _damage = (_residualVel / 900) * 0.5;
        _target setHitIndex [0, _damage, false, _projectile];
    };

    if (_ricochet > 0) then {
        // Deflect projectile
        private _newDir = direction _projectile + (_impactResult select 5);
        private _newVel = [sin _newDir * _residualVel, cos _newDir * _residualVel, _vel select 2 * 0.5];
        _projectile setVelocity _newVel;
    };
};
