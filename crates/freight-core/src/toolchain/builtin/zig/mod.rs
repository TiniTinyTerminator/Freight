use crate::toolchain::template::{CompilerTemplate, ToolchainDef, LinkingParams};

fn zig_base(name: &str, subcommand: &str) -> ToolchainDef {
    let mut d = ToolchainDef {
        name: name.into(),
        binary: "zig".into(),
        family: "".into(),
        // `zig version` prints just "0.14.0"
        version_arg: "version".into(),
        version_regex: r"(\d+\.\d+\.\d+)".into(),
        subcommand: Some(subcommand.into()),
        flags_debug: "-g".into(),
        flags_lto: "-flto".into(),
        sanitize: "-fsanitize={values}".into(),
        sanitizer_options: vec!["address".into(), "undefined".into(), "thread".into()],
        ..Default::default()
    };
    d.flags_opt.insert("0".into(), "-O0".into());
    d.flags_opt.insert("1".into(), "-O1".into());
    d.flags_opt.insert("2".into(), "-O2".into());
    d.flags_opt.insert("3".into(), "-O3".into());
    d.flags_opt.insert("s".into(), "-Os".into());
    d.flags_opt.insert("z".into(), "-Oz".into());
    d.flags_warnings.insert("none".into(), "".into());
    d.flags_warnings.insert("default".into(), "-Wall".into());
    d.flags_warnings.insert("all".into(), "-Wall -Wextra -Wpedantic".into());
    d.flags_warnings.insert("error".into(), "-Wall -Wextra -Wpedantic -Werror".into());
    d.flags_stdlib.insert("libc++".into(), "-stdlib=libc++".into());
    d.flags_stdlib.insert("libstdc++".into(), "-stdlib=libstdc++".into());
    d.flags_stdlib.insert("none".into(), "-nostdlib".into());
    d.structure.insert("include_dir".into(), "-I{path}".into());
    d.structure.insert("define".into(), "-D{name}".into());
    d.structure.insert("define_value".into(), "-D{name}={value}".into());
    d.structure.insert("output".into(), "-o {path}".into());
    d.structure.insert("compile_only".into(), "-c".into());
    d.structure.insert("dep_file".into(), "-MMD -MF {path}".into());
    // zig cc accepts --target=<triple> for cross-compilation
    d.structure.insert("target".into(), "--target={triple}".into());
    d.toolset.insert("ar".into(), "zig ar".into());
    d.toolset.insert("strip".into(), "strip".into());
    d
}

/// `zig cc` — Zig's bundled Clang used as a drop-in C compiler.
/// Excellent for cross-compilation: a single `zig` binary can target any triple.
pub fn zig_c() -> CompilerTemplate {
    let mut d = zig_base("zig-c", "cc");
    d.extensions = vec![".c".into(), ".s".into(), ".S".into()];
    d.defaults.insert("std".into(), "c11".into());
    d.standards.insert("c11".into(), "-std=c11".into());
    d.standards.insert("c17".into(), "-std=c17".into());
    d.standards.insert("c23".into(), "-std=c23".into());
    d.toolset.insert("cc".into(), "zig cc".into());
    d.toolset.insert("cxx".into(), "zig c++".into());
    d.toolset.insert("ld".into(), "zig c++".into());
    d.linking.push(("c".into(), LinkingParams {
        abi: "c".into(),
        compatible: vec!["asm".into()],
        compile_binary: None,
        linker: "".into(),
        extensions: vec![".c".into(), ".s".into(), ".S".into()],
    }));
    CompilerTemplate::from_def(d).unwrap()
}

/// `zig c++` — Zig's bundled Clang used as a drop-in C++ compiler.
pub fn zig_cxx() -> CompilerTemplate {
    let mut d = zig_base("zig-c++", "c++");
    d.extensions = vec![".cpp".into(), ".cc".into(), ".cxx".into(), ".c++".into()];
    d.defaults.insert("std".into(), "c++17".into());
    d.standards.insert("c++17".into(), "-std=c++17".into());
    d.standards.insert("c++20".into(), "-std=c++20".into());
    d.standards.insert("c++23".into(), "-std=c++23".into());
    d.toolset.insert("cc".into(), "zig cc".into());
    d.toolset.insert("cxx".into(), "zig c++".into());
    d.toolset.insert("ld".into(), "zig c++".into());
    d.linking.push(("cpp".into(), LinkingParams {
        abi: "c++".into(),
        compatible: vec!["c".into()],
        linker: "".into(),
        extensions: vec![".cpp".into(), ".cc".into(), ".cxx".into(), ".c++".into()],
        compile_binary: None,
    }));
    CompilerTemplate::from_def(d).unwrap()
}

pub fn templates() -> Vec<CompilerTemplate> {
    vec![zig_c(), zig_cxx()]
}
