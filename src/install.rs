//! `freight install` and `freight package` — copy build outputs to the system.

use std::fs;
use std::io::{Seek, Write};
use std::path::{Path, PathBuf};

use crate::build::build_project_at;
use crate::error::FreightError;
use crate::event::silent;
use crate::manifest::load_manifest;
use crate::manifest::types::LibType;
use crate::toolchain::GlobalConfig;
use crate::vendor::parse_triple;

// ── Public types ──────────────────────────────────────────────────────────────

pub struct InstallOptions {
    /// Installation prefix, e.g. `/usr/local`. The subdirectories `bin/`,
    /// `lib/`, `include/` are created beneath it.
    pub prefix: PathBuf,
    /// Optional staging root prepended before `prefix` (for package tools).
    /// Actual on-disk path = `destdir / prefix.strip_leading_slash()`.
    pub destdir: Option<PathBuf>,
    /// Build in release mode before installing.
    pub release: bool,
    /// Skip the build step — install whatever is already in `target/`.
    pub no_build: bool,
    /// Cross-compilation target triple (e.g. `aarch64-linux-gnu`).
    /// When set, overrides `[compiler] target` in the manifest and drives
    /// platform-specific install decisions (shared lib naming, DLL placement).
    pub target: Option<String>,
}

impl Default for InstallOptions {
    fn default() -> Self {
        Self {
            prefix: default_prefix(),
            destdir: None,
            release: true,
            no_build: false,
            target: None,
        }
    }
}

pub enum InstalledKind {
    Binary,
    StaticLib,
    SharedLib,
    Header,
    Symlink,
}

impl InstalledKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Binary => "binary",
            Self::StaticLib => "static-lib",
            Self::SharedLib => "shared-lib",
            Self::Header => "header",
            Self::Symlink => "symlink",
        }
    }
}

pub struct InstalledItem {
    pub dst: PathBuf,
    pub kind: InstalledKind,
}

pub struct InstallResult {
    pub items: Vec<InstalledItem>,
}

// ── Public entry points ───────────────────────────────────────────────────────

/// Build (unless `opts.no_build`) and install all outputs to `opts.prefix`.
pub fn install_project(
    project_dir: &Path,
    opts: &InstallOptions,
) -> Result<InstallResult, FreightError> {
    let manifest = load_manifest(project_dir)?;
    let profile = if opts.release { "release" } else { "dev" };

    if !opts.no_build {
        build_project_at(
            project_dir,
            profile,
            &[],
            true,
            opts.target.as_deref(),
            &[],
            &silent(),
        )?;
    }

    // Derive target OS/arch: prefer the explicit override, then ~/.freight/config.toml, then host.
    let global_target = GlobalConfig::load().target;
    let target_str = opts.target.as_deref().or_else(|| global_target.as_deref());
    let (target_arch, target_os) = target_str.map(parse_triple).unwrap_or_else(|| {
        (
            std::env::consts::ARCH.to_string(),
            std::env::consts::OS.to_string(),
        )
    });

    let root = install_root(&opts.prefix, opts.destdir.as_deref());
    let bin_dir = root.join("bin");
    let lib_dir = root.join("lib");
    let mut items: Vec<InstalledItem> = Vec::new();

    // ── Binaries ──────────────────────────────────────────────────────────────
    for bin in &manifest.bins {
        let bin_file = executable_name(&bin.name, &target_os);
        let src = project_dir.join("target").join(profile).join(&bin_file);
        if !src.exists() {
            return Err(FreightError::InstallFailed(format!(
                "binary '{}' not found in target/{profile}/ — run `freight build` first",
                bin.name
            )));
        }
        fs::create_dir_all(&bin_dir)?;
        let dst = bin_dir.join(&bin_file);
        copy_file(&src, &dst)?;
        set_mode(&dst, 0o755)?;
        items.push(InstalledItem {
            dst,
            kind: InstalledKind::Binary,
        });
    }

    // ── Library ───────────────────────────────────────────────────────────────
    if let Some(lib) = &manifest.lib {
        fs::create_dir_all(&lib_dir)?;

        // Prebuilt libs (link is set) have no built artifact to install.
        if lib.link.is_none() {
            match lib.lib_type {
                LibType::Static => {
                    let fname = format!("lib{}.a", manifest.package.name);
                    let src = project_dir.join("target").join(profile).join(&fname);
                    if src.exists() {
                        let dst = lib_dir.join(&fname);
                        copy_file(&src, &dst)?;
                        set_mode(&dst, 0o644)?;
                        items.push(InstalledItem {
                            dst,
                            kind: InstalledKind::StaticLib,
                        });
                    }
                }
                LibType::Shared => {
                    install_shared_lib(
                        project_dir,
                        profile,
                        &manifest.package.name,
                        &manifest.package.version,
                        &lib_dir,
                        &opts.prefix,
                        &target_os,
                        &mut items,
                    )?;
                }
                LibType::Header => {}
            }
        }

        // ── Public headers ────────────────────────────────────────────────────
        if !lib.hdrs.is_empty() {
            let inc_dst = root.join("include").join(&manifest.package.name);
            std::fs::create_dir_all(&inc_dst)?;
            for hdr in &lib.hdrs {
                let src = project_dir.join(hdr);
                if src.is_file() {
                    let dst = inc_dst.join(src.file_name().unwrap());
                    std::fs::copy(&src, &dst)?;
                    items.push(InstalledItem {
                        dst,
                        kind: InstalledKind::Header,
                    });
                }
            }
        }
    }

    // On Linux targets: refresh the dynamic linker cache when installing shared
    // libs to a real system path (not a destdir-staged install).
    if target_os == "linux"
        && items
            .iter()
            .any(|i| matches!(i.kind, InstalledKind::SharedLib))
        && opts.destdir.is_none()
    {
        run_ldconfig(&lib_dir);
    }

    // suppress unused-variable warning when compiled on non-Linux hosts
    let _ = target_arch;

    Ok(InstallResult { items })
}

