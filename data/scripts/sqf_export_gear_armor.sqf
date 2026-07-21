/*
 * SQF Export: CfgWeapons wearable armor (vests, helmets, uniforms, glasses) → clipboard
 * Paste into Eden Debug Console, Execute, then paste clipboard contents back.
 * Format: G|<configName>|<displayName>|<type>|<baseClass>|<uniformClass>|<hitpoint1:armor:passThrough>,<hitpoint2:armor:passThrough>,...
 *
 * Exports ALL public wearable items that have HitpointsProtectionInfo:
 *   - VestItem (vests)
 *   - HeadgearItem (helmets)
 *   - UniformItem (uniforms)
 *   - GlassesItem (glasses/goggles)
 *
 * Uniform armor is chain-resolved through uniformClass → CfgVehicles → HitPoints.
 */

private _output = [];
private _cfgWeapons = configFile >> "CfgWeapons";
private _count = count _cfgWeapons;
private _exportedTypes = ["VestItem", "HeadgearItem", "UniformItem", "GlassesItem"];

for "_i" from 1 to _count do {
    private _entry = _cfgWeapons select (_i - 1);
    if (!isClass _entry) then { continue; };

    private _scope = getNumber (_entry >> "scope");
    if (_scope < 1) then { continue; }; // hidden/protected

    private _name = configName _entry;
    private _displayName = getText (_entry >> "displayName");
    if (_displayName == "") then { continue; };

    private _parent = inheritsFrom _entry;
    private _baseName = "";
    if (!isNull _parent) then { _baseName = configName _parent; };

    // Determine item type by checking inheritance chain
    private _itemType = "";
    {
        if (_entry isKindOf [_x, configFile >> "CfgWeapons"]) exitWith {
            _itemType = _x;
        };
    } forEach _exportedTypes;

    if (_itemType == "") then { continue; };

    private _itemInfo = _entry >> "ItemInfo";
    if (!isClass _itemInfo) then { continue; };

    private _itemArmor = getNumber (_itemInfo >> "armor");
    private _itemPassThrough = getNumber (_itemInfo >> "passThrough");

    // Build hitpoint string from HitpointsProtectionInfo
    private _hpParts = [];
    private _hpInfo = _itemInfo >> "HitpointsProtectionInfo";

    if (isClass _hpInfo) then {
        private _hpCount = count _hpInfo;
        for "_j" from 1 to _hpCount do {
            private _hp = _hpInfo select (_j - 1);
            if (isClass _hp) then {
                private _hpName = configName _hp;
                private _hpArmor = getNumber (_hp >> "armor");
                private _hpPassThrough = getNumber (_hp >> "passThrough");
                private _hitpointName = getText (_hp >> "hitpointName");
                _hpParts pushBack format ["%1:%2:%3:%4", _hpName, _hpArmor, _hpPassThrough, _hitpointName];
            };
        };
    };

    // For uniforms: chain-resolve through uniformClass → CfgVehicles → HitPoints
    private _uniformClass = "";
    private _uniformHitPoints = [];
    if (_itemType == "UniformItem") then {
        _uniformClass = getText (_itemInfo >> "uniformClass");
        if (_uniformClass != "") then {
            private _cfgUnit = configFile >> "CfgVehicles" >> _uniformClass;
            if (isClass _cfgUnit) then {
                private _cfgUnitHP = _cfgUnit >> "HitPoints";
                if (isClass _cfgUnitHP) then {
                    private _hpCount = count _cfgUnitHP;
                    for "_j" from 1 to _hpCount do {
                        private _hp = _cfgUnitHP select (_j - 1);
                        if (isClass _hp) then {
                            private _hpName = configName _hp;
                            private _hpArmor = getNumber (_hp >> "armor");
                            _uniformHitPoints pushBack format ["%1:%2", _hpName, _hpArmor];
                        };
                    };
                };
            };
        };
    };

    _output pushBack format [
        "G|%1|%2|%3|%4|%5|%6|%7|%8",
        _name,
        _displayName,
        _itemType,
        _baseName,
        _itemArmor,
        _itemPassThrough,
        (_hpParts joinString "~"),
        _uniformClass + "|" + (_uniformHitPoints joinString "~")
    ];
};

copyToClipboard (_output joinString endl);
systemChat format ["Exported %1 gear items with HitpointsProtectionInfo", count _output];
