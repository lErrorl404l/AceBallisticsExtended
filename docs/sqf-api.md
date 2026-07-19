# SQF API Reference

This page documents every extension command that SQF code can call via `callExtension`.
The extension has two calling conventions:

- **String mode** — `"extName" callExtension "command"` — for simple queries with no arguments.
- **Array mode** — `"extName" callExtension ["command", [arg1, arg2, ...]]` — for all physics operations.

All return values are **strings** formatted as SQF arrays (or `"-1"` on error).

## Table of Contents

- [Loading the Extension](#loading-the-extension)
- [String-Mode Commands](#string-mode-commands)
  - [version](#version)
  - [health](#health)
- [Array-Mode Commands](#array-mode-commands)
  - [init](#init)
  - [fire](#fire)
  - [step](#step)
  - [impact](#impact)
  - [wound](#wound)
  - [zeroing](#zeroing)
  - [shooter](#shooter)
  - [component](#component)
- [Error Handling](#error-handling)
- [Performance Notes](#performance-notes)

## Loading the Extension

Load the extension once at mission start or on each client's init. The extension binary
must be installed in the Arma 3 root directory or a path accessible to the game engine.

```sqf
// Load the extension and store a handle
ABE_extension = "abe_ballistics_ext";
```

All further calls use the same extension name string. There is no need to unload the
extension; Arma 3 manages the shared object lifetime.

A typical init sequence:

```sqf
// 1. Verify the extension is loadable
private _version = ABE_extension callExtension "version";

// 2. Initialise with API version and ACE3 detection
private _result = ABE_extension callExtension ["init", [1, 0]];

// 3. Confirm initialisation
if (_result != "0") then {
    diag_log "[ABE] Extension initialisation FAILED";
};
```

## String-Mode Commands

String-mode commands take no arguments. Use them for status queries and debugging.

### version

Returns the extension build version string.

```sqf
private _ver = "abe_ballistics_ext" callExtension "version";
// Returns: "1.0.0" (or whatever the current build version is)
```

### health

Returns `"1"` if the extension has been initialised and is ready, `"0"` otherwise.

```sqf
private _ready = "abe_ballistics_ext" callExtension "health";
// Returns: "1" (ready) or "0" (not initialised)
```

## Array-Mode Commands

All array-mode commands require the extension to be initialised (see [init](#init) above).
If called before init, they return `"-1"`.

### init

Initialise the extension with API version and ACE3 compatibility mode.

**Parameters:**

| Index | Name | Type | Description |
|-------|------|------|-------------|
| 0 | `api_version` | Number | Expected API version. Must match `ABE_API_VERSION` (currently `1`). |
| 1 | `ace_present` | Number | `1` if ACE3 is loaded, `0` otherwise. |

**Returns:** `"0"` on success, `"-1"` if API version mismatch.

**SQF:**

```sqf
private _result = "abe_ballistics_ext" callExtension [
    "init", [
        1,             // API version
        [ace_common] call ace_common_fnc_isModLoaded  // ACE3 present?
    ]
];
```

This must be called exactly once before any other array-mode command. The extension uses
a global OnceLock — calling `init` again has no effect.

### fire

Calculate muzzle velocity and interior ballistics for a given weapon configuration.

**Parameters:**

| Index | Name | Type | Unit | Description |
|-------|------|------|------|-------------|
| 0 | `barrel_length_mm` | Number | mm | Barrel length |
| 1 | `chamber_pressure_mpa` | Number | MPa | Maximum chamber pressure |
| 2 | `caliber_mm` | Number | mm | Bullet caliber |
| 3 | `projectile_mass_g` | Number | g | Projectile mass |
| 4 | `cdm_id` | String | — | Drag model ID (e.g. `"g7"`, `"m855"`) |

**Returns:** `[muzzle_velocity_ms, chamber_pressure_mpa, burn_fraction, barrel_time_ms]`
or `"-1"` if calculation fails (e.g. zero inputs).

| Index | Name | Unit | Description |
|-------|------|------|-------------|
| 0 | `muzzle_velocity_ms` | m/s | Calculated muzzle velocity |
| 1 | `chamber_pressure_mpa` | MPa | Maximum chamber pressure |
| 2 | `burn_fraction` | — | Fraction of propellant burned (0–1) |
| 3 | `barrel_time_ms` | ms | Time the projectile spends in the barrel |

**SQF:**

```sqf
private _params = [
    getNumber (configFile >> "CfgWeapons" >> _weapon >> "ABO_barrelLength"),
    getNumber (configFile >> "CfgWeapons" >> _weapon >> "ABO_chamberPressure"),
    getNumber (configFile >> "CfgAmmo" >> _ammo >> "caliber"),
    getNumber (configFile >> "CfgAmmo" >> _ammo >> "ABO_projectileMass"),
    getText   (configFile >> "CfgAmmo" >> _ammo >> "ABO_cdmId")
];

private _result = callExtension ["fire", _params];
_result = parseSimpleArray _result;  // -> [948.0, 380.0, 0.92, 1.6]
```

### step

Advance a bullet one time step through the atmosphere, applying drag, gravity, and wind.

**Parameters:**

| Index | Name | Type | Unit | Description |
|-------|------|------|------|-------------|
| 0 | `pos_x` | Number | m | World X position |
| 1 | `pos_y` | Number | m | World Y position |
| 2 | `pos_z` | Number | m | World Z position (height) |
| 3 | `vel_x` | Number | m/s | Velocity X component |
| 4 | `vel_y` | Number | m/s | Velocity Y component |
| 5 | `vel_z` | Number | m/s | Velocity Z component (up) |
| 6 | `dt_s` | Number | s | Time step (typically 0.001) |
| 7 | `wind_x` | Number | m/s | Wind X component |
| 8 | `wind_y` | Number | m/s | Wind Y component |
| 9 | `wind_z` | Number | m/s | Wind Z component (vertical) |
| 10 | `density` | Number | kg/m³ | Air density at bullet position |
| 11 | `temp_c` | Number | °C | Air temperature |
| 12 | `altitude_m` | Number | m | Altitude for auto-density calculation |
| 13 | `cdm_id` | String | — | Drag model ID |
| 14 | `bc` | Number | lb/in² | Ballistic coefficient (G1 reference) |
| 15 | `mass_g` | Number | g | Projectile mass (reserved, not used currently) |
| 16 | `caliber_mm` | Number | mm | Caliber (reserved, not used currently) |

**Returns:** `[new_pos_x, new_pos_y, new_pos_z, new_vel_x, new_vel_y, new_vel_z, new_mach, dt_s]`
or `"-1"` if not initialised.

| Index | Name | Unit | Description |
|-------|------|------|-------------|
| 0–2 | `new_pos_*` | m | New position after time step |
| 3–5 | `new_vel_*` | m/s | New velocity after time step |
| 6 | `new_mach` | — | Mach number at new velocity |
| 7 | `dt_s` | s | Confirmatory time step (echoed from input) |

**Air density resolution:** If `altitude_m > 0` and the temperature is within 0.1°C of
the ISA standard 15°C at sea level, the extension computes density from altitude using
the ICAO standard atmosphere. Otherwise `density` (param index 10) is used directly.
Pass `density = 1.225` and `altitude_m = 0` for sea-level standard conditions.

**SQF:**

```sqf
private _params = [
    _posX, _posY, _posZ,
    _velX, _velY, _velZ,
    0.001,                         // dt
    _windX, _windY, 0,             // wind
    1.225,                         // density
    15,                            // temp
    _altitude,
    _cdmId,
    _bc,
    _mass,
    _caliber
];

private _result = parseSimpleArray (callExtension ["step", _params]);
// _result = [x, y, z, vx, vy, vz, mach, dt]
```

### impact

Evaluate armor penetration and ricochet for a projectile hitting a target plate.

**Parameters:**

| Index | Name | Type | Unit | Description |
|-------|------|------|------|-------------|
| 0 | `vel_x` | Number | m/s | Velocity X component |
| 1 | `vel_y` | Number | m/s | Velocity Y component |
| 2 | `vel_z` | Number | m/s | Velocity Z component |
| 3 | `mass_g` | Number | g | Projectile mass |
| 4 | `caliber_mm` | Number | mm | Projectile caliber |
| 5 | `armor_thickness_mm` | Number | mm | Armor plate thickness |
| 6 | `armor_material` | String | — | Armor material ID (e.g. `"steel_rha"`, `"aluminum"`, `"ceramic"`) |
| 7 | `impact_angle_deg` | Number | deg | Impact angle from surface normal (0 = head-on) |
| 8 | `projectile_type` | String | — | Projectile type (`"ball"`, `"ap"`, `"apfsds"`, `"heat"`, etc.) |

**Returns:** `[penetrated, residual_velocity_ms, energy_j, effective_thickness_mm, ricochet, ricochet_angle_deg, ricochet_energy_fraction, fragments, spall_fragments]`
or `"-1"` if not initialised.

| Index | Name | Type | Unit | Description |
|-------|------|------|------|-------------|
| 0 | `penetrated` | Bool | — | `1` if projectile penetrated, `0` otherwise |
| 1 | `residual_velocity_ms` | Number | m/s | Velocity after penetrating armor |
| 2 | `energy_j` | Number | J | Impact kinetic energy |
| 3 | `effective_thickness_mm` | Number | mm | Effective armor thickness at given angle |
| 4 | `ricochet` | Bool | — | `1` if projectile ricocheted, `0` otherwise |
| 5 | `ricochet_angle_deg` | Number | deg | Ricochet deflection angle |
| 6 | `ricochet_energy_fraction` | Number | — | Fraction of kinetic energy retained after ricochet (0–1) |
| 7 | `fragments` | Number | — | Number of projectile fragments generated |
| 8 | `spall_fragments` | Number | — | Number of spall fragments from armor back-face |

**Armor material IDs:**

| ID | Description |
|----|-------------|
| `"steel_rha"` | Rolled homogeneous armor (baseline) |
| `"aluminum"` | Aluminum alloy (light armor) |
| `"ceramic"` | Ceramic strike face (composite armor) |
| `"concrete"` | Concrete / building material |
| `"glass"` | Bullet-resistant glass |

**SQF:**

```sqf
private _params = [
    _velX, _velY, _velZ,
    _massG,
    _caliberMM,
    _armorThicknessMM,
    "steel_rha",
    _impactAngleDeg,
    _projectileType
];

private _result = parseSimpleArray (callExtension ["impact", _params]);
// _result = [1, 520.0, 1800.0, 38.0, 0, 0, 1, 5, 12]
//            penetrated, residual v, energy, effective thickness, no ricochet, fragments
```

### wound

Evaluate soft-tissue wound ballistics for a projectile hitting a living target.

**Parameters:**

| Index | Name | Type | Unit | Description |
|-------|------|------|------|-------------|
| 0 | `vel_x` | Number | m/s | Velocity X component |
| 1 | `vel_y` | Number | m/s | Velocity Y component |
| 2 | `vel_z` | Number | m/s | Velocity Z component |
| 3 | `mass_g` | Number | g | Projectile mass |
| 4 | `caliber_mm` | Number | mm | Projectile caliber |
| 5 | `projectile_type` | String | — | Projectile type (`"ball"`, `"ap"`, etc.) |

**Returns:** `[penetration_depth_mm, perm_cavity_diameter_mm, temp_cavity_diameter_mm, energy_deposited_j, yawed]`
or `"-1"` if not initialised.

| Index | Name | Unit | Description |
|-------|------|------|-------------|
| 0 | `penetration_depth_mm` | mm | Wound channel penetration depth |
| 1 | `perm_cavity_diameter_mm` | mm | Permanent cavity diameter |
| 2 | `temp_cavity_diameter_mm` | mm | Temporary cavity diameter |
| 3 | `energy_deposited_j` | J | Energy deposited in tissue |
| 4 | `yawed` | Bool | `1` if projectile yawed/tumbled in tissue, `0` otherwise |

**SQF:**

```sqf
private _result = parseSimpleArray (callExtension ["wound", [
    _velX, _velY, _velZ, _massG, _caliberMM, _projType
]]);
// _result = [450.0, 15.0, 85.0, 1200.0, 1]
```

### zeroing

Calculate the optical zero (in MOA) required to hit a target at a given range.

**Parameters:**

| Index | Name | Type | Unit | Description |
|-------|------|------|------|-------------|
| 0 | `sight_height_mm` | Number | mm | Height of sight axis above bore axis |
| 1 | `zero_range_m` | Number | m | Desired zero range |
| 2 | `mv_ms` | Number | m/s | Muzzle velocity |

**Returns:** `[zero_moa]` or `"-1"` if calculation fails.

| Index | Name | Unit | Description |
|-------|------|------|-------------|
| 0 | `zero_moa` | MOA | Sight zero adjustment in minutes of angle |

**SQF:**

```sqf
private _result = parseSimpleArray (callExtension ["zeroing", [
    75,     // sight height (mm) — typical for AR with iron sights
    100,    // zero range (m)
    948     // muzzle velocity (m/s)
]]);
// _result = [2.3]  — 2.3 MOA adjustment
```

### shooter

Calculate shooter dispersion and hit probability based on stance, support, and experience.

**Parameters:**

| Index | Name | Type | Unit | Description |
|-------|------|------|------|-------------|
| 0 | `base_shooter_moa` | Number | MOA | Base shooter accuracy from weapon config |
| 1 | `stance` | String | — | `"standing"`, `"kneeling"`, `"prone"`, `"crouched"`, `"sitting"` |
| 2 | `support` | String | — | `"unsupported"`, `"bipod"`, `"tripod"`, `"sandbag"`, `"sling"`, `"vehicle"` |
| 3 | `heart_rate_bpm` | Number | bpm | Shooter heart rate (60–120 typical) |
| 4 | `breath` | String | — | `"normal"`, `"hold"`, `"heavy"` |
| 5 | `experience` | String | — | `"novice"`, `"intermediate"`, `"advanced"`, `"expert"`, `"precision"` |
| 6 | `range_m` | Number | m | Target range in meters |

**Returns:** `[shooter_moa, sigma_m, hit_probability]` or `"-1"` if not initialised.

| Index | Name | Unit | Description |
|-------|------|------|-------------|
| 0 | `shooter_moa` | MOA | Total shooter dispersion in MOA |
| 1 | `sigma_m` | m | Standard deviation of impact distribution at target range |
| 2 | `hit_probability` | — | Probability of hitting a 50 cm circular target (0–1) |

**SQF:**

```sqf
private _result = parseSimpleArray (callExtension ["shooter", [
    1.5,         // base weapon accuracy in MOA
    "prone",     // stance
    "bipod",     // support
    72,          // heart rate (bpm)
    "hold",      // breath phase
    "advanced",  // shooter experience
    300          // range (m)
]]);
// _result = [0.85, 0.074, 0.65]
```

### component

Evaluate vehicle component kill probability after a penetrating hit.

**Parameters:**

| Index | Name | Type | Unit | Description |
|-------|------|------|------|-------------|
| 0 | `vehicle_type` | String | — | `"mbt"`, `"ifv"`, `"apc"`, `"truck"`, `"helicopter"`, `"light"` |
| 1 | `hit_zone` | String | — | `"front"`, `"side"`, `"rear"`, `"top"`, `"bottom"` |
| 2 | `caliber_mm` | Number | mm | Projectile caliber |
| 3 | `mass_g` | Number | g | Projectile mass |
| 4 | `velocity_ms` | Number | m/s | Impact velocity |
| 5 | `projectile_type` | String | — | `"ball"`, `"ap"`, `"apfsds"`, `"heat"`, `"incendiary"`, `"tracer"` |
| 6 | `impact_angle_deg` | Number | deg | Impact angle from surface normal |
| 7 | `residual_velocity_ms` | Number | m/s | Velocity after penetrating armor |
| 8 | `armor_penetrated` | Bool | — | `1` if armor was penetrated, `0` otherwise |

**Returns:** `[mobility_kill_probability, firepower_kill_probability, catastrophic_kill_probability]`
or `"-1"` if not initialised.

| Index | Name | Description |
|-------|------|-------------|
| 0 | `mobility_kill_probability` | Probability of mobility kill (engine, transmission, tracks) |
| 1 | `firepower_kill_probability` | Probability of firepower kill (turret, optics, ammunition) |
| 2 | `catastrophic_kill_probability` | Probability of catastrophic kill (ammo cook-off, fuel explosion) |

**SQF:**

```sqf
private _result = parseSimpleArray (callExtension ["component", [
    "mbt",          // vehicle type
    "front",        // hit zone
    30,             // caliber (mm)
    400,            // projectile mass (g)
    1600,           // impact velocity (m/s)
    "apfsds",       // projectile type
    10,             // impact angle (deg)
    1200,           // residual velocity (m/s)
    1               // armor penetrated
]]);
// _result = [0.35, 0.28, 0.15]
```

## Error Handling

All array-mode commands follow the same error convention:

| Return Value | Meaning |
|---|---|
| `"-1"` | Extension not initialised, or calculation failed |
| `"unknown: COMMAND"` | Unrecognised command name |

If the extension is not initialised, all array-mode commands return `"-1"`. Check the
return of `init` and/or use the health string-mode command before relying on results.

Parsing in SQF:

```sqf
private _raw = "abe_ballistics_ext" callExtension ["fire", _params];
if (_raw == "-1") exitWith {
    diag_log "[ABE] Fire calculation failed";
};

// Safe to parse
private _result = parseSimpleArray _raw;
```

## Performance Notes

- **`step`** is the performance-critical command. At 0.001 s step rate with 50 tracked
  bullets, you issue 50 `callExtension` calls per frame. Keep the SQF iteration loop
  tight — minimise work between step calls.
- **`init`**, **`fire`**, and **`impact`** are one-shot and performance-irrelevant.
- **`shooter`** can be called once per shot event, cached for the weapon.
- **`component`** is called once per penetrating hit — negligible overhead.
- All commands execute in under 0.01 ms in the extension. `callExtension` overhead
  (argument serialisation + FFI crossing) dominates; batch calls if the engine supports
  it in a future iteration.
