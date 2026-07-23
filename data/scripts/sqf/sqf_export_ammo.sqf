/*
 * SQF Export: CfgAmmo → clipboard
 * Paste into Eden Debug Console, Execute, then paste clipboard contents back.
 * Format: A|<configName>|<displayName>|<hit>|<caliber>|<typicalSpeed>|<airFriction>|<timeToLive>|<simulation>|<baseClass>
 */

private _output = [];
private _cfgAmmo = configFile >> "CfgAmmo";
private _count = count _cfgAmmo;

for "_i" from 1 to _count do {
    private _entry = _cfgAmmo select (_i - 1);
    if (!isClass _entry) then { continue; };
    
    private _scope = getNumber (_entry >> "scope");
    if (_scope < 1) then { continue; };
    
    private _name = configName _entry;
    private _displayName = getText (_entry >> "displayName");
    if (_displayName == "") then { continue; };
    
    private _hit = getNumber (_entry >> "hit");
    private _caliber = getNumber (_entry >> "caliber");
    private _typicalSpeed = getNumber (_entry >> "typicalSpeed");
    private _airFriction = getNumber (_entry >> "airFriction");
    private _timeToLive = getNumber (_entry >> "timeToLive");
    private _simulation = getText (_entry >> "simulation");
    
    // Get base class
    private _parent = inheritsFrom _entry;
    private _baseName = "";
    if (!isNull _parent) then { _baseName = configName _parent; };
    
    _output pushBack format ["A|%1|%2|%3|%4|%5|%6|%7|%8|%9", _name, _displayName, _hit, _caliber, _typicalSpeed, _airFriction, _timeToLive, _simulation, _baseName];
};

copyToClipboard (_output joinString endl);
systemChat format ["Exported %1 ammo types", count _output];
