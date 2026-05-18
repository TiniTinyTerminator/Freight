#!/usr/bin/env bash
# Integration test: freight fetch + freight build against a local vcpkg-style registry.
#
# Downloads real libraries from the internet:
#   nlohmann/json 3.11.3  — header-only JSON  (github.com/nlohmann/json)
#   SQLite        3.45.1  — compiled C library (sqlite.org)
#
# What this tests:
#   1. registry server responds correctly to /api/v1/packages/* requests
#   2. `freight fetch` downloads packages from the registry
#   3. `freight build` compiles the project and links against the fetched libraries
#   4. The resulting binary runs and produces the expected output
#
# Prerequisites:
#   - gcc, g++ (or set CC=/CXX=), python3, curl or wget, unzip, cargo
#   - ~/.freight/config.toml must contain a [[registries]] entry pointing at
#     http://localhost:7878  (the default config already has this)
#
# Usage:
#   ./tests/registry-integration/run.sh [--keep] [--no-download]
#
#   --keep          leave .deps/ and target/ after the test (for inspection)
#   --no-download   skip setup-packages.sh; reuse existing tarballs

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
PROJECT_DIR="$SCRIPT_DIR/project"
REGISTRY_PORT=7878
SERVER_PID=""
KEEP=false
NO_DOWNLOAD=false

for arg in "$@"; do
    case "$arg" in
        --keep)        KEEP=true ;;
        --no-download) NO_DOWNLOAD=true ;;
    esac
done

# ── Colours ────────────────────────────────────────────────────────────────────

GREEN='\033[0;32m'; YELLOW='\033[0;33m'; RED='\033[0;31m'; NC='\033[0m'
pass() { echo -e "${GREEN}  PASS${NC}  $*"; }
fail() { echo -e "${RED}  FAIL${NC}  $*"; exit 1; }
step() { echo -e "${YELLOW}──${NC} $*"; }

# ── Cleanup ────────────────────────────────────────────────────────────────────

cleanup() {
    if [[ -n "$SERVER_PID" ]]; then
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
    fi
    if ! $KEEP; then
        rm -rf "$PROJECT_DIR/.deps" "$PROJECT_DIR/target" "$PROJECT_DIR/freight.lock"
    fi
}
trap cleanup EXIT

# ── Step 1: Build freight ──────────────────────────────────────────────────────

step "Building freight"
cargo build -q --workspace 2>&1
FREIGHT="$REPO_ROOT/target/debug/freight"
[[ -x "$FREIGHT" ]] || fail "freight binary not found at $FREIGHT"
pass "freight built: $FREIGHT"

# ── Step 2: Download and package real libraries ────────────────────────────────

if $NO_DOWNLOAD; then
    step "Skipping download (--no-download)"
    [[ -f "$SCRIPT_DIR/registry/data/packages/nlohmann-json/3.11.3.tar.gz" ]] \
        || fail "tarballs not found — run without --no-download first"
else
    step "Downloading and packaging real libraries (internet required)"
    bash "$SCRIPT_DIR/setup-packages.sh"
fi

[[ -f "$SCRIPT_DIR/registry/data/packages/nlohmann-json/3.11.3.tar.gz" ]] \
    || fail "nlohmann-json tarball not created"
[[ -f "$SCRIPT_DIR/registry/data/packages/sqlite3/3.45.1.tar.gz" ]] \
    || fail "sqlite3 tarball not created"
pass "tarballs ready"

# ── Step 3: Start the registry server ─────────────────────────────────────────

step "Starting registry server on port $REGISTRY_PORT"

if ss -tlnp 2>/dev/null | grep -q ":${REGISTRY_PORT} " || \
   netstat -tlnp 2>/dev/null | grep -q ":${REGISTRY_PORT} "; then
    fail "port $REGISTRY_PORT is already in use — stop the existing server first"
fi

REGISTRY_PORT=$REGISTRY_PORT python3 "$SCRIPT_DIR/registry/server.py" &
SERVER_PID=$!

for i in $(seq 1 20); do
    if curl -sf "http://127.0.0.1:$REGISTRY_PORT/api/v1/search?q=" >/dev/null 2>&1; then
        break
    fi
    sleep 0.2
    if [[ $i -eq 20 ]]; then fail "registry server did not start within 4 seconds"; fi
done
pass "server started (pid $SERVER_PID)"

