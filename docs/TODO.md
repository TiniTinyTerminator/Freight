# TODO

---

## Big tasks

### B1 — Registry client integration
The `freight-registry` server lives in a separate repository and is fully operational. Multiple-registry
client support is done. All publisher/consumer CLI commands are wired to real HTTP calls:
`freight search`, `freight info`, `freight login`, `freight publish`, `freight yank`, `freight register`.

Remaining: implement `freight fetch` for registry version deps (download tarballs to `.deps/` and
record `source = "registry+<url>"` + checksum in `freight.lock`), and add semver resolution +
lockfile pinning to `freight add` so `version = "1.2"` deps work end-to-end.
Blocks: registry-backed `version = "x.y"` resolution in `freight build`.

### B2 — VS Code extension
Activate on `freight.toml`, delegate to `freight lsp` for diagnostics, completions, and
go-to-definition. LSP currently implemented; remaining work is the extension packaging and
Marketplace publish. Also add inlay hints and `freight.toml` schema validation.

### B3 — Progress callbacks / structured build events ✓ done
`BuildEvent` enum + `Progress = Arc<dyn Fn(BuildEvent) + Send + Sync>` threaded through the
entire build pipeline. CLI translates events to coloured output. GUI/TUI/LSP can subscribe
without parsing stdout. All `println!`/`eprintln!` removed from `freight-core` build paths.

### B4 — Slot-based substitution ✓ done
`provides = [...]` currently only detects conflicts. Full substitution: the dep declared
closest to the root wins; same-depth conflicts remain a hard error. Required before large
dependency graphs with BLAS/LAPACK-style provider aliases are usable.

### B5 — Package lookup chain expansion ✓ done
Version deps resolve via `pkg-config → conan → vcpkg → system-lib stub`. `repo = "system"|"conan"|"vcpkg"|"pkg-config"` pins a specific resolver. System PM detection (apt/brew/dnf/pacman/zypper/winget) emits install hints on failure. `conan.rs`, `system_pm.rs`, and `system_libs.rs` modules added. 24 built-in stubs in `toolchains/system-libs/` cover common OS primitives (pthread, ws2_32, libm, dl, rt, d3d11, …). Users can add stubs to `~/.freight/toolchains/system-libs/`.
Remaining: internal system cache registry (index on first install; skip probing on rebuild).

### B6 — `freight bench` ✓ done
`bench` profile (release + debug, no strip), run binaries matching `bench_*` or in `benches/`,
print a timing table. Optional Criterion integration via a flag.

### B7 — Support multiple compiler versions
The toolchain currently doesnt expect multiple versions of the same compiler. Since some systems dont support newer versions of compilers, older versions are required to be installed sometimes for compatibillity. The toolchains must thus show every version of a compiler that is available on the system, so the user can choose which one is needed.

---

## Small tasks

### S1 — Tab completion ✓ done
Generate shell completions (bash, zsh, fish) for all subcommands and their options.
When the user presses tab under `freight add`, offer known dep keys; under
`freight toolchain use`, offer detected compiler names.

### S2 — `freight outdated`
Compare locked path-dep revs and registry versions against latest available.
Print a coloured table. Analogous to `cargo outdated`.

### S3 — Better compiler diagnostics ✓ done
GCC/Clang column markers and MSVC error codes are parsed into concise
summaries with clickable `file:line:col` references and source snippets.

### S4 — `freight graph` ✓ done
Emits the dependency graph as DOT or Mermaid to stdout or a file. Useful for
auditing transitive deps in large projects.

### S5 — `--emit asm` ✓ done
`freight build --emit asm` — runs an extra `-S` pass for each source file and
writes `.s` files to `target/{profile}/asm/`, preserving the source tree structure.
Skips pure-assembler sources (gas/nasm/yasm). Non-fatal: failures are surfaced as
warnings rather than aborting the build. Normal object compilation and linking still happen.

### S6 — `--time-passes` ✓ done
`freight build --time-passes` — instruments every `compile_one` call, emits
`BuildEvent::Timing` events, then prints a per-file table sorted slowest-first after
the build completes. Uses the `FREIGHT_TIME_PASSES` env var internally so rayon workers
can record timings without changing function signatures.

### S7 — Profile inheritance ✓ done
`[profile.custom] inherits = "release"; debug = true` — fully implemented:
`resolve_profile` walks the `inherits` chain (max 16 hops, cycle-safe), merges
parent→child with child fields winning when `Some`/non-empty.

### S8 — Sanitizer preset override via CLI ✓ done
`freight test --sanitize address,undefined` — `apply_sanitize_override` patches the
active profile's sanitize list; `--sanitize` is wired through `build`, `test`, and `run`.

### S9 — `rerun_if` in `build.freight`
`rerun_if_changed("path")` and `rerun_if_env_changed("VAR")` — skip re-running
the build script when declared inputs haven't changed.

### S10 — `compile_commands.json` incremental update ✓ done
Cache the previous output and source mtimes under `target/{profile}/`; only re-emit entries
for sources that changed or when command-affecting settings change.

### S11 — `FREIGHT_SYSROOT` env auto-propagation ✓ done
When `FREIGHT_SYSROOT` is set, inject `--sysroot=` automatically without
requiring `[compiler] sysroot` in the manifest.

### S12 — Template evaluation cache ✓ done
Cache parsed `.rhai` template results to `~/.freight/template-cache.msgpack`.
Cache key = per-file content hash + base-file hash. `CompilerTemplate` and
serializable nested template metadata derive `Serialize`/`Deserialize`; templates
with runtime option handlers are evaluated live so their callbacks remain active.

### S13 — macOS / Windows distribution packages ✓ done
`freight package --target x86_64-windows` produces a `.zip` instead of `.tar.gz`.
Windows targets emit and install `.exe` binaries, with shared-library packaging
using the existing Windows DLL/import-lib layout for MinGW or Windows sysroots.

### S14 — Unity / jumbo builds ✓ done
`[compiler] unity = true` — concatenate all TUs per language into one translation
unit via `#include`. Trades incremental speed for full-build speed and better cross-TU
inlining. Applies to C, C++, CUDA, HIP, OpenCL; other languages compile individually.
Per-dep override: `mylib = { path = "../mylib", unity = true }`.
C++20 named-module projects skip unity (modules have their own dependency ordering).

### S15 — Workspace per-member feature flags ✓ done
`freight build -p mylib --features tls` — build/test/bench/run a single workspace
member and pass features to it. `freight run -p myapp` runs a member's binary.
Remaining: workspace-level `[patch]` overrides, `freight workspace graph` visualisation.