/// Build in release mode, install to a staging dir, and produce a
/// `{name}-{version}-{arch}-{os}.tar.gz` (or `.zip` for Windows targets) in `target/package/`.
///
/// `target` is an optional cross-compilation triple (e.g. `aarch64-linux-gnu`).
/// When provided it overrides the manifest's `[compiler] target` and is used to
/// derive the arch/os components of the archive filename.
pub fn package_project(
    project_dir: &Path,
    release: bool,
    target: Option<&str>,
) -> Result<PathBuf, FreightError> {
    let manifest = load_manifest(project_dir)?;
    let profile = if release { "release" } else { "dev" };

    build_project_at(project_dir, profile, &[], true, target, &[], &silent())?;

    let global_target = GlobalConfig::load().target;
    let (pkg_arch, pkg_os) = target
        .or_else(|| global_target.as_deref())
        .map(parse_triple)
        .unwrap_or_else(|| {
            (
                std::env::consts::ARCH.to_string(),
                std::env::consts::OS.to_string(),
            )
        });

    let stem = format!(
        "{}-{}-{}-{}",
        manifest.package.name, manifest.package.version, pkg_arch, pkg_os,
    );

    let pkg_dir = project_dir.join("target").join("package");
    fs::create_dir_all(&pkg_dir)?;

    let staging = pkg_dir.join(&stem);
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }

    // Install directly into the staging dir (prefix = staging, no destdir).
    install_project(
        project_dir,
        &InstallOptions {
            prefix: staging.clone(),
            destdir: None,
            release,
            no_build: true,
            target: target.map(str::to_string),
        },
    )?;

    let archive = if pkg_os == "windows" {
        let archive = pkg_dir.join(format!("{stem}.zip"));
        create_zip_archive(&pkg_dir, &stem, &archive)?;
        archive
    } else {
        let archive = pkg_dir.join(format!("{stem}.tar.gz"));
        create_tarball(&pkg_dir, &stem, &archive)?;
        archive
    };
    fs::remove_dir_all(&staging)?;

    Ok(archive)
}

// ── Platform-specific shared lib install ─────────────────────────────────────

