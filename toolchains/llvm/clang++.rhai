include "_llvm-base";
include "../languages/_cpp";

name       = "clang++";
homepage   = "https://clang.llvm.org/";
binary     = _cxx;
sanitizer_options = ["address", "undefined", "thread", "memory", "leak", "hwaddress", "dataflow", "cfi", "safestack"];

toolset["cc"]  = _cc;
toolset["cxx"] = _cxx;
toolset["ld"]  = _cxx;

modules["style"]         = "clang";
modules["precompile"]    = "--precompile";
modules["import_module"] = "-fmodule-file={name}={pcm_path}";
modules["header_unit"]   = "-x c++-header";

pch["compile"]   = "-x c++-header";
pch["use"]       = "-include-pch {pch_path}";
pch["extension"] = ".pch";

fn check() {
    for b in ["clang++", "clang++-18", "clang++-17", "clang++-16", "clang++-15", "clang++-14"] {
        if find_tool(b) != () { return true; }
    }
    false
}
