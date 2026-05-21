use crate::toolchain::template::{CompilerTemplate, OptionHandler, ToolchainDef, LinkingParams};

fn llvm_base(name: &str, binary: &str) -> ToolchainDef {
    let mut d = ToolchainDef {
        name: name.into(),
        binary: binary.into(),
        family: "llvm".into(),
        version_arg: "--version".into(),
        version_regex: r"\b(\d+\.\d+\.\d+)\b".into(),
        flags_debug: "-g".into(),
        flags_lto: "-flto".into(),
        sanitize: "-fsanitize={values}".into(),
        cpu_ext: "-m{name}".into(),
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
    d.structure.insert("target".into(), "--target={triple}".into());
    d.structure.insert("sysroot".into(), "--sysroot={path}".into());
    d.toolset.insert("ar".into(), "ar".into());
    d.toolset.insert("strip".into(), "strip".into());
    d
}

pub fn clangpp() -> CompilerTemplate {
    fn lto_mode_h(v: &str, ver: &str, _: &str, _: &str, _: &str) -> Result<Vec<String>, String> {
        if v == "thin" {
            let major: u32 = ver.split('.').next().and_then(|s| s.parse().ok()).unwrap_or(0);
            let minor: u32 = ver.split('.').nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
            if major > 3 || (major == 3 && minor >= 9) {
                return Ok(vec!["-flto=thin".into()]);
            }
        } else if v == "full" {
            return Ok(vec!["-flto=full".into()]);
        }
        Ok(vec![])
    }

    let mut d = llvm_base("clang++", "clang++");
    d.alias = Some("clang".into());
    d.extensions = vec![".cpp".into(), ".cppm".into(), ".ixx".into(), ".mpp".into(),
                        ".cc".into(), ".cxx".into(), ".c++".into()];
    d.defaults.insert("std".into(), "c++17".into());
    d.standards.insert("c++17".into(), "-std=c++17".into());
    d.standards.insert("c++20".into(), "-std=c++20".into());
    d.standards.insert("c++23".into(), "-std=c++23".into());
    d.sanitizer_options = vec![
        "address".into(), "undefined".into(), "thread".into(), "memory".into(),
        "leak".into(), "hwaddress".into(), "dataflow".into(), "cfi".into(), "safestack".into(),
    ];
    d.toolset.insert("cc".into(), "clang".into());
    d.toolset.insert("cxx".into(), "clang++".into());
    d.toolset.insert("ld".into(), "clang++".into());
    d.module_style = "clang".into();
    d.module_params.insert("precompile".into(), "--precompile".into());
    d.module_params.insert("import_module".into(), "-fmodule-file={name}={pcm_path}".into());
    d.module_params.insert("header_unit".into(), "-x c++-header".into());
    d.pch.insert("compile".into(), "-x c++-header".into());
    d.pch.insert("use".into(), "-include-pch {pch_path}".into());
    d.pch.insert("extension".into(), ".pch".into());
    d.pch.insert("clangd_flag".into(), "-include {header_path}".into());
    d.linking.push(("cpp".into(), LinkingParams {
        abi: "c++".into(),
        compatible: vec!["c".into(), "fortran".into()],
        linker: "".into(),
        extensions: vec![".cpp".into(), ".cppm".into(), ".ixx".into(), ".mpp".into(),
                         ".cc".into(), ".cxx".into(), ".c++".into()],
        compile_binary: None,
    }));
    d.linking.push(("objcpp".into(), LinkingParams {
        abi: "c++".into(),
        compatible: vec!["c".into(), "objc".into()],
        linker: "".into(),
        extensions: vec![".mm".into()],
        compile_binary: None,
    }));
    d.compiler_option_handlers.insert("lto_mode".into(), OptionHandler {
        default_value: None,
        callback: lto_mode_h,
    });
    CompilerTemplate::from_def(d).unwrap()
}

pub fn clang() -> CompilerTemplate {
    let mut d = llvm_base("clang", "clang");
    d.extensions = vec![".c".into(), ".s".into(), ".S".into()];
    d.defaults.insert("std".into(), "c11".into());
    d.standards.insert("c11".into(), "-std=c11".into());
    d.standards.insert("c17".into(), "-std=c17".into());
    d.standards.insert("c23".into(), "-std=c23".into());
    d.sanitizer_options = vec![
        "address".into(), "undefined".into(), "thread".into(), "memory".into(),
        "leak".into(), "hwaddress".into(), "dataflow".into(), "cfi".into(), "safestack".into(),
    ];
    d.toolset.insert("cc".into(), "clang".into());
    d.toolset.insert("cxx".into(), "clang++".into());
    d.toolset.insert("ld".into(), "clang++".into());
    d.linking.push(("c".into(), LinkingParams {
        abi: "c".into(),
        compatible: vec!["fortran".into(), "asm".into()],
        compile_binary: Some("clang".into()),
        linker: "".into(),
        extensions: vec![".c".into(), ".s".into(), ".S".into()],
    }));
    d.linking.push(("objc".into(), LinkingParams {
        abi: "objc".into(),
        compatible: vec!["c".into()],
        compile_binary: Some("clang".into()),
        linker: "".into(),
        extensions: vec![".m".into()],
    }));
    CompilerTemplate::from_def(d).unwrap()
}

pub fn flang() -> CompilerTemplate {
    let mut d = llvm_base("flang", "flang");
    d.version_regex = r"flang(?:-new)? version (\d+\.\d+\.\d+)".into();
    d.extensions = vec![".f90".into(), ".f95".into(), ".f03".into(), ".f08".into(),
                        ".f".into(), ".F90".into()];
    d.defaults.insert("std".into(), "f2018".into());
    d.standards.insert("f95".into(), "-std=f95".into());
    d.standards.insert("f2003".into(), "-std=f2003".into());
    d.standards.insert("f2008".into(), "-std=f2008".into());
    d.standards.insert("f2018".into(), "-std=f2018".into());
    d.flags_stdlib.clear();
    d.sanitizer_options = vec!["address".into(), "undefined".into()];
    d.flags_opt.insert("s".into(), "-O2".into());
    d.flags_opt.insert("z".into(), "-O2".into());
    d.flags_warnings.insert("all".into(), "-Wall -Wextra".into());
    d.flags_warnings.insert("error".into(), "-Wall -Wextra -Werror".into());
    d.toolset.insert("ld".into(), "flang".into());
    d.linking.push(("fortran".into(), LinkingParams {
        abi: "fortran".into(),
        compatible: vec!["c".into()],
        linker: "".into(),
        extensions: vec![".f90".into(), ".f95".into(), ".f03".into(), ".f08".into(),
                         ".f".into(), ".F90".into()],
        compile_binary: None,
    }));
    CompilerTemplate::from_def(d).unwrap()
}

pub fn ldc2() -> CompilerTemplate {
    fn dip1000_h(v: &str, _: &str, _: &str, _: &str, _: &str) -> Result<Vec<String>, String> {
        if v == "true" { Ok(vec!["-preview=dip1000".into()]) } else { Ok(vec![]) }
    }

    let mut d = llvm_base("ldc2", "ldc2");
    d.extensions = vec![".d".into()];
    d.sanitizer_options = vec![
        "address".into(), "thread".into(), "memory".into(), "undefined".into(),
    ];
    d.flags_warnings.insert("none".into(), "".into());
    d.flags_warnings.insert("default".into(), "".into());
    d.flags_warnings.insert("all".into(), "-wi".into());
    d.flags_warnings.insert("error".into(), "-w".into());
    d.flags_lto = "-flto=full".into();
    d.structure.insert("include_dir".into(), "-I{path}".into());
    d.structure.insert("define".into(), "-d-version={name}".into());
    d.structure.insert("define_value".into(), "-d-version={name}".into());
    d.structure.insert("output".into(), "-of={path}".into());
    d.structure.insert("compile_only".into(), "-c".into());
    d.structure.insert("dep_file".into(), "".into());
    d.structure.insert("dep_file_mode".into(), "none".into());
    d.structure.insert("system_lib".into(), "-L-l{name}".into());
    d.structure.insert("target".into(), "-mtriple={triple}".into());
    d.structure.insert("sysroot".into(), "".into());
    d.toolset.insert("ld".into(), "ldc2".into());
    d.linking.push(("d".into(), LinkingParams {
        abi: "d".into(),
        compatible: vec!["c".into()],
        linker: "".into(),
        extensions: vec![".d".into()],
        compile_binary: None,
    }));
    d.compiler_option_handlers.insert("dip1000".into(), OptionHandler {
        default_value: Some("false".into()),
        callback: dip1000_h,
    });
    CompilerTemplate::from_def(d).unwrap()
}

pub fn templates() -> Vec<CompilerTemplate> {
    vec![clangpp(), clang(), flang(), ldc2()]
}
