//! vcpkg dependency installation and artifact discovery.
//!
//! Freight installs vcpkg packages into the project's `.deps/vcpkg_installed/`
//! tree so builds do not depend on a mutable global vcpkg installation layout.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::FreightError;

/// Libraries and include directories made available by a vcpkg package.
pub struct VcpkgResolution {
    pub include_dirs: Vec<PathBuf>,
    pub libs: Vec<PathBuf>,
    pub raw_link_flags: Vec<String>,
}

/// Install a vcpkg package into `.deps/vcpkg_installed/`.
///
/// If the package has already been installed by Freight, the install command is
/// skipped. `triplet` defaults to [`default_triplet`] when omitted.
pub fn fetch_vcpkg_dep(
    name: &str,
    package: &str,
    triplet: Option<&str>,
    project_dir: &Path,
) -> Result<PathBuf, FreightError> {
    let triplet = triplet.map(str::to_string).unwrap_or_else(default_triplet);
    let root = installed_root(project_dir);
    let sentinel = root
        .join(".freight")
        .join(format!("{name}.{triplet}.fetched"));

    if sentinel.exists() {
        return Ok(root.join(&triplet));
    }

    use owo_colors::OwoColorize;
    println!(
        "  {} {} from vcpkg ({})",
        "Fetching".dimmed(),
        name,
        package
    );

    std::fs::create_dir_all(root.join(".freight"))?;

    let mut cmd = Command::new(vcpkg_bin());
    cmd.arg("install").arg(package);
    if !package_has_triplet(package) {
        cmd.arg("--triplet").arg(&triplet);
    }
    cmd.arg("--x-install-root").arg(&root);
    cmd.current_dir(project_dir);

    let status = cmd
        .status()
        .map_err(|e| FreightError::CompilerNotFound(format!("vcpkg not found: {e}")))?;
    if !status.success() {
        return Err(FreightError::CompileFailed(
            package.to_string(),
            format!(
                "vcpkg install exited with status {}",
                status.code().unwrap_or(-1)
            ),
        ));
    }

    std::fs::write(&sentinel, package)?;
    Ok(root.join(triplet))
}

/// Ensure a package is installed and return its discovered include dirs and libs.
pub fn resolve_vcpkg_dep(
    name: &str,
    package: &str,
    triplet: Option<&str>,
    project_dir: &Path,
) -> Result<VcpkgResolution, FreightError> {
    let triplet = triplet.map(str::to_string).unwrap_or_else(default_triplet);
    let triplet_dir = fetch_vcpkg_dep(name, package, Some(&triplet), project_dir)?;

    let include = triplet_dir.join("include");
    let include_dirs = if include.is_dir() {
        vec![include]
    } else {
        vec![]
    };

    let lib_dir = triplet_dir.join("lib");
    let mut libs = find_libs(&lib_dir)?;
    let debug_lib_dir = triplet_dir.join("debug").join("lib");
    if libs.is_empty() {
        libs = find_libs(&debug_lib_dir)?;
    }

    let mut raw_link_flags = Vec::new();
    if lib_dir.is_dir() {
        raw_link_flags.push(format!("-L{}", lib_dir.display()));
    }

    Ok(VcpkgResolution {
        include_dirs,
        libs,
        raw_link_flags,
    })
}

/// Return the project-local vcpkg installation root.
pub fn installed_root(project_dir: &Path) -> PathBuf {
    project_dir.join(".deps").join("vcpkg_installed")
}

/// Choose the default vcpkg triplet for the current host.
pub fn default_triplet() -> String {
    if let Ok(t) = std::env::var("VCPKG_DEFAULT_TRIPLET") {
        let trimmed = t.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => "x64-windows".into(),
        ("windows", "x86") => "x86-windows".into(),
        ("windows", "aarch64") => "arm64-windows".into(),
        ("macos", "aarch64") => "arm64-osx".into(),
        ("macos", _) => "x64-osx".into(),
        ("linux", "aarch64") => "arm64-linux".into(),
        ("linux", _) => "x64-linux".into(),
        (_, "aarch64") => "arm64-linux".into(),
        _ => "x64-linux".into(),
    }
}

fn vcpkg_bin() -> String {
    std::env::var("VCPKG").unwrap_or_else(|_| "vcpkg".into())
}

fn package_has_triplet(package: &str) -> bool {
    package
        .rsplit_once(':')
        .is_some_and(|(_, triplet)| !triplet.trim().is_empty())
}

fn find_libs(search_dir: &Path) -> Result<Vec<PathBuf>, FreightError> {
    if !search_dir.is_dir() {
        return Ok(vec![]);
    }

    let mut libs = Vec::new();
    for entry in std::fs::read_dir(search_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if matches!(ext, "a" | "lib" | "so" | "dylib") {
            libs.push(path);
        }
    }
    libs.sort();
    Ok(libs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_package_triplet_suffix() {
        assert!(package_has_triplet("zlib:x64-linux"));
        assert!(package_has_triplet("openssl[core]:x64-windows"));
        assert!(!package_has_triplet("zlib"));
        assert!(!package_has_triplet("zlib:"));
    }

    #[test]
    fn default_triplet_is_not_empty() {
        assert!(!default_triplet().is_empty());
    }
}
