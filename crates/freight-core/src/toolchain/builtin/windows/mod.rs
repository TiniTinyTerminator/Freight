use crate::toolchain::template::{CompilerTemplate, ToolchainDef, LinkingParams};

// ── MSVC / clang-cl ───────────────────────────────────────────────────────────

pub fn msvc() -> CompilerTemplate {
    let mut d = ToolchainDef {
        name: "msvc".into(),
        binary: "cl.exe".into(),
        family: "".into(),
        version_arg: "".into(),
        version_regex: r"Version (\d+\.\d+\.\d+\.\d+)".into(),
        extensions: vec![".cpp".into(), ".cc".into(), ".cxx".into(), ".c++".into(), ".c".into()],
        sanitizer_options: vec!["address".into()],
        supported_os: vec!["windows".into()],
        flags_debug: "/Zi /FS".into(),
        flags_lto: "/GL".into(),
        flags_lto_link: "/LTCG".into(),
        sanitize: "/fsanitize={values}".into(),
        ..Default::default()
    };
    d.flags_opt.insert("0".into(), "/Od".into());
    d.flags_opt.insert("1".into(), "/O1".into());
    d.flags_opt.insert("2".into(), "/O2".into());
    d.flags_opt.insert("3".into(), "/Ox".into());
    d.flags_opt.insert("s".into(), "/O1 /Os".into());
    d.flags_opt.insert("z".into(), "/O1 /Os".into());
    d.flags_warnings.insert("none".into(), "/W0".into());
    d.flags_warnings.insert("default".into(), "/W3".into());
    d.flags_warnings.insert("all".into(), "/W4".into());
    d.flags_warnings.insert("error".into(), "/W4 /WX".into());
    d.standards.insert("c++17".into(), "/std:c++17".into());
    d.standards.insert("c++20".into(), "/std:c++20".into());
    d.standards.insert("c++23".into(), "/std:c++latest".into());
    d.standards.insert("c17".into(), "/std:c17".into());
    d.standards.insert("c11".into(), "/std:c11".into());
    d.structure.insert("include_dir".into(), "/I{path}".into());
    d.structure.insert("define".into(), "/D{name}".into());
    d.structure.insert("define_value".into(), "/D{name}={value}".into());
    d.structure.insert("output_obj".into(), "/Fo{path}".into());
    d.structure.insert("output_bin".into(), "/Fe{path}".into());
    d.structure.insert("compile_only".into(), "/c".into());
    d.structure.insert("dep_file".into(), "/showIncludes".into());
    d.structure.insert("dep_file_mode".into(), "stdout".into());
    d.structure.insert("system_lib".into(), "{name}.lib".into());
    d.toolset.insert("cc".into(), "cl.exe".into());
    d.toolset.insert("cxx".into(), "cl.exe".into());
    d.toolset.insert("ld".into(), "link.exe".into());
    d.toolset.insert("ar".into(), "lib.exe".into());
    d.toolset.insert("strip".into(), "".into());
    d.linking.push(("c".into(), LinkingParams {
        abi: "c".into(),
        compatible: vec![],
        compile_binary: Some("cl.exe".into()),
        linker: "".into(),
        extensions: vec![".c".into()],
    }));
    d.linking.push(("cpp".into(), LinkingParams {
        abi: "c++".into(),
        compatible: vec!["c".into()],
        linker: "".into(),
        extensions: vec![".cpp".into(), ".cc".into(), ".cxx".into(), ".c++".into()],
        compile_binary: None,
    }));
    CompilerTemplate::from_def(d).unwrap()
}

