# freight TODO

Sub-TODO files exist for specific areas:
- `docs/TODO.md` — VS Code extension, system lib cache, workspace improvements
- `src/bin/freight/tui/TODO.md` — TUI screens (outdated picker, tree, build panel, test runner)
- Workspace-level `AGENTS.md` — cross-crate tasks and language/toolchain gaps

This file covers items not tracked elsewhere.

---

## High priority

### ~~Compiler version gating for language standards~~
Done. `TemplateDef` now has `standard_min_versions`; `CompilerTemplate::check_standard_floor`
checks the floor; `compile_one` rejects unsupported standards with `FreightError::OptionError`
before invoking the compiler.

Floors set for GCC (g++/gcc/gfortran) and Clang (clang++/clang):
- c++20 ≥ GCC 10 / Clang 10; c++23 ≥ GCC 12 / Clang 14; c++26 ≥ GCC 14 / Clang 17
- c17 ≥ GCC 8 / Clang 6; c23 ≥ GCC 14 / Clang 17; f2018 ≥ gfortran 8

---

## Build pipeline

### ~~`has_lang` is duplicated~~
Done. Extracted `pub(super) fn has_lang` to `build/mod.rs`; both `compile.rs` and `link.rs` call `super::has_lang`.

### ~~Linker priority list is fragile~~
Done. `LINK_PRIORITY: &[&str]` constant defined in `link.rs`; `select_linker` references it.

### ~~Missing `BuildEvent` for whole-program mode~~
Done. `BuildEvent::Compiling` emitted before `gnatmake` invocation in `compile.rs`.

---

## Migration tool

### ~~Autotools: SUBDIRS not auto-detected~~
Done. `migration/autotools.rs` now parses `SUBDIRS = ...`, recurses into each
listed directory that has a `Makefile.am`, writes a `freight.toml` there, and
adds any library targets as `{ path = "subdir" }` deps in the root manifest.
Subdirs with only `bin_PROGRAMS` are migrated independently but not added as
deps. Missing subdirs are skipped with a warning.

### ~~CMake migration: platform-conditional block handling~~
Done. `cmake_lossless::eval::platform_condition` identifies `if(WIN32)`, `if(APPLE)`,
`if(UNIX)`, etc. and routes their deps to `[os.windows.dependencies]`,
`[os.macos.dependencies]`, `[os.unix.dependencies]` in the emitted `freight.toml`.
`elseif` chains each get their own scope; `else` falls through to unconditional.
Defines/includes inside platform blocks are still dropped (freight.toml has no
per-platform define syntax).

---

## Testing

- Integration test for mixed-language linking: build `examples/multi-lang` and
  `examples/tri-lang` via the `freight-core` API; assert the binary exits 0.
- Unit test for `whole_program: true` branch in `compile.rs` / `link.rs`.
- Unit test for language auto-detection via `has_lang` (extension → linker family).
- Tests for compiler version gating once implemented.

---

## Examples / language support

See `AGENTS.md` for full detail. Summary of what's missing:

| Example         | Status / Blocker                                      |
|-----------------|-------------------------------------------------------|
| OpenCL          | ✓ Done — `examples/opencl-hello/`                    |
| CUDA            | ✓ Done — `examples/cuda-hello/`                      |
| D               | ✓ Done — `examples/d-hello/` (ldc2 + dmd)            |
| ObjC / ObjC++   | ✓ Done — `examples/objc-hello/`, `examples/objcpp-hello/` |
| HIP             | ✓ Done — `examples/hip-hello/` (requires ROCm hardware) |
| ISPC            | ✓ Done — `examples/ispc-hello/`                       |
| GDC             | ✓ Available (`gdc` 16.1.1); `d-hello` already works  |
| MSVC            | Windows machine needed                                |
| nvfortran       | NVIDIA HPC SDK needed                                 |

---

## Documentation

- `docs/manifest-reference.md`: add `[language.ada]` section.
- `examples/README.md`: keep prerequisite notes current as new toolchain examples land.
