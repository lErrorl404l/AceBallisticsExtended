#!/usr/bin/env bash
# ABE full build script
# Builds the Rust extension, copies the binary, then runs HEMTT.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "=== 1. Building Rust extension ==="
(cd ext && cargo build --release)

echo ""
echo "=== 2. Copying extension binary ==="
mkdir -p ext/target/release
# Only copy if source exists (it should after cargo build)
if [ -f ext/target/release/libabe_ballistics_ext.so ]; then
    cp ext/target/release/libabe_ballistics_ext.so ext/libabe_ballistics_ext.so
    echo "Copied libabe_ballistics_ext.so → ext/"
fi

echo ""
echo "=== 3. Running HEMTT check ==="
hemtt check

echo ""
echo "=== Build complete ==="
echo "Run: hemtt dev    (development build with file patching)"
echo "     hemtt build  (local testing build)"
echo "     hemtt release (release build)"
