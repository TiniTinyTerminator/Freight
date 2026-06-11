# Include-hygiene — implementation audit

Running log of what changed while implementing
[`include-hygiene.md`](include-hygiene.md), so the work can be traced and backed
out commit-by-commit. Newest entries at the top.

## Status

- **Phase 1 (warn):** ✅ complete and verified end-to-end.
- **Phase 2 (build enforcement):** not started.
- **Phase 3 (system libs + stdlib matching):** stdlib matching folded into Phase 1.

## Log

### Step 6 — also cover `import` / `#import` (header-bringing forms)

- `parse_includes` now recognises, in addition to `#include`:
  - `#import <h>` / `#import "h"` (Objective-C),
  - `import <h>;` / `import "h";` and `export import …;` (C++20 header units).
- Named-module imports (`import foo;`, `import std;`) carry no header token and
  are skipped — resolving a module name to a package needs a module→package map
  (a later step; noted in the plan).
- 1 new test (`parse_includes_handles_import_and_objc_forms`); **11 module tests.**
- End-to-end verified: `import <pthread.h>;` is flagged with the same
  undeclared-include warning; `import std;` and `<vector>` are not.

### Step 5 — LSP wiring (Phase 1 complete)

- `DiagCache` gained a `freight` field; both merge sites now chain
  clangd + tidy + freight.
- `ServerState.system_include_dirs: Option<Vec<PathBuf>>` (probed once, cached).
- `Server::compute_include_hygiene(uri, text)` — runs `check_includes` and stores
  the results as `source:"freight" code:"undeclared-include"` diagnostics
  (severity from the lint level: warn→2, deny→1; allow→cleared/no-op).
- Helpers: `undeclared_include_level()` (reads `[lints]` from the project
  manifest), `declared_dirs_and_compiler()` (parses `-I`/`-isystem`/`-iquote` and
  argv[0] from compile_commands.json), `cached_system_dirs()`.
- Called from `handle_did_open` / `handle_did_change` (full-text sync) /
  `handle_did_save`.
- **End-to-end verified** against the `freight lsp` binary on a real project
  (`/tmp/ih`): `#include <pthread.h>` → one Warning on the right span; `<vector>`
  and `<cstdio>` (stdlib) not flagged; `[lints] undeclared-include = "allow"`
  silences it (0 diagnostics).
- Works with the clang bridge gated off (bridge-free resolution path).
- Suite: my unit tests all pass. The 4 failing `*_hello_builds` integration tests
  are pre-existing/environmental (they invoke `freight build`; they fail
  identically with my changes stashed — the sandbox can't run them).

### Step 4 — system-dir probe + `check_includes` orchestration

- `system_include_dirs(compiler, language)` runs `<cc> -E -x <lang> - -v` and
  `parse_search_dirs()` extracts the `#include <...> search starts here:` block
  (handles macOS `(framework directory)` suffix). Empty on failure → safe (an
  unconfirmed header just isn't flagged).
- `UndeclaredInclude { line, start_col, end_col, spelling }`.
- `check_includes(source, file_dir, declared_dirs, system_dirs, language)` ties
  parse → resolve (declared then system) → classify → finding. Flags only headers
  that are undeclared **and** present; skips declared, stdlib (by name), and
  not-found (clangd's file-not-found).
- 2 new tests (system-block parse; full flow flags only `<pthread.h>`). **10
  include_policy tests total.**
- The whole classification/resolution logic is now complete and tested in
  isolation. Remaining: wire `check_includes` into the LSP diagnostic publish
  (gather declared_dirs from the file's compile command, probe system dirs once).

### Step 3 — `#include` directive parser + resolver (`include_policy.rs`)

- `IncludeDirective { name, angled, line, start_col, end_col }` (0-based, span
  includes delimiters).
- `parse_includes(source)` — line scan with a `/* */` + `//` comment state
  machine so commented-out includes aren't flagged. Handles `#  include`.
- `resolve_include(directive, file_dir, search_dirs)` — quote includes search
  the file's dir first, then the search path; returns the first existing file.
- 3 new tests (directive extraction incl. columns, multiline-block-comment skip,
  quote/angle/missing resolution). 8 include_policy tests total.
- **Resolution strategy (decided, bridge-free):** the LSP passes the file's
  compile-command `-I` dirs (declared project+dep) plus the compiler's probed
  system dirs as `search_dirs`. Resolved-under-declared → allowed; std-name →
  allowed; resolved-under-system → undeclared; unresolved → skip (clangd already
  reports file-not-found). Avoids depending on the (gated-off) bridge.

### Step 2 — `[lints]` manifest table

- `src/manifest/types.rs`: added `LintLevel { Allow, Warn(default), Deny }`
  (serde lowercase) and `LintsConfig { undeclared_include: LintLevel }`
  (`#[serde(rename = "undeclared-include")]`). New `Manifest.lints` field
  (`#[serde(default)]`).
- Re-exported `LintLevel`, `LintsConfig` from `src/manifest/mod.rs`.
- Default is `warn` even when `[lints]` is absent (matches the decision).
- 2 parse tests in `validate.rs` (default = warn; deny/allow parse).
- Test helpers build manifests from TOML strings, so no struct-literal breakage.

### Step 1 — classification core (`src/build/include_policy.rs`)

- New module `include_policy` (registered in `src/build/mod.rs`).
- `IncludeClass { Project, Dependency(name), Stdlib, Undeclared }`.
- `Language { C, Cxx }` + `Language::from_path` (`.c` → C, else C++ superset).
- `IncludeAllowlist::new(language, project_roots, dep_roots)` (canonicalises) +
  `classify(header_name, resolved_abs)`.
  - Order: project root → dep root → std-name → undeclared, so a project/dep file
    named like a std header is attributed to its owner (refines the plan's
    std-first order).
- Static `C_HEADERS` / `CXX_HEADERS` tables (C89–C23, C++98–C++23); C++ set =
  C++ ∪ C headers. Built once via `OnceLock`.
- 5 unit tests pass: stdlib-by-name, POSIX→undeclared, third-party→undeclared,
  project/dep override name, C excludes C++ headers.
- **Not yet wired** to the real resolver — `IncludeAllowlist::new` takes roots
  directly; a `from_resolved(manifest, …)` constructor comes in the wiring step.

### Step 0 — design committed (freight 3690123)

- `docs/include-hygiene.md` — the plan, with the decided stdlib-by-name policy.
- `docs/include-hygiene-audit.md` — this file.

## Decisions (frozen)

- **Stdlib-only is implicit**, matched by header *name* per language/`std` (not by
  directory — glibc and POSIX share `/usr/include`). POSIX/OS headers require a
  declared dependency.
- **Default lint level `warn`**, configurable via
  `[lints].undeclared-include = "allow" | "warn" | "deny"`.
- Diagnostics `source = "freight"`, `code = "undeclared-include"`.

## Phase 1 task checklist

- [ ] `src/build/include_policy.rs` — `IncludeAllowlist`, `IncludeClass`,
      `classify(spelling, resolved_abs)`, std-header tables, unit tests.
- [ ] `[lints].undeclared-include` in the manifest model + validation default.
- [ ] `src/lsp/include_hygiene.rs` — inclusion list → classified diagnostics.
- [ ] Hook into the LSP diagnostic merge (`src/lsp/mod.rs`).
- [ ] Fixture + integration test (`<zlib.h>` and `<pthread.h>` → one warning each).
- [ ] `manifest-reference.md` `[lints]` section.
