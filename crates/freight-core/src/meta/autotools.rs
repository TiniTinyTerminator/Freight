//! Autotools (configure/make) foreign build system integration.
//!
//! Enhancements over a plain `./configure && make && make install`:
//! - `--host=<triple>` injected for cross-compilation when a target is set.
//! - Parallel `make -j{N}` using all available CPUs.
//! - Fast-build: skips the configure step when `config.status` and `Makefile`
//!   already exist and `configure` hasn't changed since the last configure run.
//! - Emscripten support: uses `emconfigure`/`emmake` when the target is wasm/emscripten.
//! - Always passes `--enable-static --disable-shared` for predictable lib output.
use std::path::Path;

use crate::error::FreightError;
use super::run;

pub fn build_autotools(
    dep_dir: &Path,
    build_dir: &Path,
    target: Option<&str>,
) -> Result<(), FreightError> {
    let use_emscripten = target
        .map(|t| t.contains("wasm") || t.contains("emscripten"))
        .unwrap_or(false);

    // Generate configure script if missing.
    if !dep_dir.join("configure").exists() {
        if dep_dir.join("autogen.sh").exists() {
            let sh = if use_emscripten { "emconfigure" } else { "sh" };
            run(sh, &["autogen.sh"], dep_dir, "autogen.sh")?;
        } else {
            run("autoreconf", &["-fi"], dep_dir, "autoreconf")?;
        }
    }

    let install_dir = build_dir.join("install");
    std::fs::create_dir_all(&install_dir)?;

    // Configure step (skipped when already up-to-date).
    if !configure_up_to_date(dep_dir) {
        let configure = dep_dir.join("configure").to_string_lossy().into_owned();
        let prefix = format!("--prefix={}", install_dir.display());

        let mut configure_args: Vec<&str> = vec![&prefix, "--enable-static", "--disable-shared"];
        let host_arg;
        if let Some(triple) = target {
            host_arg = format!("--host={triple}");
            configure_args.push(&host_arg);
        }

        let configure_exe = if use_emscripten { "emconfigure" } else { &configure as &str };
        let actual_args: Vec<&str> = if use_emscripten {
            let mut v = vec![&configure as &str];
            v.extend_from_slice(&configure_args);
            v
        } else {
            configure_args
        };

        run(configure_exe, &actual_args, dep_dir, "configure")?;
    }

    // Build step.
    let jobs = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let jobs_str = jobs.to_string();
    let make_exe = if use_emscripten { "emmake" } else { "make" };

    if use_emscripten {
        run(make_exe, &["make", "-j", &jobs_str], dep_dir, "make")?;
        run(make_exe, &["make", "install"], dep_dir, "make install")?;
    } else {
        run(make_exe, &["-j", &jobs_str], dep_dir, "make")?;
        run(make_exe, &["install"], dep_dir, "make install")?;
    }

    Ok(())
}

/// Returns `true` when configure output is already present and up-to-date,
/// meaning both `config.status` and `Makefile` exist in `dep_dir` and the
/// `configure` script hasn't been modified since `config.status` was written.
fn configure_up_to_date(dep_dir: &Path) -> bool {
    let config_status = dep_dir.join("config.status");
    let makefile = dep_dir.join("Makefile");
    if !config_status.exists() || !makefile.exists() {
        return false;
    }
    let configure = dep_dir.join("configure");
    if !configure.exists() {
        return false;
    }
    let (Ok(c_meta), Ok(cs_meta)) = (
        std::fs::metadata(&configure),
        std::fs::metadata(&config_status),
    ) else {
        return false;
    };
    let (Ok(c_mtime), Ok(cs_mtime)) = (c_meta.modified(), cs_meta.modified()) else {
        return false;
    };
    c_mtime <= cs_mtime
}
