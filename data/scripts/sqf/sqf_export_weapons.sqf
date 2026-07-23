/* 
 * SQF Export: CfgWeapons → clipboard
 * Paste into Eden Debug Console, Execute, then paste clipboard contents back.
 * Exports all public ballistic weapons with barrel length and compatible magazines.
 * Format: W|<configName>|<displayName>|<modelLength_m>|<mag1,mag2,...>|<baseClass>
 */

private _output = [];
private _cfgWeapons = configFile >> "CfgWeapons";
private _count = count _cfgWeapons;
private _skippedNoMag = 0;
private _skippedNoDisp = 0;

for "_i" from 1 to _count do {
    private _entry = _cfgWeapons select (_i - 1);
    if (!isClass _entry) then { continue; };
    
    private _scope = getNumber (_entry >> "scope");
    if (_scope < 1) then { continue; }; // hide/protected
    
    private _name = configName _entry;
    private _displayName = getText (_entry >> "displayName");
    if (_displayName == "") then { _skippedNoDisp = _skippedNoDisp + 1; continue; };
    
    private _magazines = getArray (_entry >> "magazines");
    if (count _magazines == 0) then { _skippedNoMag = _skippedNoMag + 1; continue; };
    
    private _modelLength = getNumber (_entry >> "modelLength");
    private _initSpeed = getNumber (_entry >> "initSpeed");
    
    // Get base class
    private _parent = inheritsFrom _entry;
    private _baseName = "";
    if (!isNull _parent) then { _baseName = configName _parent; };
    
    // Build mag list (limit to first 20 to keep output manageable)
    private _magStr = "";
    {
        if (_forEachIndex > 0) then { _magStr = _magStr + ","; };
        _magStr = _magStr + _x;
        if (_forEachIndex >= 19) exitWith {};
    } forEach _magazines;
    
    _output pushBack format ["W|%1|%2|%3|%4|%5|%6", _name, _displayName, _modelLength, _initSpeed, _magStr, _baseName];
};

copyToClipboard (_output joinString endl);
systemChat format ["Exported %1 weapons. Skipped: %2 no display, %3 no magazines", count _output, _skippedNoDisp, _skippedNoMag];
