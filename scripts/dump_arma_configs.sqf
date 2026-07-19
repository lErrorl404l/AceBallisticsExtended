
private _dumpWeapon = {
    params ["_weaponClass", "_cfgPath"];
    private _parent = configName inheritsFrom _cfgPath;
    private _muzzles = getArray (_cfgPath >> "muzzles");
    private _magwells = getArray (_cfgPath >> "magazineWell");
    {
        _magwells append (getArray (_cfgPath >> _x >> "magazineWell"));
    } forEach (_muzzles select {_x != "this"});
    diag_log text format [
        "WPN|%1|%2|%3|%4|%5|%6|%7|%8",
        _weaponClass, _parent,
        getNumber (_cfgPath >> "initSpeed"),
        getNumber (_cfgPath >> "ACE_barrelLength"),
        getNumber (_cfgPath >> "ACE_barrelTwist"),
        getNumber (_cfgPath >> "ACE_RailHeight"),
        getNumber (_cfgPath >> "ACE_muzzleMoment"),
        str _magwells
    ];
};

private _dumpAmmo = {
    params ["_ammoClass", "_cfgPath"];
    diag_log text format [
        "AMMO|%1|%2|%3|%4|%5|%6|%7|%8|%9|%10|%11|%12|%13",
        _ammoClass,
        getNumber (_cfgPath >> "caliber"),
        getNumber (_cfgPath >> "mass"),
        getNumber (_cfgPath >> "hit"),
        getNumber (_cfgPath >> "indirectHitRange"),
        getNumber (_cfgPath >> "ACE_caliber"),
        getNumber (_cfgPath >> "ACE_bulletMass"),
        str (getArray (_cfgPath >> "ACE_ballisticCoefficients")),
        getNumber (_cfgPath >> "ACE_dragModel"),
        str (getArray (_cfgPath >> "ACE_muzzleVelocities")),
        str (getArray (_cfgPath >> "ACE_barrelLengths")),
        getNumber (_cfgPath >> "ACE_standardAtmosphere"),
        (getArray (_cfgPath >> "ACE_ammoTempMuzzleVelocityShifts")) param [0, 0, [0]]
    ];
};

private _dumpMagazine = {
    params ["_magClass", "_cfgPath"];
    diag_log text format [
        "MAG|%1|%2|%3|%4|%5|%6",
        _magClass,
        configName (_cfgPath >> "ammo"),
        getNumber (_cfgPath >> "initSpeed"),
        getNumber (_cfgPath >> "count"),
        getNumber (_cfgPath >> "ACE_isBelt"),
        str (getArray (_cfgPath >> "ACE_restrictToCaliber"))
    ];
};

private _dumpMagWell = {
    params ["_wellClass", "_cfgPath"];
    private _mags = [];
    for "_i" from 0 to (count _cfgPath - 1) do {
        _mags pushBack configName (_cfgPath select _i);
    };
    diag_log text format ["MAGWELL|%1|%2", _wellClass, str _mags];
};

private _dumpVehicle = {
    params ["_vehClass", "_cfgPath"];
    diag_log text format [
        "VEH|%1|%2|%3|%4|%5|%6|%7|%8",
        _vehClass, configName inheritsFrom _cfgPath,
        getNumber (_cfgPath >> "armor"),
        getNumber (_cfgPath >> "armorStructural"),
        getNumber (_cfgPath >> "cargoIsCoDriver"),
        getNumber (_cfgPath >> "ACE_armorClass"),
        str (getArray (_cfgPath >> "ACE_hitpointArmorValues")),
        getNumber (_cfgPath >> "ACE_canUseAces")
    ];
};

private _excludeWeapons = [
    "Default", "DefaultWeapon", "WeaponSlotInfo", "RifleCore", "Rifle",
    "Rifle_Base_F", "Launcher_Base_F", "Pistol_Base_F", "Launcher",
    "Pistol", "SMG", "MachineGun", "Shotgun", "SniperRifle",
    "Rifle_Long_Base_F", "Pistol_Base_F", "SMG_Base_F", "Launcher_Base_F",
    "MachineGun_Base_F", "Shotgun_Base_F", "SniperRifle_Base_F",
    "BipodCore", "WeaponAccessory", "MuzzleSlot", "PointerSlot",
    "CowsSlot", "UnderBarrelSlot", "ItemCore", "InventoryMuzzleItem_Base_F",
    "InventoryOpticsItem_Base_F", "InventoryPointerItem_Base_F",
    "InventoryUnderbarrelItem_Base_F", "Uniform_Base_F", "Vest_Base_F",
    "H_HelmetB", "NVGoggles", "Binocular", "Laserdesignator",
    "ItemCompass", "ItemGPS", "ItemMap", "ItemRadio", "ItemWatch",
    "FirstAidKit", "Medikit", "ToolKit", "MineDetector"
];

