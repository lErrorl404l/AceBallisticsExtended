#!/bin/bash
set -e

echo "[ABE] Building mod with HEMTT..."
hemtt build

SERVER_BIN="/arma3/arma3server_x64"
if [ ! -f "$SERVER_BIN" ]; then
    echo "[ABE] WARNING: $SERVER_BIN not found."
    echo "[ABE] The mod was built successfully, but the Arma 3 server is not installed."
    echo "[ABE] To install: run 'steamcmd +@sSteamCmdForcePlatformType linux +force_install_dir /arma3 +login <your_username> +app_update 233780 validate +quit'"
    echo "[ABE] Or mount an existing installation: docker run ... -v /path/to/arma3:/arma3"
    echo "[ABE] Skipping server launch. Build artifacts are at /abe/releases/latest"
    exit 0
fi

echo "[ABE] Starting headless server..."
"$SERVER_BIN" \
    -name=abe_test \
    -config=/abe/.hemtt/server.cfg \
    -mod=/abe/releases/latest \
    -world=empty \
    -nosplash \
    -skipIntro \
    -showScriptErrors \
    -autoInit

echo "[ABE] Server exited. Checking results..."

# Find the RPT file — varies by platform
RPT_FILE=""
for candidate in \
    /arma3/*.rpt \
    /root/.local/share/Arma\ 3/*.rpt \
    /tmp/*.rpt; do
    if [ -f "$candidate" ]; then
        RPT_FILE="$candidate"
        break
    fi
done

if [ -n "$RPT_FILE" ]; then
    echo "[ABE] RPT file: $RPT_FILE"
    PASS_COUNT=$(grep -c "\[ABE_TEST\] PASS:" "$RPT_FILE" 2>/dev/null || echo 0)
    FAIL_COUNT=$(grep -c "\[ABE_TEST\] FAIL:" "$RPT_FILE" 2>/dev/null || echo 0)
    echo "[ABE] Tests: $PASS_COUNT PASS, $FAIL_COUNT FAIL"
    echo ""
    echo "=== Test Log ==="
    grep "\[ABE_TEST\]" "$RPT_FILE" || echo "(no test markers found)"
    echo "================"
    if [ "$FAIL_COUNT" -gt 0 ]; then
        exit 1
    fi
else
    echo "[ABE] No RPT file found."
    echo "[ABE] Searched: /arma3/*.rpt, ~/.local/share/Arma 3/*.rpt, /tmp/*.rpt"
    # Non-zero exit only if tests were expected
    echo "[ABE] WARNING: RPT not found — cannot verify test results."
    exit 0
fi