fn install_shared_lib(
    project_dir: &Path,
    profile: &str,
    name: &str,
    version: &str,
    lib_dir: &Path,
    prefix: &Path,
    target_os: &str,
    items: &mut Vec<InstalledItem>,
) -> Result<(), FreightError> {
    match target_os {
        "linux" => {
            let src = project_dir
                .join("target")
                .join(profile)
                .join(format!("lib{name}.so"));
            if !src.exists() {
                return Ok(());
            }

            let major = version.split('.').next().unwrap_or("0");
            let versioned = format!("lib{name}.so.{version}");
            let soname = format!("lib{name}.so.{major}");
            let unversioned = format!("lib{name}.so");

            // Install the full versioned file.
            let dst = lib_dir.join(&versioned);
            copy_file(&src, &dst)?;
            set_mode(&dst, 0o755)?;
            items.push(InstalledItem {
                dst,
                kind: InstalledKind::SharedLib,
            });

            // libfoo.so.1   → libfoo.so.1.2.3   (SONAME link)
            make_symlink(lib_dir, &soname, &versioned)?;
            items.push(InstalledItem {
                dst: lib_dir.join(&soname),
                kind: InstalledKind::Symlink,
            });

            // libfoo.so     → libfoo.so.1         (linker-time link)
            make_symlink(lib_dir, &unversioned, &soname)?;
            items.push(InstalledItem {
                dst: lib_dir.join(&unversioned),
                kind: InstalledKind::Symlink,
            });
        }

        "macos" => {
            let src = project_dir
                .join("target")
                .join(profile)
                .join(format!("lib{name}.dylib"));
            if !src.exists() {
                return Ok(());
            }

            let fname = format!("lib{name}.dylib");
            let dst = lib_dir.join(&fname);
            copy_file(&src, &dst)?;
            set_mode(&dst, 0o755)?;

            // Update the embedded install name so consumers can find the lib
            // at its installed location without extra DYLD_LIBRARY_PATH magic.
            let install_name = prefix.join("lib").join(&fname);
            let _ = std::process::Command::new("install_name_tool")
                .args([
                    "-id",
                    &install_name.to_string_lossy(),
                    &dst.to_string_lossy().into_owned(),
                ])
                .status();

            items.push(InstalledItem {
                dst,
                kind: InstalledKind::SharedLib,
            });
        }

        _ => {
            // Windows — DLLs live in bin/, not lib/.
            let src = project_dir
                .join("target")
                .join(profile)
                .join(format!("{name}.dll"));
            if !src.exists() {
                return Ok(());
            }

            let bin_dir = lib_dir.parent().unwrap_or(lib_dir).join("bin");
            fs::create_dir_all(&bin_dir)?;

            let dst = bin_dir.join(format!("{name}.dll"));
            copy_file(&src, &dst)?;
            items.push(InstalledItem {
                dst,
                kind: InstalledKind::SharedLib,
            });

            // Import lib alongside the static libs if present.
            let imp_src = project_dir
                .join("target")
                .join(profile)
                .join(format!("{name}.lib"));
            if imp_src.exists() {
                let imp_dst = lib_dir.join(format!("{name}.lib"));
                copy_file(&imp_src, &imp_dst)?;
                items.push(InstalledItem {
                    dst: imp_dst,
                    kind: InstalledKind::StaticLib,
                });
            }
        }
    }
    Ok(())
}

// ── File / dir utilities ──────────────────────────────────────────────────────

/// Compute the effective on-disk install root from `prefix` and `destdir`.
fn install_root(prefix: &Path, destdir: Option<&Path>) -> PathBuf {
    match destdir {
        None => prefix.to_path_buf(),
        Some(dd) => {
            // Strip the leading `/` so joining doesn't discard destdir.
            let rel = prefix.strip_prefix("/").unwrap_or(prefix);
            dd.join(rel)
        }
    }
}

