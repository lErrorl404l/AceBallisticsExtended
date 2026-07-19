# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial project structure — Rust extension (ext/), SQF addons, data/ directory
- Interior ballistics: chamber pressure, barrel friction, twist-rate stabilization
- External ballistics: drag curve (G1/G7), Coriolis, Magnus, wind models
- Terminal ballistics: AP/APDS/APFSDS penetration, spall generation
- Armor penetration: RHA equivalence, composite/ERA/slat/cage arrays
- Ricochet modelling (in-development)
- 250+ data files: weapons, ammo, calibers, armor plates, materials
- IRL data sourced from Hornady, Lapua, SAAMI/CIP, ARL/BRL, SSAB ARMOX
- Cross-compilation for Linux (.so) and Windows (.dll)
- HEMTT build system integration
- 1134+ passing tests (Rust units + SQF integration + edge-case validation)
