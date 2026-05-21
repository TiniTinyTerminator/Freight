use crate::toolchain::template::{CompilerTemplate, OptionHandler, ToolchainDef, LinkingParams};

pub fn tcc() -> CompilerTemplate {
    let mut d = ToolchainDef {
        name: "tcc".into(),
        binary: "tcc".into(),
        family: "".into(),
        version_arg: "-v".into(),
        version_regex: r"version (\d+\.\d+\.\d+)".into(),
        extensions: vec![".c".into()],
        flags_debug: "-g".into(),
        flags_lto: "".into(),
        ..Default::default()
    };
    d.flags_opt.insert("0".into(), "".into());
    d.flags_opt.insert("1".into(), "".into());
    d.flags_opt.insert("2".into(), "".into());
    d.flags_opt.insert("3".into(), "".into());
    d.flags_opt.insert("s".into(), "".into());
    d.flags_opt.insert("z".into(), "".into());
    d.flags_warnings.insert("none".into(), "".into());
    d.flags_warnings.insert("default".into(), "-Wall".into());
    d.flags_warnings.insert("all".into(), "-Wall".into());
    d.flags_warnings.insert("error".into(), "-Wall -Werror".into());
    d.standards.insert("c99".into(), "-std=c99".into());
    d.standards.insert("c11".into(), "-std=c11".into());
    d.standards.insert("c17".into(), "-std=c17".into());
    d.structure.insert("include_dir".into(), "-I{path}".into());
    d.structure.insert("define".into(), "-D{name}".into());
    d.structure.insert("define_value".into(), "-D{name}={value}".into());
    d.structure.insert("output".into(), "-o {path}".into());
    d.structure.insert("compile_only".into(), "-c".into());
    d.toolset.insert("cc".into(), "tcc".into());
    d.toolset.insert("ld".into(), "tcc".into());
    d.toolset.insert("ar".into(), "tcc".into());
    d.linking.push(("c".into(), LinkingParams {
        abi: "c".into(),
        compatible: vec![],
        compile_binary: Some("tcc".into()),
        linker: "".into(),
        extensions: vec![".c".into()],
    }));
    CompilerTemplate::from_def(d).unwrap()
}

pub fn dmd() -> CompilerTemplate {
    fn dip1000_h(v: &str, _: &str, _: &str, _: &str, _: &str) -> Result<Vec<String>, String> {
        if v == "true" { Ok(vec!["-preview=dip1000".into()]) } else { Ok(vec![]) }
    }

    let mut d = ToolchainDef {
        name: "dmd".into(),
        binary: "dmd".into(),
        family: "".into(),
        version_arg: "--version".into(),
        version_regex: r"v(\d+\.\d+\.\d+)".into(),
        extensions: vec![".d".into()],
        flags_debug: "-g".into(),
        flags_lto: "".into(),
        ..Default::default()
    };
    d.flags_opt.insert("0".into(), "".into());
    d.flags_opt.insert("1".into(), "-O".into());
    d.flags_opt.insert("2".into(), "-O".into());
    d.flags_opt.insert("3".into(), "-O -release".into());
    d.flags_opt.insert("s".into(), "-O -release".into());
    d.flags_opt.insert("z".into(), "-O -release".into());
    d.flags_warnings.insert("none".into(), "".into());
    d.flags_warnings.insert("default".into(), "".into());
    d.flags_warnings.insert("all".into(), "-wi".into());
    d.flags_warnings.insert("error".into(), "-w".into());
    d.structure.insert("include_dir".into(), "-I{path}".into());
    d.structure.insert("define".into(), "-version={name}".into());
    d.structure.insert("define_value".into(), "-version={name}".into());
    d.structure.insert("output".into(), "-of{path}".into());
    d.structure.insert("compile_only".into(), "-c".into());
    d.structure.insert("dep_file_mode".into(), "none".into());
    d.structure.insert("system_lib".into(), "-L-l{name}".into());
    d.toolset.insert("ld".into(), "dmd".into());
    d.toolset.insert("ar".into(), "ar".into());
    d.toolset.insert("strip".into(), "strip".into());
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

pub fn opencl() -> CompilerTemplate {
    let mut d = ToolchainDef {
        name: "opencl".into(),
        binary: "clang".into(),
        family: "".into(),
        version_arg: "--version".into(),
        version_regex: r"\b(\d+\.\d+\.\d+)\b".into(),
        extensions: vec![".cl".into()],
        always_flags: vec!["-x".into(), "cl".into()],
        requires_toolchain: vec!["cpp".into()],
        flags_debug: "-g".into(),
        flags_lto: "".into(),
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
    d.standards.insert("CL1.0".into(), "-cl-std=CL1.0".into());
    d.standards.insert("CL1.1".into(), "-cl-std=CL1.1".into());
    d.standards.insert("CL1.2".into(), "-cl-std=CL1.2".into());
    d.standards.insert("CL2.0".into(), "-cl-std=CL2.0".into());
    d.standards.insert("CL3.0".into(), "-cl-std=CL3.0".into());
    d.structure.insert("include_dir".into(), "-I{path}".into());
    d.structure.insert("define".into(), "-D{name}".into());
    d.structure.insert("define_value".into(), "-D{name}={value}".into());
    d.structure.insert("output".into(), "-o {path}".into());
    d.structure.insert("compile_only".into(), "-c".into());
    d.structure.insert("dep_file".into(), "-MMD -MF {path}".into());
    d.toolset.insert("ld".into(), "clang".into());
    d.linking.push(("opencl".into(), LinkingParams {
        abi: "opencl".into(),
        compatible: vec!["c++".into(), "c".into()],
        linker: "c++".into(),
        extensions: vec![".cl".into()],
        compile_binary: None,
    }));
    CompilerTemplate::from_def(d).unwrap()
}

pub fn templates() -> Vec<CompilerTemplate> {
    vec![tcc(), dmd(), msvc(), opencl()]
}
