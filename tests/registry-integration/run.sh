#!/usr/bin/env bash
# Integration test: freight fetch + freight build against a local vcpkg-style registry.
#
# What this tests:
#   1. registry server responds correctly to /api/v1/packages/* requests
#   2. `freight fetch` downloads libvec and libstr from the registry
#   3. `freight build` compiles the project and links against the fetched libraries
#   4. The resulting binary runs and produces the expected output
#
# Prerequisites:
#   - gcc (or set CC= to another C compiler)
#   - python3
#   - cargo (to build freight)
#   - ~/.freight/config.toml has a [[registries]] entry with url = "http://localhost:7878"
#     (the default config created during `freight` install already sets this up)
#
# Usage:
#   ./tests/registry-integration/run.sh [--keep]
#
#   --keep   leave .deps/ and target/ in place after the test (useful for inspection)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
PROJECT_DIR="$SCRIPT_DIR/project"
REGISTRY_PORT=7878
SERVER_PID=""
KEEP=false

for arg in "$@"; do
    [[ "$arg" == "--keep" ]] && KEEP=true
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

# ── Step 2: Build and package test libraries ───────────────────────────────────

step "Building package tarballs"
bash "$SCRIPT_DIR/setup-packages.sh"
[[ -f "$SCRIPT_DIR/registry/data/packages/libvec/1.0.0.tar.gz" ]] \
    || fail "libvec tarball not created"
[[ -f "$SCRIPT_DIR/registry/data/packages/libstr/1.0.0.tar.gz" ]] \
    || fail "libstr tarball not created"
pass "tarballs created"

# ── Step 3: Start the registry server ─────────────────────────────────────────

step "Starting registry server on port $REGISTRY_PORT"

# Check the port is free
if ss -tlnp 2>/dev/null | grep -q ":${REGISTRY_PORT} " || \
   netstat -tlnp 2>/dev/null | grep -q ":${REGISTRY_PORT} "; then
    fail "port $REGISTRY_PORT is already in use — stop the existing server first"
fi

REGISTRY_PORT=$REGISTRY_PORT python3 "$SCRIPT_DIR/registry/server.py" &
SERVER_PID=$!

# Wait for the server to start accepting connections
for i in $(seq 1 20); do
    if curl -sf "http://127.0.0.1:$REGISTRY_PORT/api/v1/search?q=" >/dev/null 2>&1; then
        break
    fi
    sleep 0.2
    if [[ $i -eq 20 ]]; then
        fail "registry server did not start within 4 seconds"
    fi
done
pass "server started (pid $SERVER_PID)"

# ── Step 4: Verify registry API ────────────────────────────────────────────────

step "Verifying registry API"

# Metadata endpoint
LIBVEC_META=$(curl -sf "http://127.0.0.1:$REGISTRY_PORT/api/v1/packages/libvec")
echo "$LIBVEC_META" | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['latest']=='1.0.0', d" \
    || fail "libvec metadata returned unexpected data: $LIBVEC_META"

LIBSTR_META=$(curl -sf "http://127.0.0.1:$REGISTRY_PORT/api/v1/packages/libstr")
echo "$LIBSTR_META" | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['latest']=='1.0.0', d" \
    || fail "libstr metadata returned unexpected data"

# 404 for unknown package
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" "http://127.0.0.1:$REGISTRY_PORT/api/v1/packages/nonexistent")
[[ "$HTTP_CODE" == "404" ]] || fail "expected 404 for unknown package, got $HTTP_CODE"

pass "registry API responds correctly"

# ── Step 5: freight fetch ──────────────────────────────────────────────────────

step "Running freight fetch"

# Ensure a clean state
rm -rf "$PROJECT_DIR/.deps"

cd "$PROJECT_DIR"
"$FREIGHT" fetch 2>&1 | tee /tmp/freight-fetch.log

[[ -d "$PROJECT_DIR/.deps/libvec" ]] || fail ".deps/libvec not created by freight fetch"
[[ -d "$PROJECT_DIR/.deps/libstr" ]] || fail ".deps/libstr not created by freight fetch"
[[ -f "$PROJECT_DIR/.deps/libvec/.freight-fetched" ]] || fail ".freight-fetched sentinel missing for libvec"
[[ -f "$PROJECT_DIR/.deps/libstr/.freight-fetched" ]] || fail ".freight-fetched sentinel missing for libstr"
[[ -f "$PROJECT_DIR/.deps/libvec/include/vec.h" ]] || fail "vec.h not extracted"
[[ -f "$PROJECT_DIR/.deps/libstr/include/str.h" ]] || fail "str.h not extracted"
[[ -f "$PROJECT_DIR/.deps/libvec/lib/libvec.a" ]] || fail "libvec.a not extracted"
[[ -f "$PROJECT_DIR/.deps/libstr/lib/libstr.a" ]] || fail "libstr.a not extracted"
pass "freight fetch downloaded and extracted both packages"

# ── Step 6: freight fetch is idempotent ───────────────────────────────────────

step "Verifying fetch is idempotent (cached)"
"$FREIGHT" fetch 2>&1 | tee /tmp/freight-fetch2.log
grep -q "cached\|up to date\|ok" /tmp/freight-fetch2.log \
    || fail "second fetch did not report packages as cached"
pass "idempotent fetch OK"

# ── Step 7: freight build ──────────────────────────────────────────────────────

step "Running freight build"
"$FREIGHT" build 2>&1 | tee /tmp/freight-build.log

BINARY="$PROJECT_DIR/target/dev/registry-test"
[[ -x "$BINARY" ]] || fail "binary not produced at $BINARY"
pass "freight build produced binary"

# ── Step 8: Run the binary ─────────────────────────────────────────────────────

step "Running binary and checking output"
OUTPUT=$("$BINARY")
echo "$OUTPUT"

check_output() {
    echo "$OUTPUT" | grep -qF "$1" || fail "output missing: $1"
}

check_output "vec3_add:   (5.0, 7.0, 9.0)"
check_output "vec3_dot:   32.0"
check_output "count 's':  4"
check_output "repeat:     ababab"
check_output "reverse:    olleh"
check_output "PASS"

pass "all output checks passed"

# ── Done ───────────────────────────────────────────────────────────────────────

echo ""
echo -e "${GREEN}All registry integration tests passed.${NC}"
if $KEEP; then echo "(--keep: leaving .deps/ and target/ in place)"; fi