fn copy_file(src: &Path, dst: &Path) -> Result<(), FreightError> {
    if let Some(p) = dst.parent() {
        fs::create_dir_all(p)?;
    }
    fs::copy(src, dst).map(|_| ())?;
    Ok(())
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<(), FreightError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> Result<(), FreightError> {
    Ok(())
}

#[cfg(unix)]
fn make_symlink(dir: &Path, link_name: &str, target: &str) -> Result<(), FreightError> {
    let link = dir.join(link_name);
    // Remove stale link so we can re-link cleanly.
    if link.symlink_metadata().is_ok() {
        fs::remove_file(&link)?;
    }
    std::os::unix::fs::symlink(target, &link)?;
    Ok(())
}

#[cfg(not(unix))]
fn make_symlink(_dir: &Path, _link: &str, _target: &str) -> Result<(), FreightError> {
    Ok(()) // Symlinks on Windows require elevated rights; skip silently.
}

fn executable_name(name: &str, target_os: &str) -> String {
    if target_os == "windows" && !name.ends_with(".exe") {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

fn create_zip_archive(parent: &Path, stem: &str, archive: &Path) -> Result<(), FreightError> {
    let root = parent.join(stem);
    let mut files = Vec::new();
    collect_zip_files(&root, &root, stem, &mut files)?;
    files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut out = fs::File::create(archive)?;
    let mut central = Vec::new();

    for (name, path) in files {
        let data = fs::read(&path)?;
        let crc = crc32(&data);
        let offset = out.stream_position()? as u32;
        let name_bytes = name.as_bytes();

        write_u32(&mut out, 0x0403_4b50)?;
        write_u16(&mut out, 20)?;
        write_u16(&mut out, 0)?;
        write_u16(&mut out, 0)?;
        write_u16(&mut out, 0)?;
        write_u16(&mut out, 0)?;
        write_u32(&mut out, crc)?;
        write_u32(&mut out, data.len() as u32)?;
        write_u32(&mut out, data.len() as u32)?;
        write_u16(&mut out, name_bytes.len() as u16)?;
        write_u16(&mut out, 0)?;
        out.write_all(name_bytes)?;
        out.write_all(&data)?;

        central.push((name, crc, data.len() as u32, offset));
    }

    let central_start = out.stream_position()? as u32;
    for (name, crc, len, offset) in &central {
        let name_bytes = name.as_bytes();
        write_u32(&mut out, 0x0201_4b50)?;
        write_u16(&mut out, 20)?;
        write_u16(&mut out, 20)?;
        write_u16(&mut out, 0)?;
        write_u16(&mut out, 0)?;
        write_u16(&mut out, 0)?;
        write_u16(&mut out, 0)?;
        write_u32(&mut out, *crc)?;
        write_u32(&mut out, *len)?;
        write_u32(&mut out, *len)?;
        write_u16(&mut out, name_bytes.len() as u16)?;
        write_u16(&mut out, 0)?;
        write_u16(&mut out, 0)?;
        write_u16(&mut out, 0)?;
        write_u16(&mut out, 0)?;
        write_u32(&mut out, 0)?;
        write_u32(&mut out, *offset)?;
        out.write_all(name_bytes)?;
    }
    let central_end = out.stream_position()? as u32;
    let central_size = central_end - central_start;

    write_u32(&mut out, 0x0605_4b50)?;
    write_u16(&mut out, 0)?;
    write_u16(&mut out, 0)?;
    write_u16(&mut out, central.len() as u16)?;
    write_u16(&mut out, central.len() as u16)?;
    write_u32(&mut out, central_size)?;
    write_u32(&mut out, central_start)?;
    write_u16(&mut out, 0)?;

    Ok(())
}

fn collect_zip_files(
    root: &Path,
    dir: &Path,
    stem: &str,
    files: &mut Vec<(String, PathBuf)>,
) -> Result<(), FreightError> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_zip_files(root, &path, stem, files)?;
        } else if path.is_file() {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            let rel = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            files.push((format!("{stem}/{rel}"), path));
        }
    }
    Ok(())
}

fn write_u16<W: Write>(w: &mut W, n: u16) -> Result<(), FreightError> {
    w.write_all(&n.to_le_bytes()).map_err(FreightError::Io)
}

