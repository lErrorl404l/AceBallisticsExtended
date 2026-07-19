// ABE - Advanced Ballistics Extension
// CBA Settings — see https://github.com/CBATeam/CBA_A3/wiki/CBA-Settings-System

// Enable/disable ABE ballistics simulation entirely
// 0: Disabled (uses Arma 3 default/ACE3 ballistics)
// 1: Enabled (default)
force ace_advanced_ballistics_enable = true;

// ---- Projectile physics ----

// Wind effects on projectiles (crosswind drift, headwind/tailwind)
// 0: Disabled, 1: Enabled (default)
force abe_setting_wind_enabled = 1;

// Coriolis and Eötvös effect (Earth rotation drift)
// 0: Disabled, 1: Enabled (default)
// Only significant above ~800 m range
force abe_setting_coriolis_enabled = 1;

// Temperature effects on muzzle velocity
// 0: Disabled, 1: Enabled (default)
// Affects propellant burn rate based on ambient temperature
force abe_setting_temperature_effects = 1;

// ---- Terminal ballistics ----

// Ricochet behaviour
// 0: Disabled (projectile always penetrates or stops)
// 1: Enabled (default) — angle-dependent ricochet with energy retention
force abe_setting_ricochet_enabled = 1;

// Penetration calculation quality
// 0: Standard — De Marre formula, fast
// 1: Detailed — adds yaw effects, layered armour modelling (default)
force abe_setting_penetration_detail = 1;

// ---- Debug ----

// Enable debug logging (performance impact when enabled)
// 0: Disabled (default), 1: Enabled
force abe_setting_debug = 0;
