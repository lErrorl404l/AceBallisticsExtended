#!/bin/bash
# Run ABE headless tests locally
# Prerequisites:
#   - Arma 3 Dedicated Server installed (ARMA3_DIR or /arma3)
#   - HEMTT installed on PATH
#   - Rust toolchain
set -e

ARMA3_DIR="${ARMA3_DIR:-/arma3}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

echo "[ABE] === Building and testing ==="

echo "[ABE] cargo test (Rust)..."
cd "$PROJECT_DIR/ext"
cargo test

echo "[ABE] cargo build --release..."
cargo build --release

echo "[ABE] Copying .so for HEMTT packaging..."
cp target/release/libabe_ballistics_ext.so "$PROJECT_DIR/ext/libabe_ballistics_ext.so"

echo "[ABE] hemtt build..."
cd "$PROJECT_DIR"
hemtt build

echo "[ABE] === Starting headless server ==="
echo "[ABE] Server binary: $ARMA3_DIR/arma3server_x64"
echo "[ABE] Config: $PROJECT_DIR/.hemtt/server.cfg"
echo "[ABE] Mod: $PROJECT_DIR/releases/latest"

"$ARMA3_DIR/arma3server_x64" \
    -name=abe_test \
    -config="$PROJECT_DIR/.hemtt/server.cfg" \
    -mod="$PROJECT_DIR/releases/latest" \
    -world=empty \
    -nosplash \
    -skipIntro \
    -showScriptErrors \
    -autoInit \
    2>&1 | tee "$PROJECT_DIR/tests/headless/server.log"

echo "[ABE] === Checking results ==="

# Find the most recent RPT
RPT_DIR="${HOME}/.local/share/Arma 3"
RPT_FILE=$(ls -t "$RPT_DIR"/*.rpt 2>/dev/null | head -1)

if [ -z "$RPT_FILE" ]; then
    # Fallback: look near the server binary
    RPT_FILE=$(ls -t "$ARMA3_DIR"/*.rpt 2>/dev/null | head -1)
fi

if [ -n "$RPT_FILE" ]; then
    echo "[ABE] RPT file: $RPT_FILE"
    PASS_COUNT=$(grep -c "\[ABE_TEST\] PASS:" "$RPT_FILE" 2>/dev/null || echo 0)
    FAIL_COUNT=$(grep -c "\[ABE_TEST\] FAIL:" "$RPT_FILE" 2>/dev/null || echo 0)
    INFO_COUNT=$(grep -c "\[ABE_TEST\] INFO:" "$RPT_FILE" 2>/dev/null || echo 0)
    echo "[ABE] Results: $PASS_COUNT PASS, $FAIL_COUNT FAIL, $INFO_COUNT INFO"
    echo ""
    echo "=== Test Output ==="
    grep "\[ABE_TEST\]" "$RPT_FILE"
    echo "==================="
    [ "$FAIL_COUNT" -eq 0 ] || { echo "[ABE] FAILED"; exit 1; }
    echo "[ABE] PASSED"
else
    echo "[ABE] ERROR: No RPT file found"
    echo "[ABE] Searched: $RPT_DIR and $ARMA3_DIR"
    exit 1
fi
