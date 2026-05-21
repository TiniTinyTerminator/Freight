use crate::toolchain::template::{CompilerTemplate, OptionHandler, ToolchainDef, LinkingParams};

fn asm_base(name: &str, binary: &str, version_regex: &str) -> ToolchainDef {
    let mut d = ToolchainDef {
        name: name.into(),
        binary: binary.into(),
        family: "".into(),
        version_arg: "--version".into(),
        version_regex: version_regex.into(),
        extensions: vec![".asm".into(), ".nasm".into()],
        supported_archs: vec!["x86".into(), "x86_64".into()],
        requires_toolchain: vec!["c".into()],
        flags_debug: "".into(),
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
    d.flags_warnings.insert("default".into(), "".into());
    d.flags_warnings.insert("all".into(), "-w+all".into());
    d.structure.insert("include_dir".into(), "-I{path}".into());
    d.structure.insert("define".into(), "-D{name}".into());
    d.structure.insert("define_value".into(), "-D{name}={value}".into());
    d.structure.insert("output".into(), "-o {path}".into());
    d.structure.insert("compile_only".into(), "".into());
    d.arch_flags.insert("x86_64.linux".into(), "-f elf64".into());
    d.arch_flags.insert("x86_64.macos".into(), "-f macho64".into());
    d.arch_flags.insert("x86_64.windows".into(), "-f win64".into());
    d.arch_flags.insert("x86.linux".into(), "-f elf32".into());
    d.arch_flags.insert("x86.macos".into(), "-f macho32".into());
    d.arch_flags.insert("x86.windows".into(), "-f win32".into());
    d.linking.push(("asm".into(), LinkingParams {
        abi: "c".into(),
        compatible: vec!["c".into(), "cpp".into()],
        linker: "".into(),
        extensions: vec![".asm".into(), ".nasm".into()],
        compile_binary: None,
    }));
    d
}

fn arch_check_handler(v: &str, _: &str, arch: &str, _: &str, name: &str) -> Result<Vec<String>, String> {
    if !v.is_empty() && arch != v {
        Err(format!("assembler '{name}' requires arch '{v}' but the effective target is '{arch}'"))
    } else {
        Ok(vec![])
    }
}

pub fn nasm() -> CompilerTemplate {
    let mut d = asm_base("nasm", "nasm", r"NASM version (\d+\.\d+(?:\.\d+)?)");
    d.flags_debug = "-g -F dwarf".into();
    d.flags_warnings.insert("error".into(), "-w+all -w+error".into());
    d.toolset.insert("as".into(), "nasm".into());
    d.language_option_handlers.insert("arch".into(), OptionHandler {
        default_value: None,
        callback: arch_check_handler,
    });
    CompilerTemplate::from_def(d).unwrap()
}

pub fn yasm() -> CompilerTemplate {
    let mut d = asm_base("yasm", "yasm", r"yasm (\d+\.\d+\.\d+)");
    d.flags_debug = "-g dwarf2".into();
    d.flags_warnings.insert("error".into(), "-w+all -Werror".into());
    d.toolset.insert("as".into(), "yasm".into());
    d.language_option_handlers.insert("arch".into(), OptionHandler {
        default_value: None,
        callback: arch_check_handler,
    });
    CompilerTemplate::from_def(d).unwrap()
}

pub fn templates() -> Vec<CompilerTemplate> {
    vec![nasm(), yasm()]
}
