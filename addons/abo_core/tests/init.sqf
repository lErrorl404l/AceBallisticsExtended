#include "..\script_component.hpp"
#include "script_test_common.hpp"

LOG("=== ABO Core: Init Tests === (stateless portions)");

// Test: trackedBullets should be nil before init (stateless check)
TEST_LOGIC(isNil {GVAR(trackedBullets)}, "trackedBullets should be nil before init");

// Test: ace_common detection (should be false in test env)
private _aceCommon = isClass(configFile >> "CfgPatches" >> "ace_common");
if (_aceCommon) then {
    LOG("ACE3 common detected - test environment has ACE3");
} else {
    LOG("ACE3 common NOT detected - standalone mode");
};
