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
#   5. The above cycle runs for every detected compiler toolchain
#   6. nvcc "sim mode" — nvcc compiles a .cu project; binary runs without a GPU
#   7. QEMU cross-compilation — aarch64-linux-gnu binary executed via qemu-aarch64-static
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
#   --toolchain <name>  test only this toolchain (e.g. gnu, llvm, gnu-15, aarch64-linux-gnu)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
PROJECT_DIR="$SCRIPT_DIR/project"
CUDA_PROJECT_DIR="$SCRIPT_DIR/project-cuda"
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

GREEN='\033[0;32m'; YELLOW='\033[0;33m'; RED='\033[0;31m'; CYAN='\033[0;36m'
GRAY='\033[0;37m'; NC='\033[0m'
pass()  { echo -e "${GREEN}  PASS${NC}  $*"; }
fail()  { echo -e "${RED}  FAIL${NC}  $*"; exit 1; }
step()  { echo -e "${YELLOW}──${NC} $*"; }
hdr()   { echo -e "\n${CYAN}═══ $* ═══${NC}"; }
skip()  { echo -e "${GRAY}  skip${NC}  $*"; }

# ── Output checker (top-level so all test functions can use it) ────────────────

# Usage: check_lines "$OUTPUT" "expected line 1" "expected line 2" ...
# Sets the caller's local `ok` variable to false on mismatch.
check_lines() {
    local output="$1"; shift
    for pattern in "$@"; do
        echo "$output" | grep -qF "$pattern" \
            || { echo "  FAIL  output missing: $pattern"; ok=false; }
    done
}

# ── Cleanup ────────────────────────────────────────────────────────────────────

cleanup() {
    if [[ -n "$SERVER_PID" ]]; then
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
    fi
    if ! $KEEP; then
        rm -rf "$PROJECT_DIR/.deps"      "$PROJECT_DIR/target"      "$PROJECT_DIR/freight.lock"
        rm -rf "$CUDA_PROJECT_DIR/.deps" "$CUDA_PROJECT_DIR/target" "$CUDA_PROJECT_DIR/freight.lock"
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

HTTP_404=$(curl -s -o /dev/null -w "%{http_code}" \
    "http://127.0.0.1:$REGISTRY_PORT/api/v1/packages/does-not-exist")
[[ "$HTTP_404" == "404" ]] || fail "expected 404 for unknown package, got $HTTP_404"

pass "registry API correct"

# ── Toolchain detection ────────────────────────────────────────────────────────
# TC_LABELS / TC_BACKENDS: native compiler tests (main project)
# TC_RUNNERS / TC_LD_PREFIXES: optional runner + LD prefix (empty = run natively)

declare -a TC_LABELS=()
declare -a TC_BACKENDS=()
declare -a TC_RUNNERS=()
declare -a TC_LD_PREFIXES=()

add_tc() {
    local label="$1" backend="$2" bin="$3"
    local runner="${4:-}" ld_prefix="${5:-}"
    if command -v "$bin" &>/dev/null; then
        TC_LABELS+=("$label")
        TC_BACKENDS+=("$backend")
        TC_RUNNERS+=("$runner")
        TC_LD_PREFIXES+=("$ld_prefix")
    fi
}

if [[ -n "$ONLY_TOOLCHAIN" ]]; then
    case "$ONLY_TOOLCHAIN" in
        gnu*)   bin="gcc${ONLY_TOOLCHAIN#gnu}"; [[ "$ONLY_TOOLCHAIN" == "gnu" ]] && bin="gcc" ;;
        llvm*)  bin="clang${ONLY_TOOLCHAIN#llvm}"; [[ "$ONLY_TOOLCHAIN" == "llvm" ]] && bin="clang" ;;
        aarch64-linux-gnu) bin="aarch64-linux-gnu-g++" ;;
        *) bin="$ONLY_TOOLCHAIN" ;;
    esac
    if ! command -v "$bin" &>/dev/null; then
        fail "toolchain '$ONLY_TOOLCHAIN' not available (binary '$bin' not found)"
    fi
    TC_LABELS=("$ONLY_TOOLCHAIN")
    TC_BACKENDS=("$ONLY_TOOLCHAIN")
    TC_RUNNERS=("")
    TC_LD_PREFIXES=("")