fn write_u32<W: Write>(w: &mut W, n: u32) -> Result<(), FreightError> {
    w.write_all(&n.to_le_bytes()).map_err(FreightError::Io)
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in bytes {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn create_tarball(parent: &Path, stem: &str, archive: &Path) -> Result<(), FreightError> {
    // `tar` is available on Linux, macOS, and Windows 10+.
    let status = std::process::Command::new("tar")
        .args([
            "-czf",
            &archive.to_string_lossy(),
            "-C",
            &parent.to_string_lossy(),
            stem,
        ])
        .status()
        .map_err(|e| FreightError::InstallFailed(format!("tar not found: {e}")))?;

    if !status.success() {
        return Err(FreightError::InstallFailed(
            "tar exited with non-zero status".into(),
        ));
    }
    Ok(())
}

// ── Installer (self-contained bundle) ─────────────────────────────────────────

/// Like [`package_project`], but also collects the binary's transitive shared-
/// library dependencies and bundles them into `lib/` so the archive runs on a
/// machine that doesn't have those libraries installed.
///
/// Layout inside the archive:
/// ```text
/// myapp-1.0-x86_64-linux-installer/
/// ├── bin/myapp          ← the built binary (or .exe on Windows)
/// ├── lib/               ← bundled .so / .dylib / .dll dependencies
/// │   ├── libfoo.so.1
/// │   └── …
/// └── myapp              ← launcher script (Linux/macOS) that sets LD_LIBRARY_PATH
/// ```
///
/// Windows targets: DLLs are copied into `bin/` next to the executable (the
/// Windows DLL search order already checks the exe directory first), so no
/// wrapper script is needed.
pub fn installer_project(
    project_dir: &Path,
    release: bool,
    target: Option<&str>,
) -> Result<PathBuf, FreightError> {
    let manifest = load_manifest(project_dir)?;
    let profile = if release { "release" } else { "dev" };

    build_project_at(project_dir, profile, &[], true, target, &[], &silent())?;

    let global_target = GlobalConfig::load().target;
    let (pkg_arch, pkg_os) = target
        .or_else(|| global_target.as_deref())
        .map(parse_triple)
        .unwrap_or_else(|| {
            (
                std::env::consts::ARCH.to_string(),
                std::env::consts::OS.to_string(),
            )
        });

    let stem = format!(
        "{}-{}-{}-{}-installer",
        manifest.package.name, manifest.package.version, pkg_arch, pkg_os,
    );

    let pkg_dir = project_dir.join("target").join("package");
    fs::create_dir_all(&pkg_dir)?;

    let staging = pkg_dir.join(&stem);
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }

    // 1. Install built outputs into staging (bin/, lib/, include/).
    install_project(
        project_dir,
        &InstallOptions {
            prefix: staging.clone(),
            destdir: None,
            release,
            no_build: true,
            target: target.map(str::to_string),
        },
    )?;

    // 2. Collect transitive shared-lib deps for every installed binary and
    //    copy them into staging/lib/.
    let bundled_lib_dir = staging.join("lib");
    fs::create_dir_all(&bundled_lib_dir)?;

    let bin_dir = staging.join("bin");
    if bin_dir.is_dir() {
        for entry in fs::read_dir(&bin_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let deps = collect_shared_deps(&path, &pkg_os)?;
                for dep in deps {
                    let fname = dep.file_name().unwrap_or_default();
                    let dst = if pkg_os == "windows" {
                        // DLLs go beside the exe so Windows search order finds them.
                        bin_dir.join(fname)
                    } else {
                        bundled_lib_dir.join(fname)
                    };
                    if !dst.exists() {
                        fs::copy(&dep, &dst).map_err(|e| {
                            FreightError::InstallFailed(format!(
                                "bundling {}: {e}",
                                dep.display()
                            ))
                        })?;
                    }
                }

                // 3. Write a launcher script (Linux/macOS) so the binary finds
                //    its bundled libs without requiring LD_LIBRARY_PATH from the caller.
                if pkg_os != "windows" {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        write_launcher_script(&staging, name, &pkg_os)?;
                    }
                }

                // 4. macOS: rewrite dylib install names to @executable_path/../lib/.
                if pkg_os == "macos" {
                    rewrite_macos_rpaths(&path, &bundled_lib_dir)?;
                }
            }
        }
    }

    // 5. Archive and clean up staging dir.
    let archive = if pkg_os == "windows" {
        let archive = pkg_dir.join(format!("{stem}.zip"));
        create_zip_archive(&pkg_dir, &stem, &archive)?;
        archive
    } else {
        let archive = pkg_dir.join(format!("{stem}.tar.gz"));
        create_tarball(&pkg_dir, &stem, &archive)?;
        archive
    };
    fs::remove_dir_all(&staging)?;

    Ok(archive)
}

