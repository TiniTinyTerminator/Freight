//! libclang-backed TU cache for hover, go-to-definition, and inlay hints.
//!
//! libclang is loaded at runtime via dlopen (clang-sys `runtime` feature).
//! If the library is absent `TuCache::try_new` returns `None` and all callers
//! fall back to the existing text-based paths — no hard dependency at link time.

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::path::{Path, PathBuf};

use clang_sys::*;
use serde_json::Value;

// ---------------------------------------------------------------------------
// Public result types
// ---------------------------------------------------------------------------

pub struct HoverInfo {
    /// Cursor spelling (function name, variable name, type name, …).
    pub spelling: String,
    /// Type string, e.g. `"std::vector<int>"` or `"int (int, int)"`.
    pub type_str: Option<String>,
    /// Brief doc comment from the declaration site.
    pub doc: Option<String>,
}

pub struct DefinitionLocation {
    pub path: PathBuf,
    /// 0-based line.
    pub line: u32,
    /// 0-based column.
    pub col: u32,
}

// ---------------------------------------------------------------------------
// TuCache
// ---------------------------------------------------------------------------

pub struct TuCache {
    index: CXIndex,
    tus: HashMap<PathBuf, CXTranslationUnit>,
    cc_dir: Option<PathBuf>,
}

// Safety: CXIndex / CXTranslationUnit are opaque pointers. The Server lives
// entirely on the main thread; these are never aliased across threads.
unsafe impl Send for TuCache {}
unsafe impl Sync for TuCache {}

impl TuCache {
    /// Load libclang via dlopen and create a CXIndex.
    /// Returns `None` if libclang is not available on this system.
    pub fn try_new(cc_dir: Option<PathBuf>) -> Option<Self> {
        if !clang_sys::is_loaded() && clang_sys::load().is_err() {
            tracing::info!("libclang not found — AST-backed LSP features disabled");
            return None;
        }
        let index = unsafe { clang_createIndex(0, 0) };
        if index.is_null() {
            return None;
        }
        tracing::info!("libclang loaded — AST-backed hover/definition/hints active");
        Some(Self {
            index,
            tus: HashMap::new(),
            cc_dir,
        })
    }

    pub fn set_cc_dir(&mut self, cc_dir: Option<PathBuf>) {
        self.cc_dir = cc_dir;
    }

    // -----------------------------------------------------------------------
    // TU lifecycle
    // -----------------------------------------------------------------------

    /// Parse or reparse the translation unit for `path`.
    pub fn open(&mut self, path: &Path) {
        let flags = self.compile_flags_for(path);
        let c_path = match CString::new(path.to_string_lossy().as_bytes()) {
            Ok(s) => s,
            Err(_) => return,
        };
        let c_flags: Vec<CString> = flags
            .iter()
            .filter_map(|f| CString::new(f.as_str()).ok())
            .collect();
        let c_flag_ptrs: Vec<*const i8> = c_flags.iter().map(|s| s.as_ptr()).collect();

        if let Some(&tu) = self.tus.get(path) {
            let result = unsafe {
                clang_reparseTranslationUnit(
                    tu,
                    0,
                    std::ptr::null_mut(),
                    clang_defaultReparseOptions(tu),
                )
            };
            if result != 0 {
                // Reparse failed; dispose and start fresh.
                unsafe { clang_disposeTranslationUnit(tu) };
                self.tus.remove(path);
                self.parse_fresh(&c_path, &c_flag_ptrs, path);
            }
        } else {
            self.parse_fresh(&c_path, &c_flag_ptrs, path);
        }
    }

    pub fn close(&mut self, path: &Path) {
        if let Some(tu) = self.tus.remove(path) {
            unsafe { clang_disposeTranslationUnit(tu) };
        }
    }

    // -----------------------------------------------------------------------
    // Hover
    // -----------------------------------------------------------------------

    /// Return hover information for the cursor at `(line, col)` (0-based).
    pub fn hover(&self, path: &Path, line: u32, col: u32) -> Option<HoverInfo> {
        let tu = *self.tus.get(path)?;
        let c_path = CString::new(path.to_string_lossy().as_bytes()).ok()?;
        unsafe {
            let file = clang_getFile(tu, c_path.as_ptr());
            if file.is_null() {
                return None;
            }
            // clang uses 1-based line/column.
            let loc = clang_getLocation(tu, file, line + 1, col + 1);
            let cursor = clang_getCursor(tu, loc);
            if clang_Cursor_isNull(cursor) != 0 {
                return None;
            }

            // Walk to referenced symbol for richer info.
            let referenced = clang_getCursorReferenced(cursor);
            let src = if clang_Cursor_isNull(referenced) == 0 {
                referenced
            } else {
                cursor
            };

            let spelling = cx_string(clang_getCursorSpelling(src));
            if spelling.is_empty() {
                return None;
            }

            let type_cx = clang_getCursorType(src);
            let type_str = if type_cx.kind != CXType_Invalid {
                let s = cx_string(clang_getTypeSpelling(type_cx));
                if s.is_empty() { None } else { Some(s) }
            } else {
                None
            };

            let doc_raw = cx_string(clang_Cursor_getBriefCommentText(src));
            let doc = if doc_raw.is_empty() { None } else { Some(doc_raw) };

            Some(HoverInfo {
                spelling,
                type_str,
                doc,
            })
        }
    }

    // -----------------------------------------------------------------------
    // Go-to-definition
    // -----------------------------------------------------------------------

