# Module Guide

ABE is organized into discrete addon modules (PBOs), each responsible for one ballistics
domain. This page documents every module, its dependencies, configuration options, and
what it does.

## Table of Contents

- [Module Overview](#module-overview)
- [abo_core](#abo_core)
- [abo_interior](#abo_interior)
- [abo_external](#abo_external)
- [abo_terminal](#abo_terminal)
- [abo_penetration](#abo_penetration)
- [abo_ricochet](#abo_ricochet)
- [abo_armor](#abo_armor)
- [abo_bad](#abo_bad)
- [abo_damage](#abo_damage)
- [abo_environment](#abo_environment)
- [abo_fcs](#abo_fcs)
- [abo_degradation](#abo_degradation)
- [abo_ace3](#abo_ace3)
- [compat Modules](#compat-modules)

## Module Overview

All modules are installed under `@abe/addons/`. They follow the naming convention
`abo_<domain>` for ABE modules and `compat_<source>` for compatibility data packs. The
module dependency graph is rooted at `abo_core`, which every other module requires.

Modules are toggleable: server hosts can enable or disable individual PBOs to control
which ballistics features are active. Disabling a module removes its computation and
SQF overhead entirely.

## abo_core

**Dependencies:** None

**Role:** Foundation module. Loads the extension DLL, verifies API version compatibility,
provides logging infrastructure, defines common macros and constants used by all other
modules.

**Functions:**

- Extension loader: calls `abe_init` on the extension, checks return status, handles
  version negotiation.
- Logging: structured log output to RPT with configurable verbosity levels (ERROR, WARN,
  INFO, DEBUG).
- Config parser: reads JSON data tables from the mod's config namespace and caches them
  for lookup by other modules.
- Common macros: coordinate transforms, unit conversions (mils to degrees, meters to
  feet, Celsius to Kelvin), math constants.

**Configuration:**

| Setting | Type | Default | Description |
|---|---|---|---|
| `ABO_logLevel` | Number | 1 | Log verbosity: 0=ERROR, 1=WARN, 2=INFO, 3=DEBUG |
| `ABO_maxTrackedBullets` | Number | 50 | Maximum bullets tracked per frame |
| `ABO_extensionPath` | String | auto | Override path to extension binary |

## abo_interior

**Dependencies:** abo_core

**Role:** Interior ballistics. Calculates muzzle velocity from barrel length, chamber
pressure, and propellant model. Handles velocity scaling when a weapon's barrel length
differs from the reference data table value.

**Key computations:**

- Pressure-integral approximation for barrel length to MV scaling.
- Propellant burn rate model (simplified Cornell University model).
- Velocity loss for shortened barrels.
- Charge temperature effects on muzzle velocity.

**Configuration:**

| Setting | Type | Default | Description |
|---|---|---|---|
| `ABO_tempSensitivity` | Number | 0.5 | MV change per 10C ambient temp change (percent) |
| `ABO_useChargeTemp` | Bool | true | Apply charge temperature effects |

**ACE3 integration:** When ACE3 is loaded, `abo_interior` enriches ACE3's `handleFire`
event with ABE-calculated muzzle velocity. Without ACE3, it fires a custom `ABO_fire`
event consumed by downstream modules.

## abo_external

**Dependencies:** abo_interior, abo_environment

**Role:** External ballistics. Computes the bullet's trajectory through the air,
accounting for drag, wind, air density, and gravity. Steps the bullet forward on each
frame.

**Key computations:**

- Custom CDM drag tables (Mach-vs-drag for projectile families).
- G1/G7/G8 standard drag curve implementation.
- Transonic drag divergence model.
- BC-vs-Mach scaling (velocity-dependent ballistic coefficient).
- Coriolis effect, spin drift, and Magnus force.
- Wind gradient model (wind speed varies with altitude).

**Configuration:**

| Setting | Type | Default | Description |
|---|---|---|---|
| `ABO_stepRate` | Number | 0.001 | Integration step size in seconds |
| `ABO_substeps` | Number | 4 | Sub-step iterations per frame for accuracy |
| `ABO_windGradient` | Bool | true | Enable wind gradient modeling |
| `ABO_coriolis` | Bool | true | Enable Coriolis effect |
| `ABO_spinDrift` | Bool | true | Enable spin drift |
| `ABO_dragModel` | String | `custom` | Drag model: `custom`, `g1`, `g7`, `g8` |

## abo_terminal

**Dependencies:** abo_external

**Role:** Terminal ballistics. Models what happens when a projectile enters a target:
fragmentation, yaw and tumbling, temporary cavity formation.

**Key computations:**

- Fragmentation model: mass distribution, spray pattern, velocity dependence.
- Yaw-growth differential equation for tumbling in tissue.
- Temporary cavity modeling (simplified pressure field).
- Velocity threshold for fragmentation onset.
- Fragment spray cone angle calculation.

**Configuration:**

| Setting | Type | Default | Description |
|---|---|---|---|
| `ABO_fragmentation` | Bool | true | Enable fragmentation model |
| `ABO_yawGrowth` | Bool | true | Enable yaw/tumbling model |
| `ABO_tempCavity` | Bool | true | Enable temporary cavity pressure model |

## abo_penetration

**Dependencies:** abo_core

**Role:** Penetration math. Calculates whether a projectile penetrates a given armor
plate and how much velocity it retains after penetration.

**Key computations:**

- De Marre penetration formula with material-specific modifiers.
- Lanz-Odermatt (L-O) formula for long-rod penetrators.
- Angle-dependent penetration curves with non-linear falloff.
- Caliber-to-thickness ratio effects (overmatching thin plates).
- Overmatch mechanics (projectile defeats armor by being much larger than the
  plate is thick).
- Residual velocity and mass after penetration.

**Configuration:**

| Setting | Type | Default | Description |
|---|---|---|---|
| `ABO_penetrationModel` | String | `de_marre` | Penetration formula: `de_marre`, `lanz_odermatt` |
| `ABO_overmatch` | Bool | true | Enable overmatch mechanics |
| `ABO_caliberRatio` | Bool | true | Enable caliber-to-thickness ratio effects |

## abo_ricochet

**Dependencies:** abo_penetration

**Role:** Ricochet physics. Models projectiles that fail to penetrate and instead ricochet
off the armor surface.

**Key computations:**

- Ricochet threshold angle versus projectile shape and velocity.
- Energy-retaining ricochet: projectile continues with reduced velocity after deflection.
- Skip, bounce, and tumble ricochet outcomes.
- Surface geometry interaction (sloped armor promotes ricochet).
- Secondary impact calculation after ricochet.

**Configuration:**

| Setting | Type | Default | Description |
|---|---|---|---|
| `ABO_ricochetRetain` | Number | 0.6 | Fraction of velocity retained after ricochet |
| `ABO_ricochetSpread` | Number | 5 | Angular spread in degrees for ricochet direction |

## abo_armor

**Dependencies:** abo_penetration

**Role:** Armor array modeling. Evaluates complex armor configurations including ERA,
composite arrays, spaced armor, and spall liners.

**Key computations:**

- Layered armor array evaluation (sequential layer penetration).
- ERA disruption: ERA tiles detonate and disrupt the projectile before it reaches
  the main armor.
- Composite armor: ceramic strike face shatters the projectile, backing catches fragments.
- Spaced armor: air gap between plates reduces penetration capability.
- Multi-hit armor degradation: cumulative damage tracking.
- Angle-of-attack calculation for complex geometry.

**Configuration:**

| Setting | Type | Default | Description |
|---|---|---|---|
| `ABO_eraEffective` | Bool | true | Enable ERA effectiveness modeling |
| `ABO_multiHit` | Bool | true | Enable multi-hit armor degradation |
| `ABO_spallLiners` | Bool | true | Enable spall liner catch probability |

## abo_bad

**Dependencies:** abo_armor

**Role:** Behind-armor debris (BAD). Generates secondary fragments from penetrated armor
plates and spalling from non-penetrated plates.

**Key computations:**

- Spall fragment generation from non-penetrated armor (back-face spalling).
- Behind-armor debris fragment generation from penetrated armor.
- Fragment velocity and mass distribution.
- Fragment spray cone inside the vehicle crew compartment.
- Fragment energy attenuation through interior air and equipment.

**Configuration:**

| Setting | Type | Default | Description |
|---|---|---|---|
| `ABO_spallFragments` | Integer | 20 | Average number of spall fragments generated |
| `ABO_badFragments` | Integer | 15 | Average number of BAD fragments generated |
| `ABO_spallVelocity` | Number | 0.3 | Spall fragment velocity as fraction of impact velocity |

## abo_damage

**Dependencies:** abo_terminal, abo_bad

**Role:** Damage application. Converts ballistic events into game damage. Routes
fragment damage and penetrator damage through appropriate channels.

**Key computations:**

- Yaw-dependent wounding: bullet that yaws causes more tissue damage than one that
  passes straight through.
- Fragment damage vs penetrator damage: separate damage channels with different
  wound profiles.
- Temporary cavity damage (tissue crush from pressure wave).
- Damage aggregation from multiple fragments.
- Integration with ACE3 medical system or standalone damage application.

**Configuration:**

| Setting | Type | Default | Description |
|---|---|---|---|
| `ABO_damageMultiplier` | Number | 1.0 | Global damage scaling factor |
| `ABO_fragmentDamage` | Bool | true | Enable fragment damage channel |
| `ABO_yawWounding` | Bool | true | Enable yaw-dependent wounding multiplier |

## abo_environment

**Dependencies:** abo_core (independent)

**Role:** Environmental effects on ballistics. Models the atmosphere: temperature lapse
rates, air density gradients, humidity effects, and dynamic weather.

**Key computations:**

- ICAO/ISA standard atmosphere with altitude lapse rates.
- Air density calculation from temperature, pressure, and humidity.
- Wind profile: wind speed and direction as a function of altitude.
- Dynamic weather transitions (density changes from weather systems).
- Light-level effects on zeroing (twilight refraction).

**Configuration:**

| Setting | Type | Default | Description |
|---|---|---|---|
| `ABO_weatherDensity` | Bool | true | Enable dynamic weather density changes |
| `ABO_temperatureLapse` | Bool | true | Enable temperature altitude lapse rate |
| `ABO_humidity` | Bool | false | Enable humidity effects (minimal impact, off by default) |

**Note:** This module is independent and can be disabled without affecting other
ballistics modules. When disabled, standard ISA sea-level conditions are assumed.

## abo_fcs

**Dependencies:** abo_external, abo_environment

**Role:** Fire control system. Provides ballistic reticle solutions, cant error
compensation, and altitude zero correction.

**Key computations:**

- Ballistic reticle aiming solution (holdover in mils/MOA).
- Cant error: crosswind error from weapon cant angle.
- Altitude zero correction: zero changes with altitude due to air density.
- Target ranging from bullet drop (if no laser rangefinder available).
- Lead calculation for moving targets.

**Configuration:**

| Setting | Type | Default | Description |
|---|---|---|---|
| `ABO_cantCorrection` | Bool | true | Enable cant error compensation |
| `ABO_altitudeZero` | Bool | true | Enable altitude zero correction |
| `ABO_reticleUnit` | String | `mil` | Reticle unit: `mil`, `moa` |

## abo_degradation

**Dependencies:** abo_core (independent)

**Role:** Barrel degradation over sustained fire. Models barrel heat buildup, fouling
accumulation, erosion from round count, and zeroing drift from thermal effects.

**Key computations:**

- Barrel heat model: round count and firing rate drive temperature increase.
- Barrel cooling: convection and conduction cooling between shots.
- Fouling model: accuracy degradation from powder residue buildup.
- Erosion model: throat erosion causes velocity loss over barrel life.
- Zeroing drift: thermal effects shift point of impact as barrel heats.
- Accuracy dispersion increase from fouling and heat.

**Configuration:**

| Setting | Type | Default | Description |
|---|---|---|---|
| `ABO_barrelHeat` | Bool | true | Enable barrel heat modeling |
| `ABO_barrelFouling` | Bool | true | Enable barrel fouling |
| `ABO_barrelErosion` | Bool | true | Enable barrel erosion |
| `ABO_barrelLife` | Integer | 10000 | Estimated barrel life in rounds for erosion model |
| `ABO_heatDispersion` | Number | 0.1 | Dispersion increase in mils at max heat |

**Note:** This module is independent and can be disabled or enabled separately from all
other ballistics modules. It is the most performance-light module (no per-bullet
computation, only per-shot state updates).

## abo_ace3

**Dependencies:** abo_core

**Role:** ACE3 integration layer. Detects whether ACE3 is loaded and dispatches ABE
ballistics through ACE3's bullet tracking or ABE's standalone framework.

**Key behaviors:**

- **ACE3 present.** Hooks into ACE3's bullet tracking system. Intercepts
  `ace_advanced_ballistics` events and enriches them with ABE data. ACE3 manages the
  per-frame bullet iteration; ABE provides physics results.
- **ACE3 absent.** Activates the standalone SQF fire control layer. Registers a
  per-frame handler that iterates over tracked bullets, calls the extension for each
  step, and applies results. No ACE3 dependency required.

**Configuration:**

| Setting | Type | Default | Description |
|---|---|---|---|
| `ABO_ace3Integration` | Bool | true | Enable ACE3 hook integration if ACE3 is loaded |
| `ABO_standaloneFallback` | Bool | true | Enable standalone mode when ACE3 is absent |

**Note:** This module does not contain ballistics logic itself. It is the dispatch
router. All physics computation happens in the modules above; this module only
determines how the results are delivered to the game engine.

## compat Modules

Compatibility data packs provide weapon, ammo, and armor data tables for specific mod
sets. They depend on abo_ace3 and contain no code, only JSON configuration files and
CfgPatches class definitions.

| Module | Mod | Scope |
|---|---|---|
| `compat_rhs` | RHS Escalation / AFRF | 100+ weapons, 50+ vehicles |
| `compat_cup` | CUP Weapons / Vehicles | 150+ weapons, 80+ vehicles |
| `compat_niarms` | NIArms | 80+ weapons |
| `compat_ww2` | IFA / FOW | WWII weapons and vehicles |

Adding a new compatibility pack follows the same pattern: create a new `compat_<name>`
PBO with CfgPatches, JSON data tables, and no executable code. See the
[Data Format](data-format.md) page for the schema reference.

### Adding a Compatibility Pack

```yaml
# Template for a new compat module
# addons/compat_my_mod/config.cpp
class CfgPatches {
    class abo_compat_mymod {
        units[] = {};
        weapons[] = {};
        requiredVersion = 2.0;
        requiredAddons[] = {"abo_core", "abo_ace3"};
    };
};
```

Then populate `data/weapons/`, `data/ammo/`, and `data/armor/` with JSON entries for
the mod's content.
