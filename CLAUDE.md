# Crane — Build Tool & Package Manager

## What is crane?

Crane is a Cargo-inspired build tool and package manager for compiled languages that target GCC or Clang: C, C++, Fortran, assembly, CUDA, HIP, OpenCL, and others. It aims to be the single tool you need to build, test, and publish native code — no Makefile, no CMake, no Ninja required.

The project is written in Rust.

---

## Core philosophy

- **No external build system** — crane owns the entire build graph internally. No Ninja, no Make underneath.
- **Declarative compiler templates** — each compiler (gcc, clang, nvcc, gfortran, nasm…) is described in a `.toml` file that maps abstract settings to real flags. Adding a new compiler = writing a TOML, not writing Rust.
- **One tool, many languages** — file extension routes to the right compiler automatically. A single project can mix `.cpp`, `.c`, `.f90`, `.asm`, `.cu` files.
- **Incremental by default** — mtime dirty checking via Makefile `.d` dep files (source + all included headers), parallel compilation via rayon.
- **C++20 modules ready** — template infrastructure exists (`ModuleStyle::Gcc`, `ModuleStyle::Clang`), full module DAG pipeline is a future phase.

---

## Naming conventions

| Name | Meaning |
|---|---|
| `crane` | The CLI binary |
| `crane.toml` | Project manifest |
| `crane.lock` | Auto-generated lockfile (commit this) — not yet implemented |
| `build.crane` | Optional pre-build hook script — not yet implemented |
| `~/.crane/` | Global cache directory |
| `crane.dev` | The package registry — not yet implemented |

---

## Repository layout

```
crane/
├── Cargo.toml                  # workspace root
├── CLAUDE.md                   # this file
├── crates/
│   ├── crane/                  # binary crate — CLI entry point
│   │   └── src/main.rs
│   └── crane-core/             # library crate — all build logic
│       └── src/
│           ├── lib.rs
│           ├── error.rs
│           ├── new.rs          # crane new / crane init
│           ├── manifest/       # crane.toml parsing + validation
│           │   ├── mod.rs
│           │   ├── types.rs
│           │   ├── find.rs
│           │   └── validate.rs
│           ├── toolchain/      # compiler detection + templates
│           │   ├── mod.rs
│           │   ├── template.rs
│           │   ├── detect.rs
│           │   └── cache.rs
│           └── build/          # compilation + linking orchestration
│               ├── mod.rs      # cmd_build, cmd_run, cmd_test, cmd_clean
│               ├── compile.rs  # source → object, parallel via rayon
│               ├── link.rs     # object → binary / .a / .so
│               ├── discover.rs # walkdir source discovery
│               └── deps.rs     # dep graph resolution + topo sort
├── compiler-templates/         # bundled .toml files per compiler
│   ├── gcc.toml                # g++ (C++ linker), gcc (C compiler override)
│   ├── clang.toml              # clang++ (C++ linker), clang (C compiler override)
│   ├── gfortran.toml
│   ├── nvcc.toml
│   ├── hipcc.toml
│   ├── icpx.toml               # Intel oneAPI C++
│   ├── opencl.toml
│   ├── ispc.toml               # Intel SPMD
│   ├── nasm.toml               # x86/x86_64 assembly
│   └── gas.toml                # GNU assembler (.s files)
└── examples/
    ├── hello-cpp/              # basic C++ project with tests
    ├── multi-lang/             # C + C++ mixed project
    └── with-deps/              # project with a path dependency
```

---

## crane.toml — manifest format

```toml
[package]
name        = "myproject"
version     = "0.1.0"
authors     = ["You <you@example.com>"]
description = "A short description"
license     = "MIT"

# Target architecture and CPU features (optional)
[target]
arch           = "x86_64"          # drives [arch_flags] in compiler templates
cpu_extensions = ["avx2", "fma"]   # produces -mavx2 -mfma for gcc/clang

# Per-language settings; key matches the template's [linking.<key>] name
[language.cpp]
std = "c++20"

[language.c]
std = "c17"

[language.fortran]
# std is optional for Fortran

[lib]
type    = "static"   # static | shared | header-only
src     = "src/"
include = "include/"

[[bin]]
name = "myproject"
src  = "src/main.cpp"

[dependencies]
# Version deps are fetched from crane.dev (not yet implemented)
libopenblas = "0.3"
# System deps link against a system-installed library
openssl     = { system = "openssl", version = ">=3.0" }
# Path deps compile a sibling crane project and link its archive
myutils     = { path = "../myutils" }

[dev-dependencies]
libcheck = "0.15"

[compiler]
backend   = "auto"   # auto | gcc | clang | gfortran | nasm | …
opt-level = 2
debug     = false
warnings  = "all"    # none | default | all | error
defines   = ["USE_BLAS"]
flags     = []

[compiler.includes]
paths = ["include/", "third_party/include/"]

[profile.dev]
opt-level = 0
debug     = true
sanitize  = ["address", "undefined"]

[profile.release]
opt-level = 3
lto       = true
strip     = true
debug     = false
```

