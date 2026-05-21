use crate::toolchain::template::{CompilerTemplate, ToolchainDef, LinkingParams};

pub fn hipcc() -> CompilerTemplate {
    let mut d = ToolchainDef {
        name: "hipcc".into(),
        binary: "hipcc".into(),
        family: "".into(),
        version_arg: "--version".into(),
        version_regex: r"HIP version: (\d+\.\d+\.\d+)".into(),
        extensions: vec![".hip".into()],
        supported_archs: vec!["x86_64".into()],
        supported_os: vec!["linux".into()],
        required_tools: vec!["hipconfig".into()],
        requires_toolchain: vec!["cpp".into()],
        sanitizer_options: vec!["address".into(), "undefined".into()],
        flags_debug: "-g -ggdb".into(),
        flags_lto: "-flto".into(),
        sanitize: "-fsanitize={values}".into(),
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
    d.standards.insert("c++14".into(), "-std=c++14".into());
    d.standards.insert("c++17".into(), "-std=c++17".into());
    d.standards.insert("c++20".into(), "-std=c++20".into());
    d.structure.insert("include_dir".into(), "-I{path}".into());
    d.structure.insert("define".into(), "-D{name}".into());
    d.structure.insert("define_value".into(), "-D{name}={value}".into());
    d.structure.insert("output".into(), "-o {path}".into());
    d.structure.insert("compile_only".into(), "-c".into());
    d.structure.insert("dep_file".into(), "-MMD -MF {path}".into());
    d.structure.insert("target".into(), "--target={triple}".into());
    d.structure.insert("sysroot".into(), "--sysroot={path}".into());
    d.toolset.insert("ld".into(), "hipcc".into());
    d.linking.push(("hip".into(), LinkingParams {
        abi: "hip".into(),
        compatible: vec!["c++".into(), "c".into(), "fortran".into()],
        linker: "c++".into(),
        extensions: vec![".hip".into()],
        compile_binary: None,
    }));
    CompilerTemplate::from_def(d).unwrap()
}

pub fn templates() -> Vec<CompilerTemplate> {
    vec![hipcc()]
}
