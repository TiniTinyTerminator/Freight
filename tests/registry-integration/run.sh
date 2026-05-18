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
#   5. The above cycle runs correctly with every detected compiler toolchain
#
# Prerequisites:
#   - gcc, g++ (or set CC=/CXX=), python3, curl or wget, unzip, cargo
#   - ~/.freight/config.toml must contain a [[registries]] entry pointing at
#     http://localhost:7878  (the default config already has this)
#
# Usage:
#   ./tests/registry-integration/run.sh [--keep] [--no-download] [--toolchain <name>]
#
#   --keep              leave .deps/ and target/ after the last toolchain (for inspection)
#   --no-download       skip setup-packages.sh; reuse existing tarballs
#   --toolchain <name>  test only this toolchain (e.g. gnu, llvm, gnu-15)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
PROJECT_DIR="$SCRIPT_DIR/project"
REGISTRY_PORT=7878
SERVER_PID=""
KEEP=false
NO_DOWNLOAD=false
ONLY_TOOLCHAIN=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --keep)        KEEP=true; shift ;;
        --no-download) NO_DOWNLOAD=true; shift ;;
        --toolchain)   ONLY_TOOLCHAIN="$2"; shift 2 ;;
        *) echo "unknown arg: $1" >&2; exit 1 ;;
    esac
done

# ── Colours ────────────────────────────────────────────────────────────────────

GREEN='\033[0;32m'; YELLOW='\033[0;33m'; RED='\033[0;31m'; CYAN='\033[0;36m'; NC='\033[0m'
pass()  { echo -e "${GREEN}  PASS${NC}  $*"; }
fail()  { echo -e "${RED}  FAIL${NC}  $*"; exit 1; }
step()  { echo -e "${YELLOW}──${NC} $*"; }
hdr()   { echo -e "\n${CYAN}═══ $* ═══${NC}"; }
skip()  { echo -e "  skip  $*"; }

# ── Cleanup ────────────────────────────────────────────────────────────────────

cleanup() {
    if [[ -n "$SERVER_PID" ]]; then
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
    fi
    if ! $KEEP; then
        rm -rf "$PROJECT_DIR/.deps" "$PROJECT_DIR/target" "$PROJECT_DIR/freight.lock"
        rm -f  "$PROJECT_DIR/.freight/config.toml"
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

# ── Toolchain detection ────────────────────────────────────────────────────────
# Build a list of (label, backend_name) pairs to test.
# label    = human-readable name for output
# backend  = value written to default_backend in .freight/config.toml

declare -a TC_LABELS=()
declare -a TC_BACKENDS=()

add_tc() {
    local label="$1" backend="$2" bin="$3"
    if command -v "$bin" &>/dev/null; then
        TC_LABELS+=("$label")
        TC_BACKENDS+=("$backend")
    fi
}

if [[ -n "$ONLY_TOOLCHAIN" ]]; then
    # User pinned a specific toolchain; derive the binary name to probe.
    # Strip any trailing version suffix for the probe (gnu-15 → gcc-15; llvm-22 → clang-22).
    case "$ONLY_TOOLCHAIN" in
        gnu*)
            ver="${ONLY_TOOLCHAIN#gnu}"  # "" or "-15"
            bin="gcc${ver/-/-}"
            [[ -z "$ver" ]] && bin="gcc"
            ;;
        llvm*)
            ver="${ONLY_TOOLCHAIN#llvm}"
            bin="clang${ver}"
            [[ -z "$ver" ]] && bin="clang"
            ;;
        *) bin="$ONLY_TOOLCHAIN" ;;
    esac
    if ! command -v "$bin" &>/dev/null; then
        fail "toolchain '$ONLY_TOOLCHAIN' not available (binary '$bin' not found)"
    fi
    TC_LABELS=("$ONLY_TOOLCHAIN")
    TC_BACKENDS=("$ONLY_TOOLCHAIN")
