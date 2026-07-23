# ABE Environment API

The environment system gives mission designers full control over atmospheric
conditions affecting bullet trajectories. When the environment is set, the
solver uses non-ISA density, wind profiles with direction and gusts,
precipitation drag modifiers, and temperature-dependent muzzle velocity.

## Commands

### `weather` — Set environment parameters

```
["weather", [
    delta_temp_c,       // Temperature offset from ISA (15°C), range [-30, 30]
    humidity_pct,       // Relative humidity, 0–100%
    delta_pressure_pct, // Pressure offset from ISA (1013.25 hPa), %
    wind_speed_ms,      // Surface wind speed, m/s
    wind_direction_deg, // Wind direction (meteorological, 0=from N), 0–360
    gust_intensity,     // Gust/turbulence intensity, 0–1.0
    rain_mm_per_hour,   // Precipitation rate, 0–100 mm/hr
    is_snowfall,        // 0=rain, 1=snow
    powder_sens,        // Powder temp sensitivity, %/°C, default 0.15
    ambient_temp_c,     // Ambient air temperature, °C
    cloud_base_m        // Cloud base altitude, metres
]]
```

Returns `"1"` on success, `"-1"` if not initialized.

**Example — Arid desert conditions:**

```sqf
["abe_ballistics_ext", "weather", [
    15,     // +15°C above ISA
    10,     // 10% humidity
    -5,     // -5% pressure (hot, high altitude)
    10,     // 10 m/s wind
    270,    // from the west
    0.3,    // light gusting
    0,      // no rain
    0,      // no snow
    0.15,   // standard powder sensitivity
    40,     // 40°C ambient
    500     // 500m cloud base
]]
```

**Example — Arctic blizzard:**

```sqf
["abe_ballistics_ext", "weather", [
    -20,    // -20°C below ISA
    80,     // 80% humidity
    5,      // +5% pressure (cold, dense air)
    15,     // 15 m/s wind
    180,    // from the south
    0.6,    // heavy gusting
    10,     // 10 mm/hr precipitation
    1,      // snow
    0.15,   // standard powder sensitivity
    -10,    // -10°C ambient
    200     // 200m cloud base
]]
```

### `env_query` — Read current environment

```
["abe_ballistics_ext", "env_query", []]
```

Returns `"none"` if no environment is set, or a 17-element array:

```
[delta_temp, humidity, delta_pressure, wind_speed, wind_dir,
 profile_exponent, ref_height_m, gust_intensity, turb_scale_m,
 gust_amp_ms, rain_mmhr, snow_flag, cloud_base_m, temp_c,
 powder_sens, "ABE EnvironmentParams", "live"]
```

### `env_reset` — Clear environment

```
["abe_ballistics_ext", "env_reset", []]
```

Returns `"1"`. After reset, the solver falls back to per-step parameters
(density, temperature, wind passed directly in step args).

## How the Environment Affects Ballistics

### Density
When environment is set, the solver computes air density from
temperature, humidity, and pressure offsets using the full non-ISA
atmosphere model. Lower density = less drag = less drop.

| Condition | Effect on drop (800m, M80) |
|-----------|---------------------------|
| Standard (15°C, 50% RH) | baseline |
| Hot desert (40°C, 10% RH) | −7.5% |
| Arctic (−10°C, 80% RH) | +11.5% |
| High altitude (4000m) | −15.4% |

### Wind
Wind is coupled through the drag equation using the bullet's velocity
**relative to the moving air**. This means:
- Wind pushes the bullet in the same direction the air is moving
- Wind has zero effect in vacuum (correct physics)
- A low-level wind profile is applied based on altitude

### Precipitation
Rain adds a penalty to the ballistic coefficient, increasing drag.
Heavier rain → more drag → more drop. Snow reduces the penalty
slightly compared to rain at the same intensity.

### Temperature-dependent muzzle velocity
Powder burn rate changes with temperature. The formula uses:
```
MV_actual = MV_ref × (1 + sensitivity × (T_ambient − T_ref))
```
Default sensitivity: 0.15%/°C. At 40°C, MV increases ~3% vs 15°C.

## Integration Notes

The SQF step handler (`fn_step.sqf`) automatically syncs the game's
weather to the extension every 30 seconds. If ACE3 weather is present,
it uses ACE3 values for temperature, humidity, pressure, and gusts.
Otherwise it falls back to ARMA's built-in wind and overcast values.

To disable automatic weather sync, set `GVAR(lastWeatherSync)` to a
large value in init:

```sqf
missionNamespace setVariable ["ABE_lastWeatherSync", 1e9];
```
