// ABE test runner — execVM from -init parameter
// Runs all test SQF files and outputs PASSED/FAILED to RPT.
// Expected to be run at main menu (CBA + ABE mods must be loaded).

diag_log text "[ABE_TEST] === ABE Test Runner Started ===";

// Wait for CBA to be fully initialized
waitUntil { time > 0 || !isNil "CBA_fnc_registerModule" };
sleep 2;  // extra settle time

diag_log text "[ABE_TEST] CBA ready, loading test suites...";

// Run each test file
private _testFiles = [
    "z\abe\addons\abo_core\tests\config",
    "z\abe\addons\abo_core\tests\init",
    "z\abe\addons\abo_core\tests\ace3_compat"
];

{
    private _path = _x + ".sqf";
    diag_log text format ["[ABE_TEST] Loading: %1", _path];
    private _compiled = preprocessFileLineNumbers _path;
    private _result = call compile _compiled;
    diag_log text format ["[ABE_TEST] Completed: %1 (result: %2)", _path, _result];
} forEach _testFiles;

diag_log text "[ABE_TEST] === ABE Test Suite Complete ===";

// Keep running so we can check RPT
[] spawn { scriptName "ABE_Test_Watchdog"; while {true} do { sleep 60; diag_log text "[ABE_TEST] Watchdog tick — game still alive"; }; };
