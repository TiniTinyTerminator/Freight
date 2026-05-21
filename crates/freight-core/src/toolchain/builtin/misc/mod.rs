use crate::toolchain::template::{CompilerTemplate, ToolchainDef, LinkingParams};

// ── TCC ───────────────────────────────────────────────────────────────────────

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

// ── DMD ───────────────────────────────────────────────────────────────────────

pub fn dmd() -> CompilerTemplate {
    use crate::toolchain::template::OptionHandler;
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

// ── OpenCL ────────────────────────────────────────────────────────────────────

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

// ── Circle ────────────────────────────────────────────────────────────────────

/// `circle` — experimental C++20+ compiler with compile-time metaprogramming
/// extensions. Drop-in Clang-compatible flag set.
pub fn circle() -> CompilerTemplate {
    let mut d = ToolchainDef {
        name: "circle".into(),
        binary: "circle".into(),
        family: "llvm".into(),
        version_arg: "--version".into(),
        // "circle version 183" → capture the build number as version
        version_regex: r"version (\d+)".into(),
        extensions: vec![".cpp".into(), ".cc".into(), ".cxx".into(), ".c++".into()],
        flags_debug: "-g".into(),
        flags_lto: "-flto".into(),
        sanitize: "-fsanitize={values}".into(),
        sanitizer_options: vec!["address".into(), "undefined".into()],
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
    d.standards.insert("c++20".into(), "-std=c++20".into());
    d.standards.insert("c++23".into(), "-std=c++23".into());
    d.structure.insert("include_dir".into(), "-I{path}".into());
    d.structure.insert("define".into(), "-D{name}".into());
    d.structure.insert("define_value".into(), "-D{name}={value}".into());
    d.structure.insert("output".into(), "-o {path}".into());
    d.structure.insert("compile_only".into(), "-c".into());
    d.structure.insert("dep_file".into(), "-MMD -MF {path}".into());
    d.structure.insert("target".into(), "--target={triple}".into());
    d.toolset.insert("ld".into(), "circle".into());
    d.toolset.insert("ar".into(), "ar".into());
    d.linking.push(("cpp".into(), LinkingParams {
        abi: "c++".into(),
        compatible: vec!["c".into()],
        linker: "".into(),
        extensions: vec![".cpp".into(), ".cc".into(), ".cxx".into(), ".c++".into()],
        compile_binary: None,
    }));
    CompilerTemplate::from_def(d).unwrap()
}

// ── NAG Fortran ───────────────────────────────────────────────────────────────

/// `nagfor` — Numerical Algorithms Group Fortran compiler. The strictest
/// Fortran standard checker available; popular in academic HPC environments.
pub fn nagfor() -> CompilerTemplate {
    let mut d = ToolchainDef {
        name: "nagfor".into(),
        binary: "nagfor".into(),
        family: "".into(),
        version_arg: "-V".into(),
        // "NAG Fortran Compiler Release 7.2(Morzine) Build 7202"
        version_regex: r"Release (\d+\.\d+)".into(),
        extensions: vec![
            ".f90".into(), ".f95".into(), ".f03".into(), ".f08".into(),
            ".f".into(), ".F90".into(),
        ],
        flags_debug: "-g".into(),
        flags_lto: "".into(),
        ..Default::default()
    };
    d.flags_opt.insert("0".into(), "-O0".into());
    d.flags_opt.insert("1".into(), "-O1".into());
    d.flags_opt.insert("2".into(), "-O2".into());
    d.flags_opt.insert("3".into(), "-O4".into()); // NAG uses -O4 for full opt
    d.flags_opt.insert("s".into(), "-O2".into());
    d.flags_opt.insert("z".into(), "-O2".into());
    // NAG uses -w=obs etc.; -w suppresses all, -w=all enables all
    d.flags_warnings.insert("none".into(), "-w=all -quiet".into());
    d.flags_warnings.insert("default".into(), "".into());
    d.flags_warnings.insert("all".into(), "-w=obs -w=unused -w=undef".into());
    // -halt=error turns any warning into a fatal error
    d.flags_warnings.insert("error".into(), "-w=obs -w=unused -w=undef -halt=error".into());
    d.standards.insert("f95".into(), "-f95".into());
    d.standards.insert("f2003".into(), "-f2003".into());
    d.standards.insert("f2008".into(), "-f2008".into());
    d.standards.insert("f2018".into(), "-f2018".into());
    // -ieee=full enables strict IEEE floating-point; useful for numerical code
    d.structure.insert("include_dir".into(), "-I{path}".into());
    d.structure.insert("define".into(), "-D{name}".into());
    d.structure.insert("define_value".into(), "-D{name}={value}".into());
    d.structure.insert("output".into(), "-o {path}".into());
    d.structure.insert("compile_only".into(), "-c".into());
    d.structure.insert("dep_file_mode".into(), "none".into());
    d.toolset.insert("ld".into(), "nagfor".into());
    d.toolset.insert("ar".into(), "ar".into());
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

// ── GNAT (Ada) ────────────────────────────────────────────────────────────────

/// `gnat` — GNU Ada Translator (GNAT), part of GCC. Compiles `.adb` (body) and
/// `.ads` (spec) Ada source files. Requires GCC with Ada language support.
pub fn gnat() -> CompilerTemplate {
    let mut d = ToolchainDef {
        name: "gnat".into(),
        binary: "gnat".into(),
        family: "gnu".into(),
        version_arg: "--version".into(),
        // "GNAT Community Edition 2021 (20210519-103)" or "GNAT 13.2.0"
        version_regex: r"(?:GNAT.*?(\d{4})|GNAT \w+ (\d+\.\d+))".into(),
        extensions: vec![".adb".into(), ".ads".into()],
        flags_debug: "-g".into(),
        flags_lto: "-flto".into(),
        ..Default::default()
    };
    d.flags_opt.insert("0".into(), "-O0".into());
    d.flags_opt.insert("1".into(), "-O1".into());
    d.flags_opt.insert("2".into(), "-O2".into());
    d.flags_opt.insert("3".into(), "-O3".into());
    d.flags_opt.insert("s".into(), "-Os".into());
    d.flags_opt.insert("z".into(), "-Os".into());
    // GNAT uses -gnatw flags for warnings
    d.flags_warnings.insert("none".into(), "-gnatws".into());   // suppress all
    d.flags_warnings.insert("default".into(), "".into());
    d.flags_warnings.insert("all".into(), "-gnatwa".into());    // all warnings
    d.flags_warnings.insert("error".into(), "-gnatwa -gnatwe".into()); // warnings as errors
    d.standards.insert("ada83".into(), "-gnat83".into());
    d.standards.insert("ada95".into(), "-gnat95".into());
    d.standards.insert("ada2005".into(), "-gnat2005".into());
    d.standards.insert("ada2012".into(), "-gnat2012".into());
    d.standards.insert("ada2022".into(), "-gnat2022".into());
    d.structure.insert("include_dir".into(), "-I{path}".into());
    d.structure.insert("define".into(), "-D{name}".into());
    d.structure.insert("define_value".into(), "-D{name}={value}".into());
    d.structure.insert("output".into(), "-o {path}".into());
    // GNAT compile invocation: `gnat compile` or `gcc -c` with .adb
    d.structure.insert("compile_only".into(), "-c".into());
    d.structure.insert("dep_file_mode".into(), "none".into());
    d.toolset.insert("ld".into(), "gnat".into());
    d.toolset.insert("ar".into(), "ar".into());
    d.linking.push(("ada".into(), LinkingParams {
        abi: "ada".into(),
        compatible: vec!["c".into()],
        linker: "".into(),
        extensions: vec![".adb".into(), ".ads".into()],
        compile_binary: Some("gnat".into()),
    }));
    CompilerTemplate::from_def(d).unwrap()
}

// ── Swift ─────────────────────────────────────────────────────────────────────

/// `swiftc` — Apple Swift compiler. Produces object files linkable with C.
/// Swift has its own module system (`.swiftmodule`); inter-module dependencies
/// are not yet tracked by freight's module DAG.
pub fn swiftc() -> CompilerTemplate {
    let mut d = ToolchainDef {
        name: "swiftc".into(),
        binary: "swiftc".into(),
        family: "".into(),
        version_arg: "--version".into(),
        // "Swift version 5.10.1 (swift-5.10.1-RELEASE)"
        version_regex: r"Swift version (\d+\.\d+(?:\.\d+)?)".into(),
        extensions: vec![".swift".into()],
        flags_debug: "-g".into(),
        flags_lto: "-lto=llvm-full".into(),
        ..Default::default()
    };
    d.flags_opt.insert("0".into(), "-Onone".into());
    d.flags_opt.insert("1".into(), "-O".into());
    d.flags_opt.insert("2".into(), "-O".into());
    d.flags_opt.insert("3".into(), "-O -whole-module-optimization".into());
    d.flags_opt.insert("s".into(), "-Osize".into());
    d.flags_opt.insert("z".into(), "-Osize".into());
    d.flags_warnings.insert("none".into(), "-suppress-warnings".into());
    d.flags_warnings.insert("default".into(), "".into());
    d.flags_warnings.insert("all".into(), "-warnings-as-notes".into());
    d.flags_warnings.insert("error".into(), "-warnings-as-errors".into());
    d.structure.insert("include_dir".into(), "-I{path}".into());
    d.structure.insert("define".into(), "-D{name}".into());
    d.structure.insert("define_value".into(), "-D{name}={value}".into());
    d.structure.insert("output".into(), "-o {path}".into());
    d.structure.insert("compile_only".into(), "-c".into());
    d.structure.insert("dep_file_mode".into(), "none".into());
    d.toolset.insert("ld".into(), "swiftc".into());
    d.linking.push(("swift".into(), LinkingParams {
        abi: "swift".into(),
        compatible: vec!["c".into()],
        linker: "".into(),
        extensions: vec![".swift".into()],
        compile_binary: None,
    }));
    CompilerTemplate::from_def(d).unwrap()
}

pub fn templates() -> Vec<CompilerTemplate> {
    vec![tcc(), dmd(), opencl(), circle(), nagfor(), gnat(), swiftc()]
}
