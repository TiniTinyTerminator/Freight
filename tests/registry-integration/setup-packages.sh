#!/usr/bin/env bash
# Build libvec and libstr from source and package them as tarballs
# that the test registry server will serve.
#
# Output: registry/data/packages/{name}/{version}.tar.gz
#
# Tarball layout (strip-components=1 applied by freight during extract):
#   libvec-1.0.0/
#   ├── include/vec.h
#   └── lib/libvec.a

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SRC_DIR="$SCRIPT_DIR/packages-src"
OUT_DIR="$SCRIPT_DIR/registry/data/packages"
STAGING="$(mktemp -d)"
trap 'rm -rf "$STAGING"' EXIT

CC="${CC:-gcc}"

build_package() {
    local name="$1"
    local version="$2"
    local pkg_src="$SRC_DIR/$name"
    local pkg_stage="$STAGING/${name}-${version}"

    echo "→ building $name $version"

    mkdir -p "$pkg_stage/include" "$pkg_stage/lib"

    # Copy headers
    cp "$pkg_src/include/"*.h "$pkg_stage/include/"

    # Compile each .c file into an object, then archive
    local objects=()
    for src in "$pkg_src/src/"*.c; do
        local obj="$STAGING/$(basename "${src%.c}").o"
        "$CC" -c "$src" -I"$pkg_src/include" -O2 -fPIC -o "$obj"
        objects+=("$obj")
    done
    # Convention: the archive is named exactly after the package (libvec.a, libstr.a).
    # freight strips the leading "lib" when generating -l flags → -lvec, -lstr.
    ar rcs "$pkg_stage/lib/${name}.a" "${objects[@]}"

    # No .pc file: freight falls back to scanning include/ and lib/ directly,
    # which works correctly with absolute paths resolved at build time.

    # Package
    mkdir -p "$OUT_DIR/$name"
    local tarball="$OUT_DIR/$name/${version}.tar.gz"
    tar czf "$tarball" -C "$STAGING" "${name}-${version}"
    echo "   wrote $tarball ($(du -sh "$tarball" | cut -f1))"
}

build_package libvec 1.0.0
build_package libstr 1.0.0

echo ""
echo "Package tarballs ready:"
find "$OUT_DIR" -name "*.tar.gz" | sort | sed 's/^/  /'
