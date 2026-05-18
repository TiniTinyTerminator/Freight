#!/usr/bin/env bash
# Download real libraries from the internet and package them as freight source packages
# for the local test registry.
#
# Each package is a valid freight project containing freight.toml + source files.
# freight build will compile these packages automatically when they are listed as
# dependencies — no pre-built archives needed.
#
# Packages built:
#   nlohmann-json 3.11.3  — header-only JSON   (github.com/nlohmann/json)
#   sqlite3       3.45.1  — C amalgamation      (sqlite.org)
#
# Output: registry/data/packages/{name}/{version}.tar.gz
# Tarball layout (strip-components=1 extracts to .deps/{name}/):
#   {name}-{version}/
#   ├── freight.toml          # freight package descriptor
#   ├── include/...           # public headers
#   └── src/...               # source files  (compiled packages only)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT_DIR="$SCRIPT_DIR/registry/data/packages"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

download() {
    local url="$1" dest="$2"
    if command -v curl &>/dev/null; then
        curl -fsSL --retry 3 "$url" -o "$dest"
    elif command -v wget &>/dev/null; then
        wget -q --tries=3 "$url" -O "$dest"
    else
        echo "error: neither curl nor wget found" >&2; exit 1
    fi
}

make_tarball() {
    local name="$1" version="$2"
    local dir="$OUT_DIR/$name"
    mkdir -p "$dir"
    tar czf "$dir/${version}.tar.gz" -C "$WORK" "${name}-${version}"
    echo "   wrote $dir/${version}.tar.gz ($(wc -c < "$dir/${version}.tar.gz" | xargs printf '%d bytes'))"
}

# ── nlohmann/json 3.11.3 ───────────────────────────────────────────────────────
# Header-only: freight.toml declares no [[lib]] or [[bin]].
# build_deps sees no source files → contributes include/ only.

build_nlohmann_json() {
    local name="nlohmann-json" version="3.11.3"
    local stage="$WORK/${name}-${version}"
    echo "→ downloading $name $version"

    mkdir -p "$stage/include/nlohmann"
    download \
        "https://github.com/nlohmann/json/releases/download/v${version}/json.hpp" \
        "$stage/include/nlohmann/json.hpp"

    cat > "$stage/freight.toml" <<TOML
[package]
name    = "$name"
version = "$version"
TOML

    make_tarball "$name" "$version"
}

# ── SQLite 3.45.1 ──────────────────────────────────────────────────────────────
# Amalgamation: freight.toml wraps it as a [[lib]] so freight compiles sqlite3.c.
# SQLITE_THREADSAFE=0 and SQLITE_OMIT_LOAD_EXTENSION remove pthread/dl deps.

build_sqlite3() {
    local name="sqlite3" version="3.45.1"
    local vernum="3450100"
    local stage="$WORK/${name}-${version}"
    echo "→ downloading $name $version (amalgamation)"

    local zipfile="$WORK/sqlite-amalgamation-${vernum}.zip"
    download "https://www.sqlite.org/2024/sqlite-amalgamation-${vernum}.zip" "$zipfile"

    local amdir="$WORK/sqlite-amalgamation-${vernum}"
    unzip -q "$zipfile" -d "$WORK"

    mkdir -p "$stage/include" "$stage/src"
    cp "$amdir/sqlite3.h"    "$stage/include/"
    cp "$amdir/sqlite3ext.h" "$stage/include/"
    cp "$amdir/sqlite3.c"    "$stage/src/"

    cat > "$stage/freight.toml" <<TOML
[package]
name    = "$name"
version = "$version"

[language.c]
std = "c11"

[compiler]
defines = ["SQLITE_THREADSAFE=0", "SQLITE_OMIT_LOAD_EXTENSION"]

[lib]
srcs = ["src/sqlite3.c"]
TOML

    make_tarball "$name" "$version"
}

# ── Build all ─────────────────────────────────────────────────────────────────

build_nlohmann_json
build_sqlite3

echo ""
echo "Package tarballs ready:"
find "$OUT_DIR" -name "*.tar.gz" | sort | sed 's/^/  /'