else
    # Auto-detect available toolchains, from most to least specific.
    # Versioned entries first so we exercise version-pinning; then the generic
    # family name so we test the auto-select path.

    # GCC versioned (descending so newest first)
    for v in 16 15 14 13 12 11; do
        add_tc "gnu-${v} (gcc-${v})" "gnu-${v}" "gcc-${v}"
    done

    # Generic GCC (unversioned — picks whatever gcc points at)
    add_tc "gnu (gcc)" "gnu" "gcc"

    # Clang versioned
    for v in 22 21 20 19 18 17 16; do
        add_tc "llvm-${v} (clang-${v})" "llvm-${v}" "clang-${v}"
    done

    # Generic Clang
    add_tc "llvm (clang)" "llvm" "clang"

    if [[ ${#TC_LABELS[@]} -eq 0 ]]; then
        fail "no supported C++ compiler found (gcc, clang required)"
    fi
fi

# ── Per-toolchain test loop ────────────────────────────────────────────────────

PASS_COUNT=0
FAIL_COUNT=0
declare -a FAILED_TCS=()

run_one_toolchain() {
    local label="$1" backend="$2"
    hdr "Toolchain: $label"

    # Write a local freight config that overrides the default backend.
    mkdir -p "$PROJECT_DIR/.freight"
    printf 'default_backend = "%s"\n' "$backend" > "$PROJECT_DIR/.freight/config.toml"

    # ── fetch ────────────────────────────────────────────────────────────────
    step "[$label] freight fetch"
    rm -rf "$PROJECT_DIR/.deps" "$PROJECT_DIR/freight.lock"
    (cd "$PROJECT_DIR" && "$FREIGHT" fetch 2>&1) | tee /tmp/freight-fetch-"${backend//\//-}".log

    [[ -f "$PROJECT_DIR/.deps/nlohmann-json/.freight-fetched" ]] \
        || { echo "  FAIL  sentinel missing for nlohmann-json"; return 1; }
    [[ -f "$PROJECT_DIR/.deps/sqlite3/.freight-fetched" ]] \
        || { echo "  FAIL  sentinel missing for sqlite3"; return 1; }
    [[ -f "$PROJECT_DIR/.deps/nlohmann-json/include/nlohmann/json.hpp" ]] \
        || { echo "  FAIL  json.hpp not extracted"; return 1; }
    [[ -f "$PROJECT_DIR/.deps/sqlite3/include/sqlite3.h" ]] \
        || { echo "  FAIL  sqlite3.h not extracted"; return 1; }
    [[ -f "$PROJECT_DIR/.deps/sqlite3/freight.toml" ]] \
        || { echo "  FAIL  sqlite3/freight.toml not extracted"; return 1; }
    [[ -f "$PROJECT_DIR/.deps/sqlite3/src/sqlite3.c" ]] \
        || { echo "  FAIL  sqlite3/src/sqlite3.c not extracted"; return 1; }
    pass "[$label] fetch OK"

    # ── idempotency ──────────────────────────────────────────────────────────
    step "[$label] idempotency check"
    (cd "$PROJECT_DIR" && "$FREIGHT" fetch 2>&1) | tee /tmp/freight-fetch2-"${backend//\//-}".log
    grep -qE "cached|up to date|ok" /tmp/freight-fetch2-"${backend//\//-}".log \
        || { echo "  FAIL  second fetch did not report packages as cached"; return 1; }
    pass "[$label] idempotent fetch OK"

    # ── build ────────────────────────────────────────────────────────────────
    step "[$label] freight build"
    rm -rf "$PROJECT_DIR/target"
    (cd "$PROJECT_DIR" && "$FREIGHT" build 2>&1) | tee /tmp/freight-build-"${backend//\//-}".log

    BINARY="$PROJECT_DIR/target/dev/registry-test"
    [[ -x "$BINARY" ]] \
        || { echo "  FAIL  binary not produced at $BINARY"; return 1; }
    [[ -f "$PROJECT_DIR/.deps/sqlite3/target/dev/libsqlite3.a" ]] \
        || { echo "  FAIL  freight did not compile sqlite3 (libsqlite3.a missing)"; return 1; }
    pass "[$label] build OK (sqlite3 compiled by freight)"

    # ── run + output check ───────────────────────────────────────────────────
    step "[$label] running binary"
    OUTPUT=$("$BINARY")
    echo "$OUTPUT"

    local ok=true
    check_line() {
        echo "$OUTPUT" | grep -qF "$1" || { echo "  FAIL  output missing: $1"; ok=false; }
    }
    check_line "json.project: freight"
    check_line "json.stars:   42"
    check_line "json.tags[0]: build"
    check_line "sqlite nlohmann-json: 3.11.3"
    check_line "sqlite sqlite3: 3.45.1"
    check_line "sqlite.count: 2"
    check_line "PASS"

    $ok || return 1
    pass "[$label] all output checks passed"
}

for i in "${!TC_LABELS[@]}"; do
    label="${TC_LABELS[$i]}"
    backend="${TC_BACKENDS[$i]}"

    if run_one_toolchain "$label" "$backend"; then
        PASS_COUNT=$(( PASS_COUNT + 1 ))
    else
        FAIL_COUNT=$(( FAIL_COUNT + 1 ))
        FAILED_TCS+=("$label")
    fi
done

# ── Summary ───────────────────────────────────────────────────────────────────

echo ""
echo "────────────────────────────────────────────────────────────"
echo -e "  Results: ${GREEN}${PASS_COUNT} passed${NC}  /  ${RED}${FAIL_COUNT} failed${NC}  (${#TC_LABELS[@]} toolchains tested)"

if [[ ${#FAILED_TCS[@]} -gt 0 ]]; then
    echo ""
    echo "  Failed toolchains:"
    for tc in "${FAILED_TCS[@]}"; do
        echo -e "    ${RED}✗${NC}  $tc"
    done
    echo "────────────────────────────────────────────────────────────"
    exit 1
fi

echo "────────────────────────────────────────────────────────────"
echo -e "${GREEN}All registry integration tests passed.${NC}"
if $KEEP; then echo "(--keep: .deps/ and target/ left in place)"; fi
