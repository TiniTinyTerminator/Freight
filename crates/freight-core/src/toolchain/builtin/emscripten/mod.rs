use crate::toolchain::template::{CompilerTemplate, ToolchainDef, LinkingParams};

fn emcc_base(name: &str, binary: &str) -> ToolchainDef {
    let mut d = ToolchainDef {
        name: name.into(),
        binary: binary.into(),
        family: "".into(),
        version_arg: "--version".into(),
        // "emcc (Emscripten gcc/clang-like replacement + linker emulating GNU ld) 3.1.x"
        version_regex: r"emcc.*?(\d+\.\d+\.\d+)".into(),
        flags_debug: "-g".into(),
        flags_lto: "-flto".into(),
        // Emscripten only targets wasm32; cross-triple via EMSDK
        supported_archs: vec!["x86_64".into(), "aarch64".into()],
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
    d.flags_warnings.insert("all".into(), "-Wall -Wextra".into());
    d.flags_warnings.insert("error".into(), "-Wall -Wextra -Werror".into());
    d.structure.insert("include_dir".into(), "-I{path}".into());
    d.structure.insert("define".into(), "-D{name}".into());
    d.structure.insert("define_value".into(), "-D{name}={value}".into());
    d.structure.insert("output".into(), "-o {path}".into());
    d.structure.insert("compile_only".into(), "-c".into());
    d.structure.insert("dep_file".into(), "-MMD -MF {path}".into());
    d.toolset.insert("ar".into(), "emar".into());
    d
}

/// `emcc` — Emscripten C compiler targeting WebAssembly/JavaScript.
pub fn emcc() -> CompilerTemplate {
    let mut d = emcc_base("emcc", "emcc");
    d.extensions = vec![".c".into(), ".s".into()];
    d.defaults.insert("std".into(), "c11".into());
    d.standards.insert("c99".into(), "-std=c99".into());
    d.standards.insert("c11".into(), "-std=c11".into());
    d.standards.insert("c17".into(), "-std=c17".into());
    d.toolset.insert("ld".into(), "emcc".into());
    d.linking.push(("c".into(), LinkingParams {
        abi: "c".into(),
        compatible: vec![],
        compile_binary: Some("emcc".into()),
        linker: "".into(),
        extensions: vec![".c".into()],
    }));
    CompilerTemplate::from_def(d).unwrap()
}

/// `em++` — Emscripten C++ compiler targeting WebAssembly/JavaScript.
pub fn empp() -> CompilerTemplate {
    let mut d = emcc_base("em++", "em++");
    d.version_regex = r"em\+\+.*?(\d+\.\d+\.\d+)".into();
    d.extensions = vec![".cpp".into(), ".cc".into(), ".cxx".into(), ".c++".into()];
    d.defaults.insert("std".into(), "c++17".into());
    d.standards.insert("c++17".into(), "-std=c++17".into());
    d.standards.insert("c++20".into(), "-std=c++20".into());
    d.standards.insert("c++23".into(), "-std=c++23".into());
    d.toolset.insert("ld".into(), "em++".into());
    d.linking.push(("cpp".into(), LinkingParams {
        abi: "c++".into(),
        compatible: vec!["c".into()],
        linker: "".into(),
        extensions: vec![".cpp".into(), ".cc".into(), ".cxx".into(), ".c++".into()],
        compile_binary: None,
    }));
    CompilerTemplate::from_def(d).unwrap()
}

/// wasi-sdk — LLVM/Clang cross-compiler targeting `wasm32-wasi` for WASI runtimes
/// (Wasmtime, WasmEdge). Typically installed at `/opt/wasi-sdk/bin/clang`.
pub fn wasi_clang() -> CompilerTemplate {
    let mut d = ToolchainDef {
        name: "wasi-clang".into(),
        binary: "wasi-clang".into(),
        alias: Some("wasi-clang++".into()),
        family: "llvm".into(),
        version_arg: "--version".into(),
        version_regex: r"\b(\d+\.\d+\.\d+)\b".into(),
        extensions: vec![".c".into(), ".cpp".into(), ".cc".into(), ".cxx".into()],
        always_flags: vec!["--target=wasm32-wasi".into()],
        flags_debug: "-g".into(),
        flags_lto: "-flto".into(),
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
    d.flags_warnings.insert("all".into(), "-Wall -Wextra".into());
    d.flags_warnings.insert("error".into(), "-Wall -Wextra -Werror".into());
    d.standards.insert("c11".into(), "-std=c11".into());
    d.standards.insert("c17".into(), "-std=c17".into());
    d.standards.insert("c++17".into(), "-std=c++17".into());
    d.standards.insert("c++20".into(), "-std=c++20".into());
    d.structure.insert("include_dir".into(), "-I{path}".into());
    d.structure.insert("define".into(), "-D{name}".into());
    d.structure.insert("define_value".into(), "-D{name}={value}".into());
    d.structure.insert("output".into(), "-o {path}".into());
    d.structure.insert("compile_only".into(), "-c".into());
    d.structure.insert("dep_file".into(), "-MMD -MF {path}".into());
    d.toolset.insert("ar".into(), "wasi-ar".into());
    d.linking.push(("c".into(), LinkingParams {
        abi: "c".into(),
        compatible: vec![],
        compile_binary: Some("wasi-clang".into()),
        linker: "".into(),
        extensions: vec![".c".into()],
    }));
    d.linking.push(("cpp".into(), LinkingParams {
        abi: "c++".into(),
        compatible: vec!["c".into()],
        linker: "".into(),
        extensions: vec![".cpp".into(), ".cc".into(), ".cxx".into()],
        compile_binary: None,
    }));
    CompilerTemplate::from_def(d).unwrap()
}

pub fn templates() -> Vec<CompilerTemplate> {
    vec![emcc(), empp(), wasi_clang()]
}