# ── Step 4: Verify registry API ────────────────────────────────────────────────

step "Verifying registry API"

JSON_META=$(curl -sf "http://127.0.0.1:$REGISTRY_PORT/api/v1/packages/nlohmann-json")
echo "$JSON_META" | python3 -c "
import sys, json
d = json.load(sys.stdin)
assert d['latest'] == '3.11.3', f'expected 3.11.3, got {d}'
" || fail "nlohmann-json metadata wrong: $JSON_META"

SQLITE_META=$(curl -sf "http://127.0.0.1:$REGISTRY_PORT/api/v1/packages/sqlite3")
echo "$SQLITE_META" | python3 -c "
import sys, json
d = json.load(sys.stdin)
assert d['latest'] == '3.45.1', f'expected 3.45.1, got {d}'
" || fail "sqlite3 metadata wrong: $SQLITE_META"

HTTP_404=$(curl -s -o /dev/null -w "%{http_code}" "http://127.0.0.1:$REGISTRY_PORT/api/v1/packages/does-not-exist")
[[ "$HTTP_404" == "404" ]] || fail "expected 404 for unknown package, got $HTTP_404"

pass "registry API correct"

# ── Step 5: freight fetch ──────────────────────────────────────────────────────

step "Running freight fetch"
rm -rf "$PROJECT_DIR/.deps"
cd "$PROJECT_DIR"
"$FREIGHT" fetch 2>&1 | tee /tmp/freight-fetch.log

[[ -f "$PROJECT_DIR/.deps/nlohmann-json/.freight-fetched" ]] \
    || fail "sentinel missing for nlohmann-json"
[[ -f "$PROJECT_DIR/.deps/sqlite3/.freight-fetched" ]] \
    || fail "sentinel missing for sqlite3"
[[ -f "$PROJECT_DIR/.deps/nlohmann-json/include/nlohmann/json.hpp" ]] \
    || fail "json.hpp not extracted"
[[ -f "$PROJECT_DIR/.deps/sqlite3/include/sqlite3.h" ]] \
    || fail "sqlite3.h not extracted"
[[ -f "$PROJECT_DIR/.deps/sqlite3/freight.toml" ]] \
    || fail "sqlite3/freight.toml not extracted (expected a freight source package)"
[[ -f "$PROJECT_DIR/.deps/sqlite3/src/sqlite3.c" ]] \
    || fail "sqlite3/src/sqlite3.c not extracted"
pass "freight fetch: both source packages downloaded and extracted"

# ── Step 6: Idempotency ────────────────────────────────────────────────────────

step "Verifying fetch is idempotent"
"$FREIGHT" fetch 2>&1 | tee /tmp/freight-fetch2.log
grep -qE "cached|up to date|ok" /tmp/freight-fetch2.log \
    || fail "second fetch did not report packages as cached"
pass "idempotent fetch OK"

# ── Step 7: freight build ──────────────────────────────────────────────────────

step "Running freight build"
"$FREIGHT" build 2>&1 | tee /tmp/freight-build.log

BINARY="$PROJECT_DIR/target/dev/registry-test"
[[ -x "$BINARY" ]] || fail "binary not produced at $BINARY"

# sqlite3 must have been compiled by freight (not linked from a pre-built archive)
[[ -f "$PROJECT_DIR/.deps/sqlite3/target/dev/libsqlite3.a" ]] \
    || fail "freight did not compile sqlite3 — libsqlite3.a not found in .deps/sqlite3/target/"

pass "freight build produced binary (sqlite3 compiled by freight)"

# ── Step 8: Run and verify output ─────────────────────────────────────────────

step "Running binary and checking output"
OUTPUT=$("$BINARY")
echo "$OUTPUT"

check() { echo "$OUTPUT" | grep -qF "$1" || fail "output missing: $1"; }

check "json.project: freight"
check "json.stars:   42"
check "json.tags[0]: build"
check "sqlite nlohmann-json: 3.11.3"
check "sqlite sqlite3: 3.45.1"
check "sqlite.count: 2"
check "PASS"

pass "all output checks passed"

# ── Done ───────────────────────────────────────────────────────────────────────

echo ""
echo -e "${GREEN}All registry integration tests passed.${NC}"
if $KEEP; then echo "(--keep: .deps/ and target/ left in place)"; fi
