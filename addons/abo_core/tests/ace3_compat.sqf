#include "..\script_component.hpp"
#include "script_test_common.hpp"

diag_log text "=== ABO Core: ACE3 Compat Tests ===";

// In test environment without ACE3, ace3_compat should detect absence and exit
private _acePresent = isClass(configFile >> "CfgPatches" >> "ace_advanced_ballistics");
if (!_acePresent) then {
    diag_log text "ACE3 not detected — testing fallback path";
    // ace3_compat called with "enable" in standalone mode
    // Should not set ace3Overridden
    ["enable"] call FUNC(ace3_compat);
    TEST_LOGIC(isNil {GVAR(ace3Overridden)} || {!GVAR(ace3Overridden)},"ace3Overridden should be false without ACE3");
};

// Test: ace3_compat with "disable" mode should not error
["disable"] call FUNC(ace3_compat);
diag_log text "ACE3 compat disable passed";
