#include "script_component.hpp"

private _extension = missionNamespace getVariable ["ABE_extension", "abe_ballistics_ext"];
private _health = _extension callExtension "health";

if (_health != "1") then {
    diag_log "[ABE] WARNING: Extension health check failed";
};
