# ABE Development Tools

## Build
- `./build.sh` - Build extension and mod
- `hemtt build` - Build PBOs
- `hemtt dev` - Development build (faster iteration)
- `hemtt check` - Lint and validate

## Export
- `./build.sh --release --hemtt` - Full release build
- `hemtt release` - Tagged release

## Tools
- `armake` - Alternative PBO tooling
- `depbo` - Depbo existing mods for reference

## Testing
- `cargo test` - Run Rust extension tests
- `hemtt test` - Run SQF unit tests
- `python tests/validate_data.py` - Validate weapon/ammo/armor JSON
