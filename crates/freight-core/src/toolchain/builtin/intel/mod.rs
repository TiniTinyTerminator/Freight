use crate::toolchain::template::{CompilerTemplate, ToolchainDef, LinkingParams};

fn intel_base(name: &str, binary: &str) -> ToolchainDef {
    let mut d = ToolchainDef {
        name: name.into(),
        binary: binary.into(),
        family: "intel".into(),
        version_arg: "--version".into(),
        flags_debug: "-g".into(),
        supported_archs: vec!["x86".into(), "x86_64".into()],
        supported_os: vec!["linux".into(), "windows".into()],
        required_env: vec!["ONEAPI_ROOT".into()],
        ..Default::default()
    };
    d.flags_opt.insert("0".into(), "-O0".into());
    d.flags_opt.insert("1".into(), "-O1".into());
    d.flags_opt.insert("2".into(), "-O2".into());
    d.flags_opt.insert("3".into(), "-O3".into());
    d.structure.insert("include_dir".into(), "-I{path}".into());
    d.structure.insert("define".into(), "-D{name}".into());
    d.structure.insert("define_value".into(), "-D{name}={value}".into());
    d.structure.insert("output".into(), "-o {path}".into());
    d.structure.insert("compile_only".into(), "-c".into());
    d
}

pub fn icpx() -> CompilerTemplate {
    let mut d = intel_base("icpx", "icpx");
    d.version_regex = r"\b(\d+\.\d+\.\d+)\b".into();
    d.extensions = vec![
        ".cpp".into(), ".cc".into(), ".cxx".into(), ".c++".into(), ".sycl".into(),
    ];
    d.sanitizer_options = vec![
        "address".into(), "undefined".into(), "thread".into(), "leak".into(),
    ];
    d.always_flags = vec!["-fsycl".into()];
    d.flags_opt.insert("s".into(), "-Os".into());
    d.flags_opt.insert("z".into(), "-Oz".into());
    d.flags_warnings.insert("none".into(), "".into());
    d.flags_warnings.insert("default".into(), "-Wall".into());
    d.flags_warnings.insert("all".into(), "-Wall -Wextra".into());
    d.flags_warnings.insert("error".into(), "-Wall -Wextra -Werror".into());
    d.flags_lto = "-flto".into();
    d.sanitize = "-fsanitize={values}".into();
    d.structure.insert("dep_file".into(), "-MMD -MF {path}".into());
    d.structure.insert("target".into(), "--target={triple}".into());
    d.structure.insert("sysroot".into(), "--sysroot={path}".into());
    d.standards.insert("c++17".into(), "-std=c++17".into());
    d.standards.insert("c++20".into(), "-std=c++20".into());
    d.standards.insert("c++23".into(), "-std=c++23".into());
    d.toolset.insert("ld".into(), "icpx".into());
    d.linking.push(("sycl".into(), LinkingParams {
        abi: "sycl".into(),
        compatible: vec!["c++".into(), "c".into(), "fortran".into()],
        linker: "".into(),
        extensions: vec![".sycl".into(), ".cpp".into(), ".cc".into(), ".cxx".into()],
        compile_binary: None,
    }));
    CompilerTemplate::from_def(d).unwrap()
}

pub fn ifx() -> CompilerTemplate {
    let mut d = intel_base("ifx", "ifx");
    d.version_regex = r"(\d+\.\d+\.\d+)".into();
    d.extensions = vec![
        ".f90".into(), ".f95".into(), ".f03".into(), ".f08".into(), ".f".into(), ".F90".into(),
    ];
    d.sanitizer_options = vec!["address".into(), "undefined".into()];
    d.flags_opt.insert("s".into(), "-O2".into());
    d.flags_opt.insert("z".into(), "-O2".into());
    d.flags_warnings.insert("none".into(), "-warn none".into());
    d.flags_warnings.insert("default".into(), "".into());
    d.flags_warnings.insert("all".into(), "-warn all".into());
    d.flags_warnings.insert("error".into(), "-warn all -warn errors".into());
    d.flags_lto = "-ipo".into();
    d.structure.insert("dep_file".into(), "-cpp -MMD -MF {path}".into());
    d.standards.insert("f95".into(), "-std=f95".into());
    d.standards.insert("f2003".into(), "-std=f2003".into());
    d.standards.insert("f2008".into(), "-std=f2008".into());
    d.standards.insert("f2018".into(), "-std=f2018".into());
    d.toolset.insert("ld".into(), "ifx".into());
    d.toolset.insert("ar".into(), "ar".into());
    d.toolset.insert("strip".into(), "strip".into());
    d.linking.push(("fortran".into(), LinkingParams {
        abi: "fortran".into(),
        compatible: vec!["c".into()],
        linker: "".into(),
        extensions: vec![
            ".f90".into(), ".f95".into(), ".f03".into(), ".f08".into(),
            ".f".into(), ".F90".into(),
        ],
        compile_binary: None,
    }));
    CompilerTemplate::from_def(d).unwrap()
}

pub fn ispc() -> CompilerTemplate {
    let mut d = ToolchainDef {
        name: "ispc".into(),
        binary: "ispc".into(),
        family: "intel".into(),
        version_arg: "--version".into(),
        version_regex: r"(\d+\.\d+\.\d+)".into(),
        extensions: vec![".ispc".into()],
        supported_archs: vec!["x86_64".into(), "aarch64".into()],
        requires_toolchain: vec!["cpp".into()],
        flags_debug: "-g".into(),
        flags_lto: "".into(),
        ..Default::default()
    };
    d.flags_opt.insert("0".into(), "-O0".into());
    d.flags_opt.insert("1".into(), "-O1".into());
    d.flags_opt.insert("2".into(), "-O2".into());
    d.flags_opt.insert("3".into(), "-O3".into());
    d.flags_opt.insert("s".into(), "-O2".into());
    d.flags_opt.insert("z".into(), "-O2".into());
    d.flags_warnings.insert("none".into(), "--woff".into());
    d.flags_warnings.insert("default".into(), "".into());
    d.flags_warnings.insert("all".into(), "".into());
    d.flags_warnings.insert("error".into(), "--werror".into());
    d.structure.insert("include_dir".into(), "-I{path}".into());
    d.structure.insert("define".into(), "-D{name}".into());
    d.structure.insert("define_value".into(), "-D{name}={value}".into());
    d.structure.insert("output".into(), "-o {path}".into());
    d.structure.insert("compile_only".into(), "".into());
    d.structure.insert("dep_file".into(), "-MMM {path}".into());
    d.toolset.insert("ld".into(), "ispc".into());
    d.linking.push(("ispc".into(), LinkingParams {
        abi: "ispc".into(),
        compatible: vec!["c++".into(), "c".into()],
        linker: "".into(),
        extensions: vec![".ispc".into()],
        compile_binary: None,
    }));
    CompilerTemplate::from_def(d).unwrap()
}

pub fn templates() -> Vec<CompilerTemplate> {
    vec![icpx(), ifx(), ispc()]
}