else
    # ── Native GCC (versioned, then unversioned) ──────────────────────────────
    for v in 16 15 14 13 12 11; do
        add_tc "gnu-${v} (gcc-${v})" "gnu-${v}" "gcc-${v}"
    done
    add_tc "gnu (gcc)" "gnu" "gcc"

    # ── Native Clang (versioned, then unversioned) ────────────────────────────
    for v in 22 21 20 19 18 17 16; do
        add_tc "llvm-${v} (clang-${v})" "llvm-${v}" "clang-${v}"
    done
    add_tc "llvm (clang)" "llvm" "clang"

    # ── QEMU: aarch64-linux-gnu cross-compilation ─────────────────────────────
    # Requires: gcc-aarch64-linux-gnu, qemu-aarch64-static (or binfmt_misc)
    # QEMU_LD_PREFIX tells qemu-aarch64-static where to find aarch64 shared libs.
    QEMU_BIN=""
    if   command -v qemu-aarch64-static &>/dev/null; then QEMU_BIN="qemu-aarch64-static"
    elif command -v qemu-aarch64        &>/dev/null; then QEMU_BIN="qemu-aarch64"
    fi
    if [[ -n "$QEMU_BIN" ]]; then
        # Prefer the sysroot from the cross-toolchain; fall back to common path.
        AARCH64_SYSROOT=""
        for p in /usr/aarch64-linux-gnu /usr/lib/aarch64-linux-gnu; do
            [[ -d "$p/lib" ]] && { AARCH64_SYSROOT="$p"; break; }
        done
        add_tc "aarch64 via QEMU (${QEMU_BIN})" "aarch64-linux-gnu" \
               "aarch64-linux-gnu-g++" "$QEMU_BIN" "$AARCH64_SYSROOT"
    fi

    if [[ ${#TC_LABELS[@]} -eq 0 ]]; then
        fail "no supported C++ compiler found"
    fi
fi

# ── Per-toolchain fetch + build + run loop ─────────────────────────────────────

PASS_COUNT=0
FAIL_COUNT=0
SKIP_COUNT=0
declare -a FAILED_TCS=()

run_one_toolchain() {
    local label="$1" backend="$2"
    local runner="${3:-}"       # empty = run natively; e.g. "qemu-aarch64-static"
    local ld_prefix="${4:-}"    # QEMU_LD_PREFIX for dynamic lib lookup

    hdr "Toolchain: $label"

    mkdir -p "$PROJECT_DIR/.freight"
    printf 'default_backend = "%s"\n' "$backend" > "$PROJECT_DIR/.freight/config.toml"

    # ── fetch ────────────────────────────────────────────────────────────────
    step "[$label] freight fetch"
    rm -rf "$PROJECT_DIR/.deps" "$PROJECT_DIR/freight.lock"
    (cd "$PROJECT_DIR" && "$FREIGHT" fetch 2>&1) \
        | tee "/tmp/freight-fetch-${backend//\//-}.log"

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
    (cd "$PROJECT_DIR" && "$FREIGHT" fetch 2>&1) \
        | tee "/tmp/freight-fetch2-${backend//\//-}.log"
    grep -qE "cached|up to date|ok" "/tmp/freight-fetch2-${backend//\//-}.log" \
        || { echo "  FAIL  second fetch did not report packages as cached"; return 1; }
    pass "[$label] idempotent fetch OK"

    # ── build ────────────────────────────────────────────────────────────────
    step "[$label] freight build"
    rm -rf "$PROJECT_DIR/target"
    (cd "$PROJECT_DIR" && "$FREIGHT" build 2>&1) \
        | tee "/tmp/freight-build-${backend//\//-}.log"

    BINARY="$PROJECT_DIR/target/dev/registry-test"
    [[ -x "$BINARY" ]] \
        || { echo "  FAIL  binary not produced at $BINARY"; return 1; }
    [[ -f "$PROJECT_DIR/.deps/sqlite3/target/dev/libsqlite3.a" ]] \
        || { echo "  FAIL  libsqlite3.a missing (freight did not compile sqlite3)"; return 1; }
    pass "[$label] build OK"

    # ── run + output check ───────────────────────────────────────────────────
    step "[$label] running binary${runner:+ via $runner}"
    local ok=true
    if [[ -n "$runner" ]]; then
        OUTPUT=$(QEMU_LD_PREFIX="${ld_prefix}" "$runner" "$BINARY" 2>&1)
    else
        OUTPUT=$("$BINARY")
    fi
    echo "$OUTPUT"

    check_lines "$OUTPUT" \
        "json.project: freight" \
        "json.stars:   42"      \
        "json.tags[0]: build"   \
        "sqlite nlohmann-json: 3.11.3" \
        "sqlite sqlite3: 3.45.1"       \
        "sqlite.count: 2"              \
        "PASS"

    $ok || return 1
    pass "[$label] all output checks passed"
}

# ── CUDA test (nvcc + host-only sim mode) ─────────────────────────────────────

run_cuda_test() {
    hdr "CUDA: nvcc (host-only sim mode)"

    if ! command -v nvcc &>/dev/null; then
        skip "nvcc not found — install CUDA Toolkit to enable this test"
        SKIP_COUNT=$(( SKIP_COUNT + 1 ))
        return 0
    fi
    if ! command -v g++ &>/dev/null && ! command -v clang++ &>/dev/null; then
        skip "no C++ host compiler alongside nvcc — skipping CUDA test"
        SKIP_COUNT=$(( SKIP_COUNT + 1 ))
        return 0
    fi

    local ok=true

    # ── fetch ────────────────────────────────────────────────────────────────
    step "[cuda] freight fetch"
    rm -rf "$CUDA_PROJECT_DIR/.deps" "$CUDA_PROJECT_DIR/freight.lock"
    (cd "$CUDA_PROJECT_DIR" && "$FREIGHT" fetch 2>&1) | tee /tmp/freight-fetch-cuda.log

    [[ -f "$CUDA_PROJECT_DIR/.deps/nlohmann-json/.freight-fetched" ]] \
        || { echo "  FAIL  sentinel missing (nlohmann-json)"; ok=false; }
    [[ -f "$CUDA_PROJECT_DIR/.deps/sqlite3/.freight-fetched" ]] \
        || { echo "  FAIL  sentinel missing (sqlite3)"; ok=false; }
    $ok || return 1
    pass "[cuda] fetch OK"

    # ── build ────────────────────────────────────────────────────────────────
    step "[cuda] freight build (nvcc compiles main.cu, g++/clang++ links)"
    rm -rf "$CUDA_PROJECT_DIR/target"
    (cd "$CUDA_PROJECT_DIR" && "$FREIGHT" build 2>&1) | tee /tmp/freight-build-cuda.log

    local CUDA_BINARY="$CUDA_PROJECT_DIR/target/dev/registry-test-cuda"
    [[ -x "$CUDA_BINARY" ]] \
        || { echo "  FAIL  binary not produced at $CUDA_BINARY"; return 1; }

    # Confirm nvcc was actually used (it logs a line containing "nvcc")
    grep -qi "nvcc" /tmp/freight-build-cuda.log \
        || { echo "  WARN  nvcc not mentioned in build log — was nvcc used?"; }

    pass "[cuda] build OK"

    # ── run (no GPU needed — host-only binary) ───────────────────────────────
    step "[cuda] running binary (host-only, no GPU required)"
    local OUTPUT
    OUTPUT=$("$CUDA_BINARY")
    echo "$OUTPUT"

    check_lines "$OUTPUT" \
        "json.project: freight" \
        "json.stars:   42"      \
        "sqlite nlohmann-json: 3.11.3" \
        "sqlite sqlite3: 3.45.1"       \
        "sqlite.count: 2"              \
        "cuda.nvcc:"                   \
        "PASS"

    $ok || return 1
    pass "[cuda] all output checks passed"
}

# ── Run all toolchains ─────────────────────────────────────────────────────────

for i in "${!TC_LABELS[@]}"; do
    if run_one_toolchain \
           "${TC_LABELS[$i]}" \
           "${TC_BACKENDS[$i]}" \
           "${TC_RUNNERS[$i]}" \
           "${TC_LD_PREFIXES[$i]}"; then
        PASS_COUNT=$(( PASS_COUNT + 1 ))
    else
        FAIL_COUNT=$(( FAIL_COUNT + 1 ))
        FAILED_TCS+=("${TC_LABELS[$i]}")
    fi
done

# ── CUDA test (independent of the toolchain loop) ────────────────────────────

if [[ -z "$ONLY_TOOLCHAIN" || "$ONLY_TOOLCHAIN" == "nvcc" || "$ONLY_TOOLCHAIN" == "cuda" ]]; then
    if run_cuda_test; then
        PASS_COUNT=$(( PASS_COUNT + 1 ))
    else
        FAIL_COUNT=$(( FAIL_COUNT + 1 ))
        FAILED_TCS+=("nvcc (cuda sim)")
    fi
fi

# ── Summary ───────────────────────────────────────────────────────────────────

TOTAL=$(( PASS_COUNT + FAIL_COUNT + SKIP_COUNT ))
echo ""
echo "────────────────────────────────────────────────────────────"
echo -e "  Results: ${GREEN}${PASS_COUNT} passed${NC}  /  ${RED}${FAIL_COUNT} failed${NC}  /  ${GRAY}${SKIP_COUNT} skipped${NC}  (${TOTAL} total)"

if [[ ${#FAILED_TCS[@]} -gt 0 ]]; then
    echo ""
    echo "  Failed:"
    for tc in "${FAILED_TCS[@]}"; do
        echo -e "    ${RED}✗${NC}  $tc"
    done
    echo "────────────────────────────────────────────────────────────"
    exit 1
fi

echo "────────────────────────────────────────────────────────────"
echo -e "${GREEN}All registry integration tests passed.${NC}"
if $KEEP; then echo "(--keep: artifacts left in place)"; fi
