pub mod amd;
pub mod asm;
pub mod emscripten;
pub mod gnu;
pub mod intel;
pub mod llvm;
pub mod misc;
pub mod nvidia;
pub mod windows;
pub mod zig;

use super::template::CompilerTemplate;

pub fn all_compiler_templates() -> Vec<CompilerTemplate> {
    let mut v = Vec::new();
    v.extend(gnu::templates());
    v.extend(llvm::templates());
    v.extend(nvidia::templates());
    v.extend(intel::templates());
    v.extend(amd::templates());
    v.extend(asm::templates());
    v.extend(windows::templates());
    v.extend(zig::templates());
    v.extend(emscripten::templates());
    v.extend(misc::templates());
    v
}
