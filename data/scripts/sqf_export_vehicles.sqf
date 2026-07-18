/*
 * SQF Export: CfgVehicles (armed) → clipboard
 * Paste into Eden Debug Console, Execute, then paste clipboard contents back.
 * Format: V|<configName>|<displayName>|<armor>|<hitpointName1>:<armor1>,<hitpointName2>:<armor2>,...
 *
 * Exports vehicles that have HitPoints (all damageable vehicles including cars, tanks, etc.)
 * Post-filter in Python for turreted vehicles.
 */

private _output = [];
private _cfgVeh = configFile >> "CfgVehicles";
private _count = count _cfgVeh;

for "_i" from 1 to _count do {
    private _entry = _cfgVeh select (_i - 1);
    if (!isClass _entry) then { continue; };
    
    private _scope = getNumber (_entry >> "scope");
    if (_scope < 2) then { continue; }; // public only
    
    private _name = configName _entry;
    private _displayName = getText (_entry >> "displayName");
    if (_displayName == "") then { continue; };
    
    // Skip pure infantry (man) units
    if (isKindOf [_name, "Man"]) then { continue; };
    
    private _cfgHitPoints = _entry >> "HitPoints";
    if (!isClass _cfgHitPoints) then { continue; };
    
    private _hpCount = count _cfgHitPoints;
    if (_hpCount == 0) then { continue; };
    
    private _armor = getNumber (_entry >> "armor");
    
    // Get parent class
    private _parent = inheritsFrom _entry;
    private _baseName = "";
    if (!isNull _parent) then { _baseName = configName _parent; };
    
    // Build hitpoint string
    private _hpParts = [];
    for "_j" from 1 to _hpCount do {
        private _hp = _cfgHitPoints select (_j - 1);
        if (isClass _hp) then {
            private _hpName = configName _hp;
            private _hpArmor = getNumber (_hp >> "armor");
            private _hpMat = getNumber (_hp >> "material");
            private _hpPassthrough = getNumber (_hp >> "passthrough");
            _hpParts pushBack format ["%1=%2=%3=%4", _hpName, _hpArmor, _hpMat, _hpPassthrough];
        };
    };
    
    _output pushBack format ["V|%1|%2|%3|%4|%5|%6|%7", _name, _displayName, _armor, _baseName, (_hpParts joinString "~"), _hpCount];
};

copyToClipboard (_output joinString endl);
systemChat format ["Exported %1 vehicles with hitpoints", count _output];