// ── Shared-lib dependency collection ─────────────────────────────────────────

/// Paths to system-provided libraries that we never bundle.
/// Bundling libc, libm, or ld-linux would break on glibc version mismatches.
fn is_system_lib(path: &Path, target_os: &str) -> bool {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();

    match target_os {
        "linux" => {
            // glibc and kernel-interface libs must come from the host.
            let skip = [
                "libc.so", "libm.so", "libdl.so", "libpthread.so", "librt.so",
                "libresolv.so", "libutil.so", "libnss_", "libnsl.so",
                "libgcc_s.so",  // ABI-compatible on all modern distros
                "ld-linux", "linux-vdso", "linux-gate",
            ];
            skip.iter().any(|s| name.starts_with(s))
        }
        "macos" => {
            // Apple system frameworks and /usr/lib dylibs.
            path.starts_with("/usr/lib")
                || path.starts_with("/System/")
                || path.starts_with("/Library/Apple/")
        }
        "windows" => {
            // Windows system DLLs (system32 and friends).
            let skip = [
                "kernel32.dll", "user32.dll", "gdi32.dll", "ole32.dll",
                "oleaut32.dll", "ntdll.dll", "advapi32.dll", "shell32.dll",
                "shlwapi.dll", "ws2_32.dll", "msvcp", "vcruntime", "ucrtbase",
                "api-ms-win", "ext-ms-win",
            ];
            skip.iter().any(|s| name.starts_with(s))
        }
        _ => false,
    }
}

/// Run the appropriate tool to collect the binary's shared-lib dependencies.
///
/// Returns absolute paths to library files that should be bundled.
fn collect_shared_deps(binary: &Path, target_os: &str) -> Result<Vec<PathBuf>, FreightError> {
    match target_os {
        "linux" => collect_shared_deps_ldd(binary, target_os),
        "macos" => collect_shared_deps_otool(binary, target_os),
        "windows" => collect_shared_deps_dumpbin(binary, target_os),
        other => {
            eprintln!("warning: shared-lib collection not supported on {other}; bundling no deps");
            Ok(vec![])
        }
    }
}

fn collect_shared_deps_ldd(binary: &Path, target_os: &str) -> Result<Vec<PathBuf>, FreightError> {
    let out = std::process::Command::new("ldd")
        .arg(binary)
        .output()
        .map_err(|e| FreightError::InstallFailed(format!("ldd not found: {e}")))?;

    if !out.status.success() {
        // Static binaries or non-ELF files cause ldd to exit non-zero; treat as no deps.
        return Ok(vec![]);
    }

    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut deps = Vec::new();

    // ldd output lines are one of:
    //   linux-vdso.so.1 (0x…)
    //   libfoo.so.1 => /lib/x86_64-linux-gnu/libfoo.so.1 (0x…)
    //   /lib64/ld-linux-x86-64.so.2 (0x…)
    for line in stdout.lines() {
        let line = line.trim();
        let path = if let Some(idx) = line.find("=>") {
            // "name => /path (0x…)"
            let after = line[idx + 2..].trim();
            after.split_whitespace().next().filter(|&p| p != "not")
        } else {
            // bare "/lib64/ld-linux…" line
            let p = match line.split_whitespace().next() {
                Some(p) => p,
                None => continue,
            };
            if p.starts_with('/') { Some(p) } else { None }
        };

        if let Some(p) = path {
            let pb = PathBuf::from(p);
            if pb.exists() && !is_system_lib(&pb, target_os) {
                deps.push(pb);
            }
        }
    }
    Ok(deps)
}