    /// Return the definition location for the cursor at `(line, col)` (0-based).
    pub fn definition(&self, path: &Path, line: u32, col: u32) -> Option<DefinitionLocation> {
        let tu = *self.tus.get(path)?;
        let c_path = CString::new(path.to_string_lossy().as_bytes()).ok()?;
        unsafe {
            let file = clang_getFile(tu, c_path.as_ptr());
            if file.is_null() {
                return None;
            }
            let loc = clang_getLocation(tu, file, line + 1, col + 1);
            let cursor = clang_getCursor(tu, loc);
            if clang_Cursor_isNull(cursor) != 0 {
                return None;
            }

            // Prefer definition; fall back to declaration.
            let def = clang_getCursorDefinition(cursor);
            let target = if clang_Cursor_isNull(def) == 0 {
                def
            } else {
                let referenced = clang_getCursorReferenced(cursor);
                if clang_Cursor_isNull(referenced) == 0 {
                    referenced
                } else {
                    return None;
                }
            };

            let target_loc = clang_getCursorLocation(target);
            let mut def_file: CXFile = std::ptr::null_mut();
            let mut def_line: u32 = 0;
            let mut def_col: u32 = 0;
            let mut def_offset: u32 = 0;
            clang_getSpellingLocation(
                target_loc,
                &mut def_file,
                &mut def_line,
                &mut def_col,
                &mut def_offset,
            );

            if def_file.is_null() {
                return None;
            }
            let def_path = cx_string(clang_getFileName(def_file));
            if def_path.is_empty() {
                return None;
            }

            Some(DefinitionLocation {
                path: PathBuf::from(def_path),
                line: def_line.saturating_sub(1),
                col: def_col.saturating_sub(1),
            })
        }
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn parse_fresh(&mut self, c_path: &CString, c_flag_ptrs: &[*const i8], path: &Path) {
        let options = unsafe { clang_defaultEditingTranslationUnitOptions() };
        let tu = unsafe {
            clang_parseTranslationUnit(
                self.index,
                c_path.as_ptr(),
                c_flag_ptrs.as_ptr(),
                c_flag_ptrs.len() as i32,
                std::ptr::null_mut(),
                0,
                options,
            )
        };
        if !tu.is_null() {
            self.tus.insert(path.to_path_buf(), tu);
            tracing::debug!(path = %path.display(), "libclang: TU parsed");
        } else {
            tracing::warn!(path = %path.display(), "libclang: clang_parseTranslationUnit returned null");
        }
    }

    fn compile_flags_for(&self, path: &Path) -> Vec<String> {
        let Some(cc_dir) = &self.cc_dir else {
            return vec![];
        };
        let cc_file = cc_dir.join("compile_commands.json");
        let Ok(content) = std::fs::read_to_string(&cc_file) else {
            return vec![];
        };
        let Ok(entries) = serde_json::from_str::<Vec<Value>>(&content) else {
            return vec![];
        };
        let path_str = path.to_string_lossy();
        for entry in &entries {
            let file = entry.get("file").and_then(Value::as_str).unwrap_or("");
            if file == path_str || PathBuf::from(file) == path {
                return extract_compile_flags(entry);
            }
        }
        vec![]
    }
}

impl Drop for TuCache {
    fn drop(&mut self) {
        for tu in self.tus.values().copied() {
            unsafe { clang_disposeTranslationUnit(tu) };
        }
        unsafe { clang_disposeIndex(self.index) };
    }
}

// ---------------------------------------------------------------------------
// Hover markdown rendering
// ---------------------------------------------------------------------------

pub fn hover_info_to_markdown(info: &HoverInfo) -> String {
    let mut out = String::new();
    if let Some(type_str) = &info.type_str {
        out.push_str(&format!("```cpp\n{}: {}\n```", info.spelling, type_str));
    } else {
        out.push_str(&format!("```cpp\n{}\n```", info.spelling));
    }
    if let Some(doc) = &info.doc {
        out.push_str("\n\n");
        out.push_str(doc);
    }
    out
}

// ---------------------------------------------------------------------------
// compile_commands.json flag extraction
// ---------------------------------------------------------------------------

fn extract_compile_flags(entry: &Value) -> Vec<String> {
    // Prefer "arguments" array; fall back to shell-split "command" string.
    if let Some(args) = entry.get("arguments").and_then(Value::as_array) {
        let mut flags: Vec<String> = args
            .iter()
            .filter_map(Value::as_str)
            .skip(1) // skip compiler executable
            .map(str::to_string)
            .collect();
        strip_output_and_source(&mut flags);
        return flags;
    }
    if let Some(cmd) = entry.get("command").and_then(Value::as_str) {
        let mut flags: Vec<String> = cmd
            .split_whitespace()
            .skip(1)
            .map(str::to_string)
            .collect();
        strip_output_and_source(&mut flags);
        return flags;
    }
    vec![]
}

/// Remove `-o <path>` pairs and the source file argument from a flags list.
fn strip_output_and_source(flags: &mut Vec<String>) {
    let source_exts = [
        ".c", ".cc", ".cpp", ".cxx", ".c++", ".cu", ".hip", ".m", ".mm", ".cl", ".ispc",
    ];
    let mut i = 0;
    while i < flags.len() {
        let f = &flags[i];
        if f == "-o" {
            flags.remove(i);
            if i < flags.len() {
                flags.remove(i);
            }
        } else if f.starts_with("-o") && f.len() > 2 {
            flags.remove(i);
        } else if !f.starts_with('-') && source_exts.iter().any(|ext| f.ends_with(ext)) {
            flags.remove(i);
        } else {
            i += 1;
        }
    }
}

// ---------------------------------------------------------------------------
// CXString helper
// ---------------------------------------------------------------------------

/// Convert a `CXString` to a Rust `String` and dispose it.
unsafe fn cx_string(s: CXString) -> String {
    let ptr = clang_getCString(s);
    let result = if ptr.is_null() {
        String::new()
    } else {
        CStr::from_ptr(ptr).to_string_lossy().into_owned()
    };
    clang_disposeString(s);
    result
}
