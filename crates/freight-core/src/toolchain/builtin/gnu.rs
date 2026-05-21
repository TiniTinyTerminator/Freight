use std::collections::HashMap;
use crate::toolchain::template::{CompilerTemplate, ToolchainDef, LinkingParams};

fn gnu_base(name: &str, binary: &str) -> ToolchainDef {
    let mut d = ToolchainDef {
        name: name.into(),
        binary: binary.into(),
        family: "gnu".into(),
        version_arg: "--version".into(),
        version_regex: r"\b(\d+\.\d+\.\d+)\b".into(),
        flags_debug: "-g".into(),
        flags_lto: "-flto".into(),
        sanitize: "-fsanitize={values}".into(),
        cpu_ext: "-m{name}".into(),
        sanitizer_options: vec![
            "address".into(), "undefined".into(), "thread".into(), "leak".into(),
        ],
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
    d.flags_stdlib.insert("libstdc++".into(), "".into());
    d.flags_stdlib.insert("none".into(), "-nostdlib".into());
    d.structure.insert("include_dir".into(), "-I{path}".into());
    d.structure.insert("define".into(), "-D{name}".into());
    d.structure.insert("define_value".into(), "-D{name}={value}".into());
    d.structure.insert("output".into(), "-o {path}".into());
    d.structure.insert("compile_only".into(), "-c".into());
    d.structure.insert("dep_file".into(), "-MMD -MF {path}".into());
    d.structure.insert("sysroot".into(), "--sysroot={path}".into());
    d.toolset.insert("ar".into(), "ar".into());
    d.toolset.insert("strip".into(), "strip".into());
    // x86_64 always-flag is added at detect time; arch_flags handles it
    d
}

pub fn gpp() -> CompilerTemplate {
    let mut d = gnu_base("g++", "g++");
    d.alias = Some("gcc".into());
    d.extensions = vec![".cpp".into(), ".cppm".into(), ".ixx".into(), ".mpp".into(),
                        ".cc".into(), ".cxx".into(), ".c++".into()];
    d.defaults.insert("std".into(), "c++17".into());
    d.standards.insert("c++17".into(), "-std=c++17".into());
    d.standards.insert("c++20".into(), "-std=c++20".into());
    d.standards.insert("c++23".into(), "-std=c++23".into());
    d.toolset.insert("cc".into(), "gcc".into());
    d.toolset.insert("cxx".into(), "g++".into());
    d.toolset.insert("ld".into(), "g++".into());
    d.module_style = "gcc".into();
    d.module_params.insert("enable_flag".into(), "-fmodules-ts".into());
    d.module_params.insert("compile_miu".into(), "-fmodule-output={pcm_path}".into());
    d.module_params.insert("import_module".into(), "-fmodule-file={name}={pcm_path}".into());
    d.module_params.insert("header_unit".into(), "-fmodule-header".into());
    d.pch.insert("compile".into(), "-x c++-header".into());
    d.pch.insert("use".into(), "-include {header_path}".into());
    d.pch.insert("extension".into(), ".gch".into());
    d.pch.insert("clangd_flag".into(), "-include {header_path}".into());
    d.linking.push(("cpp".into(), LinkingParams {
        abi: "c++".into(),
        compatible: vec!["c".into(), "fortran".into()],
        linker: "".into(),
        extensions: vec![".cpp".into(), ".cppm".into(), ".ixx".into(), ".mpp".into(),
                         ".cc".into(), ".cxx".into(), ".c++".into()],
        compile_binary: None,
    }));
    CompilerTemplate::from_def(d).unwrap()
}

pub fn gcc() -> CompilerTemplate {
    let mut d = gnu_base("gcc", "gcc");
    d.extensions = vec![".c".into(), ".s".into(), ".S".into()];
    d.defaults.insert("std".into(), "c11".into());
    d.standards.insert("c11".into(), "-std=c11".into());
    d.standards.insert("c17".into(), "-std=c17".into());
    d.standards.insert("c23".into(), "-std=c23".into());
    d.toolset.insert("cc".into(), "gcc".into());
    d.toolset.insert("cxx".into(), "g++".into());
    d.toolset.insert("ld".into(), "g++".into());
    d.linking.push(("c".into(), LinkingParams {
        abi: "c".into(),
        compatible: vec!["fortran".into(), "asm".into()],
        compile_binary: Some("gcc".into()),
        linker: "".into(),
        extensions: vec![".c".into(), ".s".into(), ".S".into()],
    }));
    CompilerTemplate::from_def(d).unwrap()
}

pub fn gfortran() -> CompilerTemplate {
    let mut d = gnu_base("gfortran", "gfortran");
    d.extensions = vec![".f90".into(), ".f95".into(), ".f03".into(), ".f08".into(),
                        ".f".into(), ".F90".into()];
    d.defaults.insert("std".into(), "f2018".into());
    d.standards.insert("f95".into(), "-std=f95".into());
    d.standards.insert("f2003".into(), "-std=f2003".into());
    d.standards.insert("f2008".into(), "-std=f2008".into());
    d.standards.insert("f2018".into(), "-std=f2018".into());
    d.flags_stdlib = HashMap::new();
    d.sanitizer_options = vec!["address".into(), "undefined".into()];
    d.cpu_ext = "".into();
    d.structure.insert("dep_file".into(), "-cpp -MMD -MF {path}".into());
    d.toolset.insert("ld".into(), "gfortran".into());
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

pub fn gdc() -> CompilerTemplate {
    let mut d = gnu_base("gdc", "gdc");
    d.extensions = vec![".d".into()];
    d.flags_warnings.insert("none".into(), "".into());
    d.flags_warnings.insert("default".into(), "".into());
    d.flags_warnings.insert("all".into(), "-Wall".into());
    d.flags_warnings.insert("error".into(), "-Wall -Werror".into());
    d.structure.insert("include_dir".into(), "-I{path}".into());
    d.structure.insert("define".into(), "-fversion={name}".into());
    d.structure.insert("define_value".into(), "-fversion={name}".into());
    d.toolset.insert("ld".into(), "gdc".into());
    d.linking.push(("d".into(), LinkingParams {
        abi: "d".into(),
        compatible: vec!["c".into()],
        linker: "".into(),
        extensions: vec![".d".into()],
        compile_binary: None,
    }));
    CompilerTemplate::from_def(d).unwrap()
}

pub fn gas() -> CompilerTemplate {
    let mut d = ToolchainDef {
        name: "gas".into(),
        binary: "as".into(),
        family: "".into(),
        version_arg: "--version".into(),
        version_regex: r"GNU assembler (?:\([^)]+\) )?(\d+\.\d+(?:\.\d+)?)".into(),
        extensions: vec![".s".into(), ".S".into()],
        requires_toolchain: vec!["c".into()],
        flags_debug: "--gdwarf-2".into(),
        flags_lto: "".into(),
        ..Default::default()
    };
    d.flags_opt.insert("0".into(), "".into());
    d.flags_opt.insert("1".into(), "".into());
    d.flags_opt.insert("2".into(), "".into());
    d.flags_opt.insert("3".into(), "".into());
    d.flags_opt.insert("s".into(), "".into());
    d.flags_opt.insert("z".into(), "".into());
    d.flags_warnings.insert("none".into(), "--no-warn".into());
    d.flags_warnings.insert("default".into(), "".into());
    d.flags_warnings.insert("all".into(), "--warn".into());
    d.flags_warnings.insert("error".into(), "--warn --fatal-warnings".into());
    d.structure.insert("include_dir".into(), "-I{path}".into());
    d.structure.insert("define".into(), "--defsym {name}=1".into());
    d.structure.insert("define_value".into(), "--defsym {name}={value}".into());
    d.structure.insert("output".into(), "-o {path}".into());
    d.structure.insert("compile_only".into(), "".into());
    d.arch_flags.insert("x86".into(), "--32".into());
    d.linking.push(("gas".into(), LinkingParams {
        abi: "c".into(),
        compatible: vec!["c".into(), "cpp".into()],
        linker: "".into(),
        extensions: vec![".s".into(), ".S".into()],
        compile_binary: None,
    }));
    CompilerTemplate::from_def(d).unwrap()
}

pub fn templates() -> Vec<CompilerTemplate> {
    vec![gpp(), gcc(), gfortran(), gdc(), gas()]
}
