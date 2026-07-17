#include "..\script_component.hpp"
#include "script_test_common.hpp"

diag_log text "=== ABO Core: Config Tests ===";

// CfgPatches
TEST_DEFINED(configFile >> "CfgPatches" >> "abo_core","CfgPatches entry exists");

// CfgFunctions — check all 6 functions registered
TEST_DEFINED(configFile >> "CfgFunctions" >> "abe" >> "abo_core" >> "init","init function");
TEST_DEFINED(configFile >> "CfgFunctions" >> "abe" >> "abo_core" >> "fire","fire function");
TEST_DEFINED(configFile >> "CfgFunctions" >> "abe" >> "abo_core" >> "step","step function");
TEST_DEFINED(configFile >> "CfgFunctions" >> "abe" >> "abo_core" >> "impact","impact function");
TEST_DEFINED(configFile >> "CfgFunctions" >> "abe" >> "abo_core" >> "health","health function");
TEST_DEFINED(configFile >> "CfgFunctions" >> "abe" >> "abo_core" >> "ace3_compat","ace3_compat function");

// CfgWeapons base
TEST_DEFINED(configFile >> "CfgWeapons" >> "abe_weapon_base","Weapon base class");
TEST_OP(getNumber(configFile >> "CfgWeapons" >> "abe_weapon_base" >> "ABO_barrelLength"),==,-1,"barrelLength defaults to -1");
TEST_OP(getNumber(configFile >> "CfgWeapons" >> "abe_weapon_base" >> "ABO_zeroRange"),==,100,"zeroRange defaults to 100");

// CfgAmmo base
TEST_DEFINED(configFile >> "CfgAmmo" >> "abe_ammo_base","Ammo base class");
TEST_OP(getText(configFile >> "CfgAmmo" >> "abe_ammo_base" >> "ABO_cdmId"),==,"g7","Default cdmId is g7");