---

## Compiler template format

Each compiler is described by a flat `.toml` file — no `[compiler]` nesting. Crane loads all `.toml` files from `compiler-templates/` at startup. Adding a new compiler = writing a new TOML, not touching Rust.

```toml
# compiler-templates/gcc.toml

name          = "gcc"
binary        = "g++"          # binary used for linking
version_arg   = "--version"
version_regex = "\\b(\\d+\\.\\d+\\.\\d+)\\b"

[extensions]
handles = [".cpp", ".cc", ".cxx", ".c++", ".c"]

[flags]
opt.0            = "-O0"
opt.1            = "-O1"
opt.2            = "-O2"
opt.3            = "-O3"
debug.true       = "-g"
debug.false      = ""
warnings.none    = ""
warnings.default = "-Wall"
warnings.all     = "-Wall -Wextra -Wpedantic"
warnings.error   = "-Wall -Wextra -Wpedantic -Werror"
lto.true         = "-flto"
lto.false        = ""
strip.true       = "-s"
strip.false      = ""
sanitize         = "-fsanitize={values}"
cpu_extension    = "-m{name}"   # each cpu_extension → -mavx2, -mfma, etc.

[standards]
"c11"   = "-std=c11"
"c17"   = "-std=c17"
"c23"   = "-std=c23"
"c++17" = "-std=c++17"
"c++20" = "-std=c++20"
"c++23" = "-std=c++23"

# Optional: maps arch names → compiler flags (mainly for assemblers)
[arch_flags]
# gcc/clang leave this empty; arch is handled via target_triple

[structure]
include_dir  = "-I{path}"
define       = "-D{name}"
define_value = "-D{name}={value}"
output       = "-o {path}"
compile_only = "-c"
dep_file     = "-MMD -MF {path}"   # generates Makefile dep file for header tracking

[modules]
supported     = true
enable_flag   = "-fmodules-ts"
compile_miu   = "-fmodule-output={pcm_path}"
import_module = "-fmodule-file={name}={pcm_path}"

[passthrough]
enabled = false
prefix  = ""

# A template can claim multiple language keys.
# [linking.<key>] declares ABI + linker compatibility for that language.
# compiler = "..." overrides the top-level binary for *compilation* only.
[linking.c]
abi      = "c"
compiler = "gcc"          # C files compiled with gcc, not g++
compatible = ["fortran"]
linker     = ""
extensions = [".c"]

[linking.cpp]
abi        = "c++"
compatible = ["c", "fortran"]
linker     = ""
extensions = [".cpp", ".cc", ".cxx", ".c++"]
```

### Assembly template example (NASM)

```toml
# compiler-templates/nasm.toml

name          = "nasm"
binary        = "nasm"
version_arg   = "--version"
version_regex = "NASM version (\\d+\\.\\d+\\.\\d+)"

[extensions]
handles = [".asm", ".nasm"]

[arch_flags]
"x86_64" = "-f elf64"    # [target] arch = "x86_64" → -f elf64
"x86"    = "-f elf32"

[linking.asm]
abi        = "c"
compatible = ["c", "cpp", "fortran"]
linker     = ""
extensions = [".asm", ".nasm"]
```

---

## Build engine — internal pipeline

```
crane build
  │
  ├── 1. Parse + validate crane.toml
  ├── 2. Detect toolchain (probe $PATH, load compiler templates, version cache)
  ├── 3. Resolve dependency graph (topo sort, compile path deps in order)
  │       ├── compile each dep → archive (.a)
  │       └── collect dep include dirs
  ├── 4. Walk src/ — discover sources by file extension → language key
  ├── 5. Dirty check each source (mtime of .o vs source + headers via .d file)
  ├── 6. Compile dirty sources in parallel (rayon)
  │       ├── select compiler by lang_key (respects backend = "auto" or named)
  │       ├── resolve compile binary (gcc.toml: g++ for linking, gcc for .c files)
  │       └── emit .d dep file alongside .o for next-run header tracking
  └── 7. Link all .o + dep .a files → binary / .a / .so
```