private _cfgRoot = configFile;

diag_log text "=== ABE DUMP START ===";
diag_log text format ["Arma3 %1.%2", productVersion select 2, productVersion select 3];
diag_log text format ["Mods: %1", str (activatedAddons)];

diag_log text "=== ABE SECTION CfgWeapons ===";
private _wpnCount = 0;
for "_i" from 0 to (count (_cfgRoot >> "CfgWeapons") - 1) do {
    private _class = (_cfgRoot >> "CfgWeapons") select _i;
    private _className = configName _class;
    if (!(_className in _excludeWeapons)) then {
        if (isClass _class) then {
            [_className, _class] call _dumpWeapon;
            _wpnCount = _wpnCount + 1;
        };
    };
};
diag_log text format ["Dumped %1 weapons", _wpnCount];

diag_log text "=== ABE SECTION CfgAmmo ===";
private _ammoCount = 0;
for "_i" from 0 to (count (_cfgRoot >> "CfgAmmo") - 1) do {
    private _class = (_cfgRoot >> "CfgAmmo") select _i;
    private _className = configName _class;
    if (isClass _class) then {
        [_className, _class] call _dumpAmmo;
        _ammoCount = _ammoCount + 1;
    };
};
diag_log text format ["Dumped %1 ammo types", _ammoCount];

diag_log text "=== ABE SECTION CfgMagazines ===";
private _magCount = 0;
for "_i" from 0 to (count (_cfgRoot >> "CfgMagazines") - 1) do {
    private _class = (_cfgRoot >> "CfgMagazines") select _i;
    private _className = configName _class;
    if (!(_className in ["Default", "CA_Magazine", "VehicleMagazine"])) then {
        if (isClass _class) then {
            [_className, _class] call _dumpMagazine;
            _magCount = _magCount + 1;
        };
    };
};
diag_log text format ["Dumped %1 magazines", _magCount];

diag_log text "=== ABE SECTION CfgMagazineWells ===";
private _wellCount = 0;
for "_i" from 0 to (count (_cfgRoot >> "CfgMagazineWells") - 1) do {
    private _class = (_cfgRoot >> "CfgMagazineWells") select _i;
    private _className = configName _class;
    if (isClass _class) then {
        [_className, _class] call _dumpMagWell;
        _wellCount = _wellCount + 1;
    };
};
diag_log text format ["Dumped %1 magazine wells", _wellCount];

private _excludeVehicles = [
    "All","AllVehicles","Land","LandVehicle","Car","Car_F",
    "Tank","Tank_F","Wheeled_APC_F","Tracked_APC_F","MBT_01_base_F",
    "MBT_02_base_F","MBT_03_base_F","APC_Tracked_01_base_F","APC_Tracked_02_base_F",
    "APC_Tracked_03_base_F","APC_Wheeled_01_base_F","APC_Wheeled_02_base_F",
    "APC_Wheeled_03_base_F","StaticWeapon","StaticMGWeapon","StaticCannon",
    "Ship","Ship_F","Submarine","Helicopter","Helicopter_Base_F",
    "Plane","Plane_Base_F","UAV","Drone","ReammoBox_F",
    "Strategic","Thing","Animal","Man","CAManBase",
    "Uniform_Base_F","Vest_Base_F","H_HelmetB","NVGoggles","Binocular",
    "Laserdesignator","ItemCompass","ItemGPS","ItemMap","ItemRadio","ItemWatch",
    "FirstAidKit","Medikit","ToolKit","MineDetector",
    "Slingload_base_F","ParachuteBase","B_Parachute_00_F",
    "Weapon_Bone","Weapon_Base","Bag_Base","Bag_Base_EP1","Bag_F",
    "WeaponHolder","WeaponHolderSimulated","GroundWeaponHolder",
    "ReammoBox","ReammoBox_F","NATO_Box_Base","EAST_Box_Base",
    "Guerilla_Box_Base","SupplyBox_Base","ThingEffect",
    "CraterLong","CraterLong_small","Effect"
];

diag_log text "=== ABE SECTION CfgVehicles ===";
private _vehCount = 0;
for "_i" from 0 to (count (_cfgRoot >> "CfgVehicles") - 1) do {
    private _class = (_cfgRoot >> "CfgVehicles") select _i;
    private _className = configName _class;
    if (!(_className in _excludeVehicles)) then {
        if (isClass _class) then {
            [_className, _class] call _dumpVehicle;
            _vehCount = _vehCount + 1;
        };
    };
};
diag_log text format ["Dumped %1 vehicle classes", _vehCount];

diag_log text "=== ABE DUMP COMPLETE ===";
systemChat "ABE dump complete";

