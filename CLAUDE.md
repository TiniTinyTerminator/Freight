# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

---

## Build, test, and lint commands

```sh
# Build the entire workspace
cargo build --workspace

# Run all tests (261 as of last count)
cargo test --workspace

# Run a single test by substring match
cargo test --workspace build::discover::tests::finds_cpp_sources_in_src

# Run tests in one crate
cargo test -p freight-core

# Check without producing binaries
cargo check --workspace

# Lint
cargo clippy --workspace

# Format
cargo fmt --workspace
```

---

## Repository layout

```
Cargo.toml                      # workspace root (members: freight, freight-core, freight-doc)
toolchains/                     # runtime Rhai compiler templates — one .rhai per compiler
│   dmd.rhai                    # D (DMD)
│   msvc.rhai                   # MSVC
│   tcc.rhai                    # TCC
│   opencl.rhai                 # OpenCL
│   gnu/                        # gcc, g++, gfortran, gdc, gdb
│   llvm/                       # clang, clang++, flang, ldc2, lldb
│   nvidia/                     # nvcc, nvc, nvc++, nvfortran
│   intel/                      # icpx, ifx, ispc
│   amd/                        # hipcc
│   asm/                        # gas, nasm, yasm
│   languages/                  # shared lang fragments: _c.rhai, _cpp.rhai, _fortran.rhai
│   astyle/ clang-format/ …     # formatter templates
│   cppcheck/ clang-tidy/ …     # linter templates
crates/
│   freight/                    # binary crate — CLI only, no build logic
│   │   src/main.rs             # clap dispatch
│   │   src/output.rs           # coloured print helpers
│   │   src/commands/           # one cmd_* per subcommand
│   freight-core/               # library crate — all build logic, pure functions
│   │   src/build/              # compilation + linking
│   │   src/manifest/           # freight.toml parsing + validation + supports exprs
│   │   src/toolchain/          # template loading, compiler detection, flag assembly
│   │   src/fetch/              # git, http, vcpkg dep fetching
│   │   src/meta/               # foreign dep build orchestration (cmake, make, …)
│   │   src/install.rs          # freight install / freight package
│   │   src/dep_cmds.rs         # freight add/remove/update/fetch/tree
│   │   src/vendor.rs           # parse_triple, static ARCH_TOKENS/OS_TOKENS tables
│   freight-doc/                # doc extraction + rendering (HTML, Markdown, LaTeX, PDF)
docs/                           # documentation
examples/                       # fully buildable example projects
```

---

## Architecture

### Two-crate split
`freight` owns the CLI (clap, `output.rs` colour helpers, `commands/`). Each `cmd_*` function reads cwd, calls a pure function in `freight-core`, and prints the result. `freight-core` is a library: no `print!` in logic paths, no clap dependency. The sole exception is inline `println!` in the build engine for progress output, pending a future callback abstraction.

### Compiler templates (Rhai)
Every compiler is described by a `.rhai` file in `toolchains/`. The script sets scalar variables (`binary`, `name`, `family`, …) and lookup tables (`opt["0"]`, `warnings["all"]`, …), then registers `compiler_option` / `language_option` callbacks for non-standard keys. `CompilerTemplate::assemble_flags(settings)` is pure and unit-tested. Base files (`_gnu-base.rhai`, `_llvm-base.rhai`, `_asm-base.rhai`) are pulled in by `include "path"` at the top of child scripts. Adding a new compiler = writing a `.rhai` file; no Rust changes required.

### Family / guest model
`load_templates()` reads all `.rhai` files. `detect_all()` probes `PATH`. `group_into_toolchains()` splits detected compilers into:
- **Family groups** — compilers sharing a `family` label (e.g. `"gnu"`, `"llvm"`) are displayed together.
- **Guest extensions** — compilers with `requires_toolchain = ["cpp"]` (nvcc, hipcc, ispc, opencl) are attached to whichever family group satisfies that requirement.
- **Standalones** — compilers with `family = ""` and no `requires_toolchain` appear individually.

### Build pipeline (`build/mod.rs`)
`build_project_at` runs in order:
1. Parse + validate `freight.toml` (supports expression checked here)
2. `detect_all_cached` — probe compilers once, cache version results
3. `resolve_dep_graph` — topo sort, slot-conflict check, compile each dep → `.a`
4. `discover` — walk `src/`, classify by extension → language key via `build_ext_map`
5. C++ module scan (`has_modules`) → phased module-aware build or flat parallel compile
6. `link_targets` — `.o` + dep `.a` → binary / `.a` / `.so`

### Version deps (pkg-config + vcpkg)
`zlib = "1.3.1"` style deps resolve via `meta/mod.rs`: pkg-config first; if that fails, `fetch::vcpkg::resolve_vcpkg_dep` runs `vcpkg install` into `.deps/vcpkg_installed/`. The sentinel file `.deps/vcpkg_installed/.freight/{name}.{triplet}.fetched` prevents re-installs. `VCPKG` env var overrides the vcpkg binary path; `VCPKG_DEFAULT_TRIPLET` overrides the triplet.

### `package.supports` expressions
`manifest/supports.rs` contains a hand-rolled recursive-descent parser for boolean platform expressions (vcpkg-style). Identifiers: `windows`, `linux`, `macos`/`osx`, `unix`, `bsd`, `x86`, `x64`/`x86_64`, `arm`, `arm64`/`aarch64`, `uwp`, and others. When a target triple is set, expressions evaluate against that triple rather than the host.

### `freight doc` TUI
`freight doc` (no `--format`) opens a ratatui + crossterm interactive browser for dependency docs. `freight doc --format html|md|latex|pdf|all` generates files to `target/doc/`.

### Platform-conditional sources
`[os.*]` and `[arch.*]` sections use `srcs` (globs). Files matched by any conditional section's `srcs` glob are added to a `build_exclusion_set` and excluded from the unconditional `src/` walk, so they are never compiled on a non-matching platform.

### Key manifest field names
| Field | Notes |
|---|---|
| `[lib].srcs` | String or list; globs allowed |
| `[lib].hdrs` | Public API headers; include dirs inferred from parent paths |
| `[compiler].includes` | Flat list of `-I` directories (not a subtable) |
| `[os.*].srcs` / `[arch.*].srcs` | Conditional source globs |
| `[target].cpu-extensions` | Hyphenated |
| `pkg-config` | Hyphenated dep field (alias: `pkg_config`) |
| `cmake-args` | Hyphenated (alias: `cmake_args`) |
| `backend` | Replaces old `build_system`; auto-detected from marker files |

---

## Architecture rules (non-negotiable)

1. `freight-core` is a library — no CLI knowledge, no `output.rs`, no clap.
2. `CompilerTemplate::assemble_flags()` is pure — no side effects.
3. Adding a compiler = writing a `.rhai` file, not modifying Rust.
4. DAG cycles (dep graph or module graph) = hard error with full cycle path.
5. All deps live in the root project's flat `.deps/` pool — no nested `.deps/`.
6. Errors use `thiserror` in `freight-core`; surfaced at the CLI boundary.
7. Feature branches: `feature/<name>` off `master`; merge + delete when done.