---

## Dependency kinds

| Kind | crane.toml syntax | How it works |
|---|---|---|
| Path | `{ path = "../mylib" }` | Compiles the dep project, links its `.a` archive |
| System | `{ system = "openssl" }` | Passes `-l{name}` to the linker |
| Version | `"0.3"` | Fetched from crane.dev (not yet implemented) |
| Git | `{ git = "..." }` | Not yet implemented |

Path dependencies are non-recursive: crane checks that a dep's own deps are already present in `.deps/` but does not download them. The topo sort ensures deps are compiled in the right order.

---

## CLI commands

```
crane new <name> --lang <lang>    scaffold a new project              ✓ implemented
crane init                        init crane in current directory     ✓ implemented
crane build [--release]           build the project                   ✓ implemented
crane run [--release] [-- <args>] build and run default binary        ✓ implemented
crane test [<name>]               build and run tests                 ✓ implemented
crane clean                       wipe target/                        ✓ implemented
crane check                       validate crane.toml                 ✓ implemented
crane toolchain list              show detected compilers             ✓ implemented

crane add <package>[@version]     add a dependency                    ✗ not yet
crane remove <package>            remove a dependency                 ✗ not yet
crane update [<package>]          update deps within semver ranges    ✗ not yet
crane fetch                       download deps without building      ✗ not yet
crane tree                        print dependency tree               ✗ not yet
crane info <package>              show package metadata               ✗ not yet
crane search <query>              search crane.dev                    ✗ not yet
crane migrate [--from <format>]   import existing build system        ✗ not yet
crane login                       authenticate with crane.dev         ✗ not yet
crane publish                     upload package to registry          ✗ not yet
crane yank <version>              yank a published version            ✗ not yet
crane toolchain add <name>        install a compiler template         ✗ not yet
crane toolchain use <name>        set default compiler backend        ✗ not yet
```

---

## Development roadmap

### Phase 1 — CLI skeleton ✓ COMPLETE
- [x] Cargo workspace: `crane` (bin) + `crane-core` (lib)
- [x] `clap` wiring — all subcommands stubbed
- [x] `CraneError` enum with `thiserror`
- [x] Coloured output helpers: success `✓`, warning `⚠`, error `✗`
- [x] `crane new <name> --lang <lang>` — scaffold directory + crane.toml + hello-world src
- [x] `crane init [--lang <lang>]` — init in current dir, auto-detects language from existing files

### Phase 2 — Manifest ✓ COMPLETE
- [x] Serde structs for every crane.toml section (`manifest/types.rs`)
- [x] Parse + validate with `toml_edit`
- [x] `crane check` — validate manifest, print clear errors or a summary
- [x] `find_manifest_dir` — walk up the directory tree to locate `crane.toml`
- [x] `Manifest::build_settings_for(profile)` — convert manifest + profile into `BuildSettings`
- [x] ABI compatibility validation for path dependencies
- [x] C/C++ standard consistency validation (c23 must not be newer than c++17, etc.)

### Phase 3 — Compiler detection ✓ COMPLETE
- [x] Probe `$PATH` for known compiler binaries
- [x] Load + deserialize compiler template `.toml` files at runtime
- [x] `CompilerTemplate` struct + `assemble_flags()` method (pure, unit-tested)
- [x] `crane toolchain list`
- [x] Toolchain version cache (`~/.crane/toolchain-cache.json`, mtime-validated)
- [x] Template system supports: gcc, clang, gfortran, nvcc, hipcc, icpx, opencl, ispc, nasm, gas

### Phase 4 — Build engine ✓ COMPLETE
- [x] Source discovery with `walkdir` — extension → language key routing
- [x] Parallel compilation via `rayon`
- [x] Mtime dirty checking — source vs object, headers via `.d` dep files
- [x] `.d` dep file generation (`-MMD -MF`) for transitive header tracking
- [x] Linker invocation — binary, static lib (`.a`), shared lib (`.so`)
- [x] `crane build` + `crane run` end-to-end
- [x] `crane test` — compiles test files, links against project objects (excluding `main()`), runs each test binary
- [x] `crane clean` — wipes `target/`
- [x] Multi-language builds — C + C++ in one project, each compiled with the right binary

