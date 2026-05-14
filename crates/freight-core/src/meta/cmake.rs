//! CMake foreign build system integration.
//!
//! Enhancements over a plain `cmake -S -B`:
//! - Prefers the Ninja generator when `ninja` is on `$PATH`.
//! - Injects `CMAKE_SYSTEM_NAME` / `CMAKE_SYSTEM_PROCESSOR` for cross-builds.
//! - Uses `cmake --build --parallel` when CMake ≥ 3.12 is detected.
//! - Runs `cmake --install` so headers and archives land in a predictable prefix.
use std::path::Path;
use std::process::Command;

use crate::error::FreightError;
use super::run;

pub fn build_cmake(
    dep_dir: &Path,
    build_dir: &Path,
    profile: &str,
    extra_args: &[String],
    target: Option<&str>,
) -> Result<(), FreightError> {
    let build_type = if profile == "release" { "Release" } else { "Debug" };
    let install_prefix = build_dir.join("install");
    std::fs::create_dir_all(&install_prefix)?;

    let src    = dep_dir.to_string_lossy().into_owned();
    let bdir   = build_dir.to_string_lossy().into_owned();
    let btype  = format!("-DCMAKE_BUILD_TYPE={build_type}");
    let prefix = format!("-DCMAKE_INSTALL_PREFIX={}", install_prefix.display());

    let mut configure_args: Vec<String> = vec![
        "-S".into(), src,
        "-B".into(), bdir.clone(),
        btype, prefix,
    ];

    // Generator selection: prefer Ninja when available.
    if let Some(gen) = select_generator() {
        configure_args.push("-G".into());
        configure_args.push(gen);
    }

    // Cross-compilation: inject system name and processor.
    if let Some(triple) = target {
        if let Some((system_name, processor)) = cmake_system_from_triple(triple) {
            configure_args.push(format!("-DCMAKE_SYSTEM_NAME={system_name}"));
            configure_args.push(format!("-DCMAKE_SYSTEM_PROCESSOR={processor}"));
        }
    }

    for a in extra_args {
        configure_args.push(a.clone());
    }

    let args_refs: Vec<&str> = configure_args.iter().map(String::as_str).collect();
    run("cmake", &args_refs, dep_dir, "cmake configure")?;

    // Build step, with --parallel on CMake ≥ 3.12.
    let jobs = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    let jobs_str = jobs.to_string();
    let cmake_ver = cmake_version();
    let parallel_supported = cmake_ver.map(|(maj, min)| (maj, min) >= (3, 12)).unwrap_or(false);

    if parallel_supported {
        run("cmake", &["--build", &bdir, "--parallel", &jobs_str], dep_dir, "cmake build")?;
    } else {
        run("cmake", &["--build", &bdir], dep_dir, "cmake build")?;
    }

    // Install step.
    let install_prefix_str = install_prefix.to_string_lossy().into_owned();
    // cmake --install is available since CMake 3.15; fall back gracefully.
    let install_supported = cmake_ver.map(|(maj, min)| (maj, min) >= (3, 15)).unwrap_or(false);
    if install_supported {
        run(
            "cmake",
            &["--install", &bdir, "--prefix", &install_prefix_str],
            dep_dir,
            "cmake install",
        )?;
    } else {
        // Older CMake: use the install target via the native build system.
        run("cmake", &["--build", &bdir, "--target", "install"], dep_dir, "cmake install")?;
    }

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Detect the installed CMake version as (major, minor).
fn cmake_version() -> Option<(u32, u32)> {
    let out = Command::new("cmake").arg("--version").output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().next()?;
    let ver_str = line.strip_prefix("cmake version ")?.trim();
    let mut parts = ver_str.split('.');
    let major: u32 = parts.next()?.parse().ok()?;
    let minor: u32 = parts.next()?.parse().ok()?;
    Some((major, minor))
}

/// Pick a CMake generator. Returns `None` to let CMake choose its default.
fn select_generator() -> Option<String> {
    // Prefer Ninja for faster incremental builds.
    if Command::new("ninja")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return Some("Ninja".into());
    }
    None
}

/// Map a compiler target triple to (CMAKE_SYSTEM_NAME, CMAKE_SYSTEM_PROCESSOR).
fn cmake_system_from_triple(triple: &str) -> Option<(&'static str, &'static str)> {
    if triple.contains("linux") {
        let proc = if triple.starts_with("aarch64") {
            "aarch64"
        } else if triple.starts_with("arm") {
            "arm"
        } else if triple.starts_with("riscv64") {
            "riscv64"
        } else if triple.starts_with("i686") || triple.starts_with("i386") {
            "x86"
        } else {
            "x86_64"
        };
        return Some(("Linux", proc));
    }
    if triple.contains("darwin") || triple.contains("apple") {
        let proc = if triple.starts_with("aarch64") { "arm64" } else { "x86_64" };
        return Some(("Darwin", proc));
    }
    if triple.contains("windows") {
        let proc = if triple.starts_with("aarch64") {
            "ARM64"
        } else if triple.starts_with("i686") || triple.starts_with("i386") {
            "X86"
        } else {
            "AMD64"
        };
        return Some(("Windows", proc));
    }
    if triple.contains("wasm") || triple.contains("emscripten") {
        return Some(("Emscripten", "asm.js"));
    }
    if triple.contains("android") {
        let proc = if triple.starts_with("aarch64") {
            "aarch64"
        } else if triple.starts_with("arm") {
            "arm"
        } else {
            "x86_64"
        };
        return Some(("Android", proc));
    }
    None
}
