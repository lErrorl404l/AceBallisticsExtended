/*
 * SQF Export: CfgMagazines → clipboard
 * Paste into Eden Debug Console, Execute, then paste clipboard contents back.
 * Format: M|<configName>|<displayName>|<initSpeed>|<ammo>|<count>|<baseClass>
 */

private _output = [];
private _cfgMags = configFile >> "CfgMagazines";
private _count = count _cfgMags;

for "_i" from 1 to _count do {
    private _entry = _cfgMags select (_i - 1);
    if (!isClass _entry) then { continue; };
    
    private _scope = getNumber (_entry >> "scope");
    if (_scope < 1) then { continue; };
    
    private _name = configName _entry;
    private _displayName = getText (_entry >> "displayName");
    if (_displayName == "") then { continue; };
    
    private _initSpeed = getNumber (_entry >> "initSpeed");
    private _ammo = getText (_entry >> "ammo");
    private _count_mag = getNumber (_entry >> "count");
    
    // Get base class
    private _parent = inheritsFrom _entry;
    private _baseName = "";
    if (!isNull _parent) then { _baseName = configName _parent; };
    
    _output pushBack format ["M|%1|%2|%3|%4|%5|%6", _name, _displayName, _initSpeed, _ammo, _count_mag, _baseName];
};

copyToClipboard (_output joinString endl);
systemChat format ["Exported %1 magazines", count _output];