fn collect_shared_deps_otool(binary: &Path, target_os: &str) -> Result<Vec<PathBuf>, FreightError> {
    let out = std::process::Command::new("otool")
        .args(["-L", &binary.to_string_lossy()])
        .output()
        .map_err(|e| FreightError::InstallFailed(format!("otool not found: {e}")))?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut deps = Vec::new();

    // otool -L output:
    //   binary:
    //     /usr/lib/libSystem.B.dylib (compatibility version …)
    //     /usr/local/lib/libfoo.1.dylib (…)
    for line in stdout.lines().skip(1) {
        let line = line.trim();
        if let Some(path_str) = line.split(' ').next() {
            let pb = PathBuf::from(path_str);
            if pb.is_absolute() && pb.exists() && !is_system_lib(&pb, target_os) {
                deps.push(pb);
            }
        }
    }
    Ok(deps)
}

fn collect_shared_deps_dumpbin(binary: &Path, target_os: &str) -> Result<Vec<PathBuf>, FreightError> {
    // dumpbin is part of MSVC; may not be present in all CI environments.
    let out = match std::process::Command::new("dumpbin")
        .args(["/DEPENDENTS", &binary.to_string_lossy()])
        .output()
    {
        Ok(o) => o,
        Err(_) => {
            eprintln!("warning: dumpbin not found; falling back to ldd for DLL detection");
            return collect_shared_deps_ldd(binary, target_os);
        }
    };

    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut deps = Vec::new();

    // Parse the "Image has the following dependencies:" section.
    let mut in_section = false;
    for line in stdout.lines() {
        let line = line.trim();
        if line.contains("has the following dependencies") {
            in_section = true;
            continue;
        }
        if in_section {
            if line.is_empty() {
                break;
            }
            if line.ends_with(".dll") || line.ends_with(".DLL") {
                // Resolve the DLL via PATH (same search order Windows uses).
                if let Some(resolved) = resolve_dll_on_path(line) {
                    if !is_system_lib(&resolved, target_os) {
                        deps.push(resolved);
                    }
                }
            }
        }
    }
    Ok(deps)
}

fn resolve_dll_on_path(dll_name: &str) -> Option<PathBuf> {
    let path_var = std::env::var("PATH").ok()?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(dll_name);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

// ── Launcher script ───────────────────────────────────────────────────────────

fn write_launcher_script(staging: &Path, bin_name: &str, target_os: &str) -> Result<(), FreightError> {
    // Strip .exe suffix for the script name (shouldn't happen for Linux/macOS but be safe).
    let script_name = bin_name.trim_end_matches(".exe");
    let script_path = staging.join(script_name);

    let lib_var = if target_os == "macos" { "DYLD_LIBRARY_PATH" } else { "LD_LIBRARY_PATH" };

    let content = format!(
        "#!/bin/sh\n\
         DIR=\"$(cd \"$(dirname \"$0\")\" && pwd)\"\n\
         export {lib_var}=\"$DIR/lib${{{}:+:${{{}}}}}\"\n\
         exec \"$DIR/bin/{bin_name}\" \"$@\"\n",
        lib_var, lib_var,
    );

    fs::write(&script_path, content)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))?;
    }

    Ok(())
}

// ── macOS rpath rewriting ─────────────────────────────────────────────────────

fn rewrite_macos_rpaths(binary: &Path, bundled_lib_dir: &Path) -> Result<(), FreightError> {
    if !bundled_lib_dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(bundled_lib_dir)? {
        let entry = entry?;
        let lib = entry.path();
        if lib.extension().map_or(false, |e| e == "dylib") {
            let old = lib.to_string_lossy();
            let new = format!(
                "@executable_path/../lib/{}",
                lib.file_name().unwrap_or_default().to_string_lossy()
            );
            let _ = std::process::Command::new("install_name_tool")
                .args(["-change", &old, &new, &binary.to_string_lossy()])
                .status();
        }
    }
    Ok(())
}

fn run_ldconfig(lib_dir: &Path) {
    // Only meaningful on a Linux host; no-op when cross-compiling from another OS.
    if cfg!(target_os = "linux") {
        // Non-fatal — fails silently when not running as root.
        let _ = std::process::Command::new("ldconfig").arg(lib_dir).status();
    }
}

fn default_prefix() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(r"C:\Program Files")
    } else {
        PathBuf::from("/usr/local")
    }
}