### Phase 5 — Dependencies ✓ COMPLETE
- [x] Path dependency resolution — compile dep, archive to `.a`, link into project
- [x] System dependency linking — `{ system = "..." }` → `-l{name}`
- [x] Dependency graph with topological sort (Kahn's algorithm)
- [x] Cycle detection with error
- [x] `.deps/<name>/` folder convention for version-pinned deps
- [x] Transitive dep checks — errors if a dep's dep is not present, does not fetch recursively
- [x] Dep include dirs accumulated in topo order for multi-level dep builds

### Phase 6 — Assembly + target config ✓ COMPLETE
- [x] NASM template (`nasm.toml`) — `.asm`/`.nasm`, x86/x86_64 arch flags
- [x] GAS template (`gas.toml`) — `.s`, x86/x86_64/aarch64 arch flags
- [x] `[target]` section in crane.toml — `arch` and `cpu_extensions`
- [x] `arch` drives `[arch_flags]` lookups in templates (e.g. `-f elf64` for NASM)
- [x] `cpu_extensions` produces per-extension flags (e.g. `-mavx2`, `-mfma` via `cpu_extension = "-m{name}"`)
- [x] Unified C/C++ templates — `compiler = "gcc"` override in `[linking.c]` so C files are not compiled with `g++`

### Phase 7 — Git workflow ✓ COMPLETE
- [x] Feature-branch workflow: `feature/<name>` branches off master, merged when done
- [x] `feature/assembly-support` — first feature branch, contains Phase 6 work

### Phase 8 — C++20 modules (planned)
- [ ] Scan source files for `export module` / `import` statements
- [ ] Classify files as MIU / MImplU / TU
- [ ] Build module DAG (nodes = files, edges = `import` dependencies)
- [ ] Compile MIUs first → produce `.pcm` files
- [ ] Pass `-fmodule-file=` to dependents
- [ ] GCC vs Clang strategy differences (one step vs two steps)
- [ ] `crane build` respects module order in parallel batches

### Phase 9 — Registry + lockfile (planned)
- [ ] `crane.lock` read/write (deterministic dep pinning)
- [ ] `crane fetch` — download deps from crane.dev into `.deps/`
- [ ] `crane add / remove / update` — manifest mutation + lockfile update
- [ ] `crane search / info` — registry queries
- [ ] `crane login / publish / yank` — publishing workflow

### Phase 10 — Cross-compilation (planned)
- [ ] `[compiler] target = "aarch64-linux-gnu"` → `--target=` / `-march=` flags
- [ ] `[compiler] sysroot = "/opt/sysroot"` → `--sysroot=`
- [ ] Prebuilt dep filtering by `targets = [...]` in crane.toml
- [ ] `crane toolchain add` — install a cross-compiler template

### Phase 11 — Importer (planned)
- [ ] `crane migrate` — detect and import existing build system
- [ ] CMake importer (`cmake-parser` crate)
- [ ] Makefile importer (`makefile-lossless` crate)
- [ ] Meson importer (regex-based)
- [ ] Unrecognised constructs → `# CRANE: could not import — review manually`

---

## Architecture rules

1. **`crane` crate is thin** — only `main.rs`, CLI parsing, delegates everything to `crane-core`
2. **All logic in `crane-core`** — testable without the CLI
3. **Compiler templates are runtime data** — loaded from `compiler-templates/` directory, not hardcoded
4. **One template per toolchain, not per language** — `gcc.toml` handles both C and C++; the `compiler` field in `[linking.c]` overrides which binary compiles that language
5. **DAG cycles = hard error** — report the full cycle path
6. **`CompilerTemplate::assemble_flags()` is pure** — no side effects, unit-tested
7. **Never shell out to Make / Ninja / CMake during a build** — crane owns the build graph entirely
8. **Errors use `thiserror` in crane-core, surface at the CLI boundary**
9. **Feature branches** — each new feature gets its own `feature/<name>` branch off `master`

---

## Key Rust dependencies

```toml
[dependencies]
clap          = { version = "4", features = ["derive"] }
owo-colors    = "4"
toml_edit     = "0.22"
serde         = { version = "1", features = ["derive"] }
rayon         = "1"
walkdir       = "2"
regex         = "1"
semver        = "1"
tempfile      = "3"    # test helpers
thiserror     = "1"
```