/// `clang-cl` — Clang with MSVC-compatible flags. Uses the same flag scheme as
/// `cl.exe` but is detected by `clang-cl` on PATH.
pub fn clang_cl() -> CompilerTemplate {
    let mut d = ToolchainDef {
        name: "clang-cl".into(),
        binary: "clang-cl".into(),
        family: "llvm".into(),
        version_arg: "--version".into(),
        version_regex: r"\b(\d+\.\d+\.\d+)\b".into(),
        extensions: vec![".cpp".into(), ".cc".into(), ".cxx".into(), ".c++".into(), ".c".into()],
        sanitizer_options: vec!["address".into(), "undefined".into()],
        supported_os: vec!["windows".into()],
        flags_debug: "/Zi /FS".into(),
        flags_lto: "/GL".into(),
        flags_lto_link: "/LTCG".into(),
        sanitize: "/fsanitize={values}".into(),
        ..Default::default()
    };
    d.flags_opt.insert("0".into(), "/Od".into());
    d.flags_opt.insert("1".into(), "/O1".into());
    d.flags_opt.insert("2".into(), "/O2".into());
    d.flags_opt.insert("3".into(), "/Ox".into());
    d.flags_opt.insert("s".into(), "/O1 /Os".into());
    d.flags_opt.insert("z".into(), "/O1 /Os".into());
    d.flags_warnings.insert("none".into(), "/W0".into());
    d.flags_warnings.insert("default".into(), "/W3".into());
    d.flags_warnings.insert("all".into(), "/W4 -Wextra".into());
    d.flags_warnings.insert("error".into(), "/W4 -Wextra /WX".into());
    d.standards.insert("c++17".into(), "/std:c++17".into());
    d.standards.insert("c++20".into(), "/std:c++20".into());
    d.standards.insert("c++23".into(), "/std:c++latest".into());
    d.standards.insert("c17".into(), "/std:c17".into());
    d.standards.insert("c11".into(), "/std:c11".into());
    d.structure.insert("include_dir".into(), "/I{path}".into());
    d.structure.insert("define".into(), "/D{name}".into());
    d.structure.insert("define_value".into(), "/D{name}={value}".into());
    d.structure.insert("output_obj".into(), "/Fo{path}".into());
    d.structure.insert("output_bin".into(), "/Fe{path}".into());
    d.structure.insert("compile_only".into(), "/c".into());
    d.structure.insert("dep_file".into(), "/showIncludes".into());
    d.structure.insert("dep_file_mode".into(), "stdout".into());
    d.structure.insert("system_lib".into(), "{name}.lib".into());
    d.toolset.insert("ld".into(), "lld-link".into());
    d.toolset.insert("ar".into(), "llvm-lib".into());
    d.linking.push(("c".into(), LinkingParams {
        abi: "c".into(),
        compatible: vec![],
        compile_binary: Some("clang-cl".into()),
        linker: "".into(),
        extensions: vec![".c".into()],
    }));
    d.linking.push(("cpp".into(), LinkingParams {
        abi: "c++".into(),
        compatible: vec!["c".into()],
        linker: "".into(),
        extensions: vec![".cpp".into(), ".cc".into(), ".cxx".into(), ".c++".into()],
        compile_binary: None,
    }));
    CompilerTemplate::from_def(d).unwrap()
}

// ── MASM — Microsoft Macro Assembler ─────────────────────────────────────────

/// `ml.exe` / `ml64.exe` — Microsoft Macro Assembler, required for Windows
/// kernel and driver development. Windows only.
pub fn masm() -> CompilerTemplate {
    let mut d = ToolchainDef {
        name: "masm".into(),
        binary: "ml64.exe".into(),
        alias: Some("ml.exe".into()),
        family: "".into(),
        version_arg: "".into(),
        version_regex: r"(\d+\.\d+\.\d+\.\d+)".into(),
        extensions: vec![".asm".into(), ".masm".into()],
        supported_os: vec!["windows".into()],
        flags_debug: "/Zi".into(),
        flags_lto: "".into(),
        requires_toolchain: vec!["cpp".into()],
        ..Default::default()
    };
    d.flags_opt.insert("0".into(), "".into());
    d.flags_opt.insert("1".into(), "".into());
    d.flags_opt.insert("2".into(), "".into());
    d.flags_opt.insert("3".into(), "".into());
    d.flags_opt.insert("s".into(), "".into());
    d.flags_opt.insert("z".into(), "".into());
    d.flags_warnings.insert("none".into(), "".into());
    d.flags_warnings.insert("default".into(), "".into());
    d.flags_warnings.insert("all".into(), "".into());
    d.flags_warnings.insert("error".into(), "".into());
    d.structure.insert("include_dir".into(), "/I{path}".into());
    d.structure.insert("define".into(), "/D{name}".into());
    d.structure.insert("define_value".into(), "/D{name}={value}".into());
    d.structure.insert("output".into(), "/Fo {path}".into());
    d.structure.insert("compile_only".into(), "/c".into());
    d.structure.insert("dep_file_mode".into(), "none".into());
    d.linking.push(("asm".into(), LinkingParams {
        abi: "c".into(),
        compatible: vec!["c".into(), "cpp".into()],
        linker: "c++".into(),
        extensions: vec![".asm".into(), ".masm".into()],
        compile_binary: None,
    }));
    CompilerTemplate::from_def(d).unwrap()
}

pub fn templates() -> Vec<CompilerTemplate> {
    vec![msvc(), clang_cl(), masm()]
}
