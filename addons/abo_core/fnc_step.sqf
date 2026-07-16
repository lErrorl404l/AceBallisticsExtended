#include "script_component.hpp"

private _extension = missionNamespace getVariable ["ABE_extension", "abe_ballistics_ext"];
private _tracked = GVAR(trackedBullets);
private _toRemove = [];

// Iterate all tracked bullets
private _bulletIds = keys _tracked;
{
    private _bulletId = _x;
    private _state = _tracked get _bulletId;
    if (!isNil "_state") then {
        _state params ["_pos", "_vel", "_fireTime", "_cdmId", "_bc", "_massG", "_caliberMm"];

    // Find the projectile object
    private _projectile = _bulletId call BIS_fnc_objectFromNetId;
    if (isNull _projectile) then {
        _toRemove pushBack _bulletId;
    };

    // Calculate delta time
    private _dt = diag_tickTime - _fireTime;

    // Read environment
    private _wind = wind;
    private _altitude = (_pos select 2) max 0;
    private _temp = ([_altitude] call ace_weather_fnc_calculateTemperature) param [0, 15.0];
    private _density = 1.225;

    // Call extension to step bullet
    private _stepResult = _extension callExtension [
        "step",
        [
            _pos select 0, _pos select 1, _pos select 2,
            _vel select 0, _vel select 1, _vel select 2,
            _dt,
            _wind select 0, _wind select 1, 0,
            _density,
            _temp,
            _altitude,
            _cdmId,
            _bc,
            _massG,
            _caliberMm
        ]
    ];

    // Update bullet position and velocity
    if (count _stepResult >= 6) then {
        private _newPos = [_stepResult select 0, _stepResult select 1, _stepResult select 2];
        private _newVel = [_stepResult select 3, _stepResult select 4, _stepResult select 5];

        _projectile setPosASL _newPos;
        _projectile setVelocity _newVel;

        // Update tracked state
        _tracked set [_bulletId, [_newPos, _newVel, _fireTime, _cdmId, _bc, _massG, _caliberMm]];
    };
    };
} forEach _bulletIds;

// Clean up dead bullets
{
    _tracked deleteAt _x;
} forEach _toRemove;
