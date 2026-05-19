//! TOML-based toolchain template loader.
//!
//! Provides a declarative alternative to Rhai scripts. The loader:
//!   1. Parses the TOML file into [`TomlToolchain`].
//!   2. Resolves `base = "X"` chains (merging base values first, then current).
//!   3. Evaluates `$(...)` expressions in string values.
//!   4. Detects the binary via the `detect` list or the explicit `binary` field.
//!   5. Constructs a [`super::script::EvalResult`] compatible with the Rhai path,
//!      so the rest of the engine is unaffected.

use std::path::Path;
use std::collections::HashMap;

use indexmap::IndexMap;
use serde::Deserialize;

use crate::error::FreightError;
use super::script::{EvalResult, ToolchainDef, LinkingParams, OptionHandler};

// ── TOML deserialization types ─────────────────────────────────────────────────

/// `base` may be a single string or a list of strings.
#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
enum TomlBase {
    Single(String),
    Multiple(Vec<String>),
}

impl TomlBase {
    fn into_vec(self) -> Vec<String> {
        match self {
            TomlBase::Single(s) => vec![s],
            TomlBase::Multiple(v) => v,
        }
    }
}

#[derive(Deserialize, Default, Clone, Debug)]
struct TomlStructure {
    include_dir:  Option<String>,
    define:       Option<String>,
    define_value: Option<String>,
    output:       Option<String>,
    output_obj:   Option<String>,
    output_bin:   Option<String>,
    compile_only: Option<String>,
    dep_file:     Option<String>,
    dep_file_mode: Option<String>,
    system_lib:   Option<String>,
    target:       Option<String>,
    sysroot:      Option<String>,
}

#[derive(Deserialize, Default, Clone, Debug)]
struct TomlDap {
    #[serde(default)]
    binaries: Vec<String>,
    #[serde(default)]
    vscode_type: String,
    #[serde(default)]
    mi_mode: String,
}

#[derive(Deserialize, Default, Clone, Debug)]
struct TomlVersion {
    argument: Option<String>,
    regex:    Option<String>,
}

#[derive(Deserialize, Default, Clone, Debug)]
struct TomlPassthrough {
    prefix: Option<String>,
}

#[derive(Deserialize, Default, Clone, Debug)]
struct TomlSanitizer {
    options:   Option<Vec<String>>,
    argument:  Option<String>,
}

/// The full TOML toolchain document.
#[derive(Deserialize, Default, Clone, Debug)]
struct TomlToolchain {
    name:              Option<String>,
    family:            Option<String>,
    alias:             Option<String>,
    homepage:          Option<String>,
    kind:              Option<String>,
    base:              Option<TomlBase>,
    binary:            Option<String>,
    version:           Option<TomlVersion>,
    passthrough:       Option<TomlPassthrough>,
    sanitizer:         Option<TomlSanitizer>,
    extensions:        Option<Vec<String>>,
    always_flags:      Option<Vec<String>>,
    supported_archs:   Option<Vec<String>>,
    supported_os:      Option<Vec<String>>,
    required_tools:    Option<Vec<String>>,
    required_env:      Option<Vec<String>>,
    requires_toolchain: Option<Vec<String>>,
    debug:             Option<String>,
    lto:               Option<String>,
    lto_link:          Option<String>,
    cpu_ext:           Option<String>,

    structure:    Option<TomlStructure>,
    toolset:      Option<IndexMap<String, String>>,
    arch_flags:   Option<IndexMap<String, String>>,
    optimization: Option<IndexMap<String, String>>,
    warnings:     Option<IndexMap<String, String>>,
    linking:      Option<IndexMap<String, TomlLinking>>,
    /// `[language.std]` → standards map; `[language.modules]` → module params;
    /// `[language.pch]` → PCH params; other entries become language option handlers.
    language:     Option<IndexMap<String, IndexMap<String, String>>>,
    /// `[compiler.stdlib]` → stdlib flag map; other entries become compiler option handlers.
    compiler:     Option<IndexMap<String, IndexMap<String, String>>>,

    // debugger-specific
    launch:   Option<IndexMap<String, String>>,
    dap:      Option<TomlDap>,
    settings: Option<IndexMap<String, String>>,
    values:   Option<IndexMap<String, Vec<String>>>,

    // formatter/linter-specific
    run: Option<IndexMap<String, String>>,
}

#[derive(Deserialize, Default, Clone, Debug)]
struct TomlLinking {
    #[serde(default)]
    abi: String,
    #[serde(default)]
    compatible: Vec<String>,
    #[serde(default)]
    linker: String,
    #[serde(default)]
    extensions: Vec<String>,
    compile_binary: Option<String>,
}

// ── Expression evaluator ───────────────────────────────────────────────────────

/// Context for evaluating `$(...)` expressions.
#[derive(Clone, Default)]
pub struct EvalCtx<'a> {
    pub arch:    &'a str,
    pub os:      &'a str,
    pub binary:  &'a str,
    pub version: &'a str,
    pub value:   &'a str,
}

/// Expand all `$(...)` occurrences in `s`, replacing each with the evaluated result.
pub fn eval_expr(s: &str, ctx: &EvalCtx<'_>) -> String {
    let mut result = String::new();
    let mut rest = s;
    while let Some(start) = rest.find("$(") {
        result.push_str(&rest[..start]);
        let after_dollar = &rest[start + 2..];
        // Find matching closing paren (handles nesting).
        let Some(end) = find_close_paren(after_dollar) else {
            // Malformed — emit as-is.
            result.push_str(&rest[start..]);
            return result;
        };
        let inner = &after_dollar[..end];
        result.push_str(&eval_inner(inner.trim(), ctx));
        rest = &after_dollar[end + 1..];
    }
    result.push_str(rest);
    result
}

/// Find the position of the `)` that closes the opening `(` (already consumed).
fn find_close_paren(s: &str) -> Option<usize> {
    let mut depth = 1usize;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 { return Some(i); }
            }
            _ => {}
        }
    }
    None
}

/// Evaluate an expression inside `$(...)`.
fn eval_inner(expr: &str, ctx: &EvalCtx<'_>) -> String {
    eval_ternary(expr, ctx)
}

/// Top-level: ternary `cond ? a : b`.
fn eval_ternary(expr: &str, ctx: &EvalCtx<'_>) -> String {
    // Find `?` not inside quotes or parens.
    if let Some((cond_part, rest)) = split_outer(expr, '?') {
        let cond = eval_or(cond_part.trim(), ctx);
        let is_true = cond != "false" && !cond.is_empty() && cond != "0";
        if let Some((true_part, false_part)) = split_outer(rest.trim(), ':') {
            if is_true {
                return eval_ternary(true_part.trim(), ctx);
            } else {
                return eval_ternary(false_part.trim(), ctx);
            }
        }
    }
    eval_or(expr, ctx)
}

/// `a || b`.
fn eval_or(expr: &str, ctx: &EvalCtx<'_>) -> String {
    if let Some(pos) = find_op_pos(expr, "||") {
        let lv = eval_and(expr[..pos].trim(), ctx);
        if lv != "false" && !lv.is_empty() {
            return lv;
        }
        return eval_and(expr[pos+2..].trim(), ctx);
    }
    eval_and(expr, ctx)
}

/// `a && b`.
fn eval_and(expr: &str, ctx: &EvalCtx<'_>) -> String {
    if let Some(pos) = find_op_pos(expr, "&&") {
        let lv = eval_compare(expr[..pos].trim(), ctx);
        if lv == "false" || lv.is_empty() {
            return "false".to_string();
        }
        return eval_compare(expr[pos+2..].trim(), ctx);
    }
    eval_compare(expr, ctx)
}

/// `a == b`, `a != b`, `a >= b`, etc.
fn eval_compare(expr: &str, ctx: &EvalCtx<'_>) -> String {
    for op in &[">=", "<=", "!=", "==", ">", "<"] {
        if let Some(pos) = find_op_pos(expr, op) {
            let lhs = eval_add(expr[..pos].trim(), ctx);
            let rhs = eval_add(expr[pos+op.len()..].trim(), ctx);
            let result = compare_values(&lhs, &rhs, op);
            return if result { "true".to_string() } else { "false".to_string() };
        }
    }
    eval_add(expr, ctx)
}

fn compare_values(a: &str, b: &str, op: &str) -> bool {
    // Try semver comparison if both sides look like versions.
    let cmp = if looks_like_version(a) && looks_like_version(b) {
        version_cmp(a, b)
    } else {
        a.cmp(b)
    };
    match op {
        "==" => cmp == std::cmp::Ordering::Equal,
        "!=" => cmp != std::cmp::Ordering::Equal,
        ">=" => cmp != std::cmp::Ordering::Less,
        "<=" => cmp != std::cmp::Ordering::Greater,
        ">"  => cmp == std::cmp::Ordering::Greater,
        "<"  => cmp == std::cmp::Ordering::Less,
        _    => false,
    }
}

fn looks_like_version(s: &str) -> bool {
    let s = s.trim_matches('\'').trim_matches('"');
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() >= 2 && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()))
}

fn version_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |s: &str| -> Vec<u64> {
        let s = s.trim_matches('\'').trim_matches('"');
        s.split('-').next().unwrap_or(s)
         .split('.')
         .filter_map(|c| c.parse().ok())
         .collect()
    };
    let av = parse(a);
    let bv = parse(b);
    let len = av.len().max(bv.len());
    for i in 0..len {
        let ai = av.get(i).copied().unwrap_or(0);
        let bi = bv.get(i).copied().unwrap_or(0);
        match ai.cmp(&bi) {
            std::cmp::Ordering::Equal => continue,
            ord => return ord,
        }
    }
    std::cmp::Ordering::Equal
}

/// `a + b` (string concatenation).
fn eval_add(expr: &str, ctx: &EvalCtx<'_>) -> String {
    if let Some(pos) = find_op_pos(expr, "+") {
        let lhs = eval_atom(expr[..pos].trim(), ctx);
        let rhs = eval_add(expr[pos+1..].trim(), ctx);
        return format!("{lhs}{rhs}");
    }
    eval_atom(expr, ctx)
}

/// Atoms: variables, string literals, method calls, `!expr`.
fn eval_atom(expr: &str, ctx: &EvalCtx<'_>) -> String {
    let expr = expr.trim();

    // `!expr`
    if let Some(rest) = expr.strip_prefix('!') {
        let val = eval_atom(rest.trim(), ctx);
        return if val == "false" || val.is_empty() {
            "true".to_string()
        } else {
            "false".to_string()
        };
    }

    // Parenthesized sub-expression: (...)
    if let Some(inner_and_rest) = expr.strip_prefix('(') {
        if let Some(end) = find_close_paren(inner_and_rest) {
            return eval_inner(&inner_and_rest[..end], ctx);
        }
    }

    // String literal: 'text' or "text"
    if (expr.starts_with('\'') && expr.ends_with('\'') && expr.len() >= 2)
        || (expr.starts_with('"') && expr.ends_with('"') && expr.len() >= 2)
    {
        return expr[1..expr.len()-1].to_string();
    }

    // env.NAME
    if let Some(var_name) = expr.strip_prefix("env.") {
        // Strip any method calls
        let (var, methods) = split_methods(var_name);
        let val = std::env::var(var).unwrap_or_default();
        return apply_methods(val, methods, ctx);
    }

    // Variables with potential method calls: binary.replace('a','b')
    let (var_expr, methods) = split_methods(expr);

    let base_val = match var_expr {
        "arch"    => ctx.arch.to_string(),
        "os"      => ctx.os.to_string(),
        "binary"  => ctx.binary.to_string(),
        "version" => ctx.version.to_string(),
        "value"   => ctx.value.to_string(),
        "true"    => "true".to_string(),
        "false"   => "false".to_string(),
        _ => var_expr.to_string(),
    };

    apply_methods(base_val, methods, ctx)
}

/// Split `expr` into `(base, [method_calls])` where method_calls are `.replace(...)` etc.
/// Returns (base, vec_of_method_strings).
fn split_methods(expr: &str) -> (&str, Vec<&str>) {
    // Find the first '.' that is followed by a known method name.
    let mut methods = Vec::new();
    let mut base_end = expr.len();
    let mut search_start = 0;
    while let Some(dot) = expr[search_start..].find('.').map(|p| p + search_start) {
        let rest = &expr[dot+1..];
        if rest.starts_with("replace(") || rest.starts_with("len()") {
            base_end = dot;
            // Collect all method calls.
            let mut pos = dot;
            while let Some(method_dot) = expr[pos..].find('.').map(|p| p + pos) {
                let after = &expr[method_dot+1..];
                if let Some(paren) = after.find('(') {
                    if let Some(end_paren) = find_close_paren(&after[paren+1..]) {
                        let method_end = method_dot + 1 + paren + 1 + end_paren + 1;
                        methods.push(&expr[method_dot..method_end]);
                        pos = method_end;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
            break;
        } else {
            search_start = dot + 1;
        }
    }
    (&expr[..base_end], methods)
}

fn apply_methods(mut val: String, methods: Vec<&str>, _ctx: &EvalCtx<'_>) -> String {
    for method in methods {
        if let Some(rest) = method.strip_prefix(".replace(") {
            if let Some(args_end) = rest.rfind(')') {
                let args = &rest[..args_end];
                if let Some((from_raw, to_raw)) = split_outer(args, ',') {
                    let from = unquote(from_raw.trim());
                    let to   = unquote(to_raw.trim());
                    val = val.replace(from.as_ref(), to.as_ref());
                }
            }
        }
    }
    val
}

fn unquote(s: &str) -> std::borrow::Cow<'_, str> {
    if (s.starts_with('\'') && s.ends_with('\''))
        || (s.starts_with('"') && s.ends_with('"'))
    {
        std::borrow::Cow::Owned(s[1..s.len()-1].to_string())
    } else {
        std::borrow::Cow::Borrowed(s)
    }
}

/// Find position of `op` that is not inside single-quotes, double-quotes, or parentheses.
fn find_op_pos(expr: &str, op: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_single = false;
    let mut in_double = false;
    let bytes = expr.as_bytes();
    let op_bytes = op.as_bytes();
    let mut i = 0;
    while i + op.len() <= expr.len() {
        let c = bytes[i] as char;
        if c == '\'' && !in_double { in_single = !in_single; i += 1; continue; }
        if c == '"'  && !in_single { in_double = !in_double; i += 1; continue; }
        if in_single || in_double { i += 1; continue; }
        if c == '(' { depth += 1; i += 1; continue; }
        if c == ')' { depth = depth.saturating_sub(1); i += 1; continue; }
        if depth == 0 && &bytes[i..i+op.len()] == op_bytes {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Split `expr` at the first occurrence of `ch` not inside quotes/parens.
fn split_outer(expr: &str, ch: char) -> Option<(&str, &str)> {
    let pos = find_op_pos(expr, &ch.to_string())?;
    Some((&expr[..pos], &expr[pos+1..]))
}

// ── Base merging ───────────────────────────────────────────────────────────────

/// Merge `overlay` on top of `base`. Scalar fields: overlay wins.
/// Arrays: overlay replaces (when Some). Maps: entries merged (base provides defaults).
fn merge(base: TomlToolchain, overlay: TomlToolchain) -> TomlToolchain {
    macro_rules! scalar {
        ($field:ident) => { overlay.$field.or(base.$field) };
    }
    macro_rules! arr {
        ($field:ident) => { overlay.$field.or(base.$field) };
    }
    macro_rules! map {
        ($field:ident) => {
            match (base.$field, overlay.$field) {
                (None, v) | (v, None) => v,
                (Some(mut b), Some(o)) => {
                    for (k, v) in o { b.insert(k, v); }
                    Some(b)
                }
            }
        };
    }
    macro_rules! map_nested {
        ($field:ident) => {
            match (base.$field, overlay.$field) {
                (None, v) | (v, None) => v,
                (Some(mut b), Some(o)) => {
                    for (k, v) in o {
                        let entry = b.entry(k).or_default();
                        for (ik, iv) in v { entry.insert(ik, iv); }
                    }
                    Some(b)
                }
            }
        };
    }

    TomlToolchain {
        name:               scalar!(name),
        family:             scalar!(family),
        alias:              scalar!(alias),
        homepage:           scalar!(homepage),
        kind:               scalar!(kind),
        base:               overlay.base,   // not propagated
        binary:             scalar!(binary),
        version:            match (base.version, overlay.version) {
            (None, v) | (v, None) => v,
            (Some(b), Some(o)) => Some(TomlVersion {
                argument: o.argument.or(b.argument),
                regex:    o.regex.or(b.regex),
            }),
        },
        passthrough:        match (base.passthrough, overlay.passthrough) {
            (None, v) | (v, None) => v,
            (Some(b), Some(o)) => Some(TomlPassthrough {
                prefix: o.prefix.or(b.prefix),
            }),
        },
        sanitizer:          match (base.sanitizer, overlay.sanitizer) {
            (None, v) | (v, None) => v,
            (Some(b), Some(o)) => Some(TomlSanitizer {
                options:  o.options.or(b.options),
                argument: o.argument.or(b.argument),
            }),
        },
        extensions:         arr!(extensions),
        always_flags:       arr!(always_flags),
        supported_archs:    arr!(supported_archs),
        supported_os:       arr!(supported_os),
        required_tools:     arr!(required_tools),
        required_env:       arr!(required_env),
        requires_toolchain: arr!(requires_toolchain),
        debug:              scalar!(debug),
        lto:                scalar!(lto),
        lto_link:           scalar!(lto_link),
        cpu_ext:            scalar!(cpu_ext),
        structure:          merge_structure(base.structure, overlay.structure),
        toolset:            map!(toolset),
        arch_flags:         map!(arch_flags),
        optimization:       map!(optimization),
        warnings:           map!(warnings),
        linking:            map!(linking),
        language:           map_nested!(language),
        compiler:           map_nested!(compiler),
        launch:             map!(launch),
        dap:                overlay.dap.or(base.dap),
        settings:           map!(settings),
        values:             match (base.values, overlay.values) {
            (None, v) | (v, None) => v,
            (Some(mut b), Some(o)) => { for (k, v) in o { b.insert(k, v); } Some(b) }
        },
        run:                map!(run),
    }
}

fn merge_structure(base: Option<TomlStructure>, overlay: Option<TomlStructure>) -> Option<TomlStructure> {
    match (base, overlay) {
        (None, v) | (v, None) => v,
        (Some(b), Some(o)) => Some(TomlStructure {
            include_dir:  o.include_dir.or(b.include_dir),
            define:       o.define.or(b.define),
            define_value: o.define_value.or(b.define_value),
            output:       o.output.or(b.output),
            output_obj:   o.output_obj.or(b.output_obj),
            output_bin:   o.output_bin.or(b.output_bin),
            compile_only: o.compile_only.or(b.compile_only),
            dep_file:     o.dep_file.or(b.dep_file),
            dep_file_mode: o.dep_file_mode.or(b.dep_file_mode),
            system_lib:   o.system_lib.or(b.system_lib),
            target:       o.target.or(b.target),
            sysroot:      o.sysroot.or(b.sysroot),
        }),
    }
}

// ── Main loader ────────────────────────────────────────────────────────────────

/// Parse `src` as a TOML toolchain template, resolve base inheritance, and
/// return an `EvalResult` compatible with the Rhai path.
pub(crate) fn load_toml_toolchain(src: &str, dir: Option<&Path>) -> Result<EvalResult, FreightError> {
    let raw: TomlToolchain = toml_edit::de::from_str(src)
        .map_err(|e| FreightError::TemplateError(format!("TOML parse error: {e}")))?;

    // Resolve base inheritance.
    let merged = resolve_bases(raw, dir)?;

    // Detect binary.
    let binary = resolve_binary(&merged);

    // Build evaluation context.
    let ctx = EvalCtx {
        arch:    std::env::consts::ARCH,
        os:      std::env::consts::OS,
        binary:  &binary,
        version: "",
        value:   "",
    };

    // Build ToolchainDef from merged data.
    let def = build_def(merged, &binary, &ctx)?;

    // Build option handlers from [compiler.*] and [language.*].
    let (compiler_option_handlers, language_option_handlers) = build_handlers(&def.toml_compiler, &def.toml_language, &ctx);

    let actual_def = def.def;

    Ok(EvalResult {
        def: actual_def,
        engine: rhai::Engine::new(),
        ast: rhai::AST::empty(),
        compiler_option_handlers,
        language_option_handlers,
    })
}

// Temporary wrapper to carry the raw maps alongside the def.
struct DefWithMaps {
    def: ToolchainDef,
    toml_compiler: IndexMap<String, IndexMap<String, String>>,
    toml_language: IndexMap<String, IndexMap<String, String>>,
}

fn resolve_bases(mut current: TomlToolchain, dir: Option<&Path>) -> Result<TomlToolchain, FreightError> {
    let Some(base_field) = current.base.take() else {
        return Ok(current);
    };
    let bases = base_field.into_vec();
    let mut result = TomlToolchain::default();
    for base_name in bases {
        let base_tc = load_base(&base_name, dir)?;
        result = merge(result, base_tc);
    }
    Ok(merge(result, current))
}

fn load_base(name: &str, dir: Option<&Path>) -> Result<TomlToolchain, FreightError> {
    let dir = dir.ok_or_else(|| FreightError::TemplateError(
        format!("base = \"{name}\" requires a directory context")
    ))?;

    // Try name as given first, then with .toml extension.
    let path = {
        let p = dir.join(name);
        if p.extension().is_some() { p } else { p.with_extension("toml") }
    };

    let src = std::fs::read_to_string(&path)
        .map_err(|e| FreightError::TemplateError(
            format!("loading base \"{name}\": {e}")
        ))?;

    let mut raw: TomlToolchain = toml_edit::de::from_str(&src)
        .map_err(|e| FreightError::TemplateError(
            format!("TOML parse error in base \"{name}\": {e}")
        ))?;

    // Recursively resolve the base's own bases.
    if let Some(nested_base) = raw.base.take() {
        let base_dir = path.parent().unwrap_or(dir);
        let nested = resolve_bases(TomlToolchain { base: Some(nested_base), ..raw }, Some(base_dir))?;
        return Ok(nested);
    }

    Ok(raw)
}

/// Resolve the binary name: explicit `binary` field, else fall back to `name`.
fn resolve_binary(tc: &TomlToolchain) -> String {
    tc.binary.clone()
        .or_else(|| tc.name.clone())
        .unwrap_or_default()
}


fn eval_opt_string(s: Option<&str>, ctx: &EvalCtx<'_>) -> String {
    s.map(|v| eval_expr(v, ctx)).unwrap_or_default()
}

fn eval_map(m: Option<&IndexMap<String, String>>, ctx: &EvalCtx<'_>) -> HashMap<String, String> {
    match m {
        None => HashMap::new(),
        Some(im) => im.iter()
            .map(|(k, v)| (k.clone(), eval_expr(v, ctx)))
            .collect(),
    }
}

#[allow(clippy::field_reassign_with_default)]
fn build_def(tc: TomlToolchain, binary: &str, ctx: &EvalCtx<'_>) -> Result<DefWithMaps, FreightError> {
    // Extract compiler/language maps before consuming `tc`.
    let toml_compiler = tc.compiler.clone().unwrap_or_default();
    let toml_language = tc.language.clone().unwrap_or_default();

    let mut def = ToolchainDef::default();

    def.name    = tc.name.clone().unwrap_or_default();
    def.family  = tc.family.clone().unwrap_or_default();
    def.alias   = tc.alias.clone().filter(|s| !s.is_empty());
    def.binary  = binary.to_string();
    def.kind    = tc.kind.clone().unwrap_or_else(|| "compiler".to_string());

    def.version_arg         = tc.version.as_ref().and_then(|v| v.argument.clone()).unwrap_or_default();
    def.version_regex       = tc.version.as_ref().and_then(|v| v.regex.clone()).unwrap_or_default();
    def.passthrough_enabled = tc.passthrough.is_some();
    def.passthrough_prefix  = tc.passthrough.as_ref().and_then(|p| p.prefix.clone()).unwrap_or_default();
    def.sanitizer_options   = tc.sanitizer.as_ref().and_then(|s| s.options.clone()).unwrap_or_default();

    // If no top-level extensions, derive them from all linking section extensions.
    def.extensions = if let Some(exts) = tc.extensions.clone().filter(|v| !v.is_empty()) {
        exts
    } else if let Some(linking) = &tc.linking {
        let mut all: Vec<String> = linking.values()
            .flat_map(|lp| lp.extensions.iter().cloned())
            .collect();
        all.sort();
        all.dedup();
        all
    } else {
        Vec::new()
    };
    def.supported_archs    = tc.supported_archs.clone().unwrap_or_default();
    def.supported_os       = tc.supported_os.clone().unwrap_or_default();
    def.required_tools     = tc.required_tools.clone().unwrap_or_default();
    def.required_env       = tc.required_env.clone().unwrap_or_default();
    def.requires_toolchain = tc.requires_toolchain.clone().unwrap_or_default();

    def.flags_debug    = eval_opt_string(tc.debug.as_deref(), ctx);
    def.flags_lto      = eval_opt_string(tc.lto.as_deref(), ctx);
    def.flags_lto_link = eval_opt_string(tc.lto_link.as_deref(), ctx);
    def.sanitize       = tc.sanitizer.as_ref()
        .and_then(|s| s.argument.as_deref())
        .map(|f| eval_expr(f, ctx))
        .unwrap_or_default();
    def.cpu_ext        = eval_opt_string(tc.cpu_ext.as_deref(), ctx);

    // Flag maps.
    def.flags_opt      = eval_map(tc.optimization.as_ref(), ctx);
    def.flags_warnings = eval_map(tc.warnings.as_ref(), ctx);
    // [compiler.stdlib] → stdlib flag map.
    def.flags_stdlib   = eval_map(
        tc.compiler.as_ref().and_then(|c| c.get("stdlib")),
        ctx,
    );

    // always_flags — evaluate and filter empty strings.
    def.always_flags = tc.always_flags.as_deref().unwrap_or(&[])
        .iter()
        .map(|f| eval_expr(f, ctx))
        .filter(|f| !f.is_empty())
        .collect();

    // arch_flags.
    def.arch_flags = eval_map(tc.arch_flags.as_ref(), ctx);

    // toolset — evaluate values.
    def.toolset = eval_map(tc.toolset.as_ref(), ctx);

    // Structure.
    let s = tc.structure.as_ref();
    let gs = |field: Option<&String>| field.map(|v| eval_expr(v, ctx)).unwrap_or_default();
    def.structure.insert("include_dir".into(),  gs(s.and_then(|s| s.include_dir.as_ref())));
    def.structure.insert("define".into(),       gs(s.and_then(|s| s.define.as_ref())));
    def.structure.insert("define_value".into(), gs(s.and_then(|s| s.define_value.as_ref())));
    def.structure.insert("output".into(),       gs(s.and_then(|s| s.output.as_ref())));
    def.structure.insert("output_obj".into(),   gs(s.and_then(|s| s.output_obj.as_ref())));
    def.structure.insert("output_bin".into(),   gs(s.and_then(|s| s.output_bin.as_ref())));
    def.structure.insert("compile_only".into(), gs(s.and_then(|s| s.compile_only.as_ref())));
    def.structure.insert("dep_file".into(),     gs(s.and_then(|s| s.dep_file.as_ref())));
    def.structure.insert("dep_file_mode".into(),gs(s.and_then(|s| s.dep_file_mode.as_ref())));
    def.structure.insert("system_lib".into(),   gs(s.and_then(|s| s.system_lib.as_ref())));
    def.structure.insert("target".into(),       gs(s.and_then(|s| s.target.as_ref())));
    def.structure.insert("sysroot".into(),      gs(s.and_then(|s| s.sysroot.as_ref())));

    // Linking.
    if let Some(linking) = &tc.linking {
        for (lang, lp) in linking {
            let cb = lp.compile_binary.as_deref()
                .map(|s| eval_expr(s, ctx))
                .filter(|s| !s.is_empty());
            def.linking.push((lang.clone(), LinkingParams {
                abi:            lp.abi.clone(),
                compatible:     lp.compatible.clone(),
                extensions:     lp.extensions.clone(),
                compile_binary: cb,
                linker:         lp.linker.clone(),
            }));
        }
    }

    // [language.std] → standards map and default; [language.modules] → module params;
    // [language.pch] → PCH params; other entries become language option handlers.
    if let Some(lang_map) = &tc.language {
        if let Some(std_map) = lang_map.get("std") {
            for (k, v) in std_map {
                def.standards.insert(k.clone(), v.clone());
            }
            if let Some((first_key, _)) = std_map.iter().next() {
                def.defaults.insert("std".to_string(), first_key.clone());
            }
        }
        if let Some(modules) = lang_map.get("modules") {
            def.module_style = modules.get("style").cloned().unwrap_or_default();
            for (k, v) in modules {
                if k != "style" {
                    def.module_params.insert(k.clone(), eval_expr(v, ctx));
                }
            }
        }
        if let Some(pch) = lang_map.get("pch") {
            for (k, v) in pch {
                def.pch.insert(k.clone(), eval_expr(v, ctx));
            }
        }
    }

    // Defaults from [optimization] first key.
    if let Some(opt) = &tc.optimization {
        if let Some((first_key, _)) = opt.iter().next() {
            def.defaults.insert("opt_level".to_string(), first_key.clone());
        }
    }

    // name is required.
    if def.name.is_empty() {
        return Err(FreightError::TemplateError(
            "TOML toolchain must set `name = \"...\"`".into()
        ));
    }

    Ok(DefWithMaps { def, toml_compiler, toml_language })
}

// ── Option handlers ────────────────────────────────────────────────────────────

/// Build `OptionHandler` entries from `[compiler.*]` and `[language.*]` TOML maps.
///
/// Each key in `[compiler.lto_mode]` / `[language.std]` etc. maps a manifest value
/// to a flag string. The handler looks up the value in the map and calls `add_flag`.
///
/// Special key `"value"`: interpolates `$(value)` directly — the manifest value
/// becomes part of the flag (e.g. `[compiler.sm_arch] value = "--gpu-architecture=$(value)"`).
fn build_handlers(
    compiler_maps: &IndexMap<String, IndexMap<String, String>>,
    language_maps: &IndexMap<String, IndexMap<String, String>>,
    ctx: &EvalCtx<'_>,
) -> (HashMap<String, OptionHandler>, HashMap<String, OptionHandler>) {
    // We can't use Rhai FnPtrs here, so we use a workaround:
    // store the map data in the OptionHandler using a synthetic Rhai script that
    // encodes the lookup. However, since OptionHandler requires a FnPtr, and we
    // don't want to use Rhai at all for TOML handlers, we need a different approach.
    //
    // Looking at run_handlers() in script.rs: it calls handler.callback.call(engine, ast, ctx).
    // For TOML templates, we'll return empty handlers here and instead store the maps
    // on the ToolchainDef — but ToolchainDef doesn't have a field for TOML maps.
    //
    // The cleanest solution: encode each [compiler.*] / [language.*] map as a tiny
    // Rhai script that does the lookup inline. This avoids any changes to the caller.

    let mut comp_handlers = HashMap::new();
    let mut lang_handlers = HashMap::new();

    // Skip `compiler.stdlib` — handled via def.flags_stdlib, not an option handler.
    for (opt_name, map) in compiler_maps {
        if opt_name == "stdlib" { continue; }
        if let Some(handler) = make_map_handler(map, ctx) {
            comp_handlers.insert(opt_name.clone(), handler);
        }
    }

    // Skip `language.std` (→ def.standards), `language.modules` and `language.pch`
    // — all three are handled directly in build_def, not as option handlers.
    for (opt_name, map) in language_maps {
        if matches!(opt_name.as_str(), "std" | "modules" | "pch") { continue; }
        if let Some(handler) = make_map_handler(map, ctx) {
            lang_handlers.insert(opt_name.clone(), handler);
        }
    }

    (comp_handlers, lang_handlers)
}

/// Create a Rhai-backed OptionHandler from a TOML `[compiler.X]` / `[language.X]` map.
///
/// The generated script performs the map lookup and calls `add_flag()`.
fn make_map_handler(
    map: &IndexMap<String, String>,
    ctx: &EvalCtx<'_>,
) -> Option<OptionHandler> {
    // Determine if this is a "value" key (direct interpolation).
    let has_value_key = map.contains_key("value");

    // Build a Rhai script that encodes the lookup.
    let mut script = String::new();

    // Default value = first key in the map.
    let default_value = map.iter().next().map(|(k, _)| k.clone());

    if has_value_key {
        // Special case: flag is generated by interpolating the manifest value.
        let template = map.get("value").map(String::as_str).unwrap_or("");
        // The template may contain $(value); replace it with a Rhai string concat.
        // We generate: add_flag(flag_template.replace("{value_placeholder}", ctx.value))
        // Simplest: bake the template into the script, replacing $(value) with ctx.value ref.
        // Since we can't use the EvalCtx at handler-call time (we only get the Rhai ctx),
        // we need to generate a Rhai script that reads ctx.value.
        let rhai_flag = toml_template_to_rhai(template);
        script = format!(r#"
            fn handler(ctx) {{
                let flag = {rhai_flag};
                if flag != "" {{ add_flag(flag); }}
            }}
        "#);
    } else {
        // Normal case: manifest value is looked up as a key in the map.
        script.push_str("fn handler(ctx) {\n");
        script.push_str("    let v = ctx.value;\n");
        script.push_str("    let flag = \"\";\n");
        for (k, v) in map {
            let escaped_k = k.replace('\\', "\\\\").replace('"', "\\\"");
            // Evaluate the flag at script-generation time (ctx available now).
            // Values like `$(version >= '3.9' ? ...)` need ctx.version at call time.
            // If the value contains `$(version ...)`, we must defer to Rhai.
            if v.contains("$(version") || v.contains("$(arch") || v.contains("$(os") {
                let rhai_flag = toml_template_to_rhai(v);
                let escaped_flag_expr = rhai_flag;
                script.push_str(&format!(
                    "    if v == \"{escaped_k}\" {{ flag = {escaped_flag_expr}; }}\n"
                ));
            } else {
                let evaluated = eval_expr(v, ctx);
                let escaped_v = evaluated.replace('\\', "\\\\").replace('"', "\\\"");
                script.push_str(&format!(
                    "    if v == \"{escaped_k}\" {{ flag = \"{escaped_v}\"; }}\n"
                ));
            }
        }
        script.push_str("    if flag != \"\" { add_flag(flag); }\n");
        script.push_str("}\n");
    }

    // Compile the Rhai script.
    // The engine needs `add_flag` and `version_gte`/`version_lte`/`version_gt`/`version_lt`
    // registered, mirroring what the main Rhai engine provides.
    let mut engine = rhai::Engine::new();
    engine.set_max_operations(100_000);

    // `add_flag` accumulates flags via the PENDING_FLAGS thread-local (shared with script.rs).
    use super::script::PENDING_FLAGS_PUSH;
    engine.register_fn("add_flag", |flag: String| {
        PENDING_FLAGS_PUSH(flag);
    });

    // Version comparison helpers (same as in script.rs).
    engine.register_fn("version_gte", |a: String, b: String| version_cmp(&a, &b) != std::cmp::Ordering::Less);
    engine.register_fn("version_lte", |a: String, b: String| version_cmp(&a, &b) != std::cmp::Ordering::Greater);
    engine.register_fn("version_gt",  |a: String, b: String| version_cmp(&a, &b) == std::cmp::Ordering::Greater);
    engine.register_fn("version_lt",  |a: String, b: String| version_cmp(&a, &b) == std::cmp::Ordering::Less);

    let ast = engine.compile(&script).ok()?;
    let fn_ptr = rhai::FnPtr::new("handler").ok()?;

    Some(OptionHandler {
        default_value,
        callback: fn_ptr,
        engine: Some(std::sync::Arc::new(engine)),
        ast,
    })
}

/// Convert a TOML template string (with `$(value)`, `$(version)`, etc.)
/// into a Rhai expression that reads `ctx.value`, `ctx.version`, etc.
fn toml_template_to_rhai(template: &str) -> String {
    // Replace $(value) → ctx.value, $(version) → ctx.version, $(arch) → ctx.arch
    // Handle simple cases; for complex ternaries, emit a ternary in Rhai.
    let mut rest = template;
    let mut parts: Vec<String> = Vec::new();

    while let Some(start) = rest.find("$(") {
        let literal = &rest[..start];
        if !literal.is_empty() {
            parts.push(format!("\"{}\"", literal.replace('"', "\\\"")));
        }
        let after = &rest[start + 2..];
        if let Some(end) = find_close_paren(after) {
            let inner = &after[..end];
            parts.push(toml_expr_to_rhai(inner.trim()));
            rest = &after[end + 1..];
        } else {
            parts.push(format!("\"{}\"", rest.replace('"', "\\\"")));
            rest = "";
            break;
        }
    }
    if !rest.is_empty() {
        parts.push(format!("\"{}\"", rest.replace('"', "\\\"")));
    }

    if parts.is_empty() {
        format!("\"{}\"", template.replace('"', "\\\""))
    } else if parts.len() == 1 {
        parts.remove(0)
    } else {
        parts.join(" + ")
    }
}

/// Convert a single expression inside `$(...)` to Rhai.
fn toml_expr_to_rhai(expr: &str) -> String {
    // Handle ternary: `cond ? a : b`
    if let Some((cond, rest)) = split_outer(expr, '?') {
        if let Some((a, b)) = split_outer(rest.trim(), ':') {
            let rhai_cond = toml_cond_to_rhai(cond.trim());
            let rhai_a = toml_expr_to_rhai(a.trim());
            let rhai_b = toml_expr_to_rhai(b.trim());
            return format!("(if {rhai_cond} {{ {rhai_a} }} else {{ {rhai_b} }})");
        }
    }
    // Simple variables.
    toml_atom_to_rhai(expr)
}

fn toml_cond_to_rhai(cond: &str) -> String {
    for op in &[">=", "<=", "!=", "==", ">", "<"] {
        if let Some(pos) = find_op_pos(cond, op) {
            let lhs = toml_atom_to_rhai(cond[..pos].trim());
            let rhs = cond[pos+op.len()..].trim();
            // rhs might be a string literal like '3.9'
            let rhs_str = unquote(rhs);
            // Use version comparison helper for version strings.
            if looks_like_version(rhs) {
                let rhai_op = match *op {
                    ">=" => "version_gte",
                    "<=" => "version_lte",
                    ">"  => "version_gt",
                    "<"  => "version_lt",
                    "==" => return format!("{lhs} == \"{rhs_str}\""),
                    "!=" => return format!("{lhs} != \"{rhs_str}\""),
                    _ => unreachable!(),
                };
                return format!("{rhai_op}({lhs}, \"{rhs_str}\")");
            }
            return format!("{lhs} {op} \"{rhs_str}\"");
        }
    }
    toml_atom_to_rhai(cond)
}

fn toml_atom_to_rhai(atom: &str) -> String {
    let atom = atom.trim();
    match atom {
        "version" => "ctx.version".to_string(),
        "arch"    => "ctx.arch".to_string(),
        "os"      => "ctx.os".to_string(),
        "binary"  => "ctx.name".to_string(),
        "value"   => "ctx.value".to_string(),
        other => {
            if (other.starts_with('\'') && other.ends_with('\''))
                || (other.starts_with('"') && other.ends_with('"')) {
                format!("\"{}\"", &other[1..other.len()-1])
            } else {
                format!("\"{}\"", other.replace('"', "\\\""))
            }
        }
    }
}

// ── Quick-kind for TOML files ──────────────────────────────────────────────────

/// Fast pre-check that reads the `kind = "..."` line from a TOML file without
/// full parsing. Returns `"compiler"` when no explicit kind is set.
pub fn quick_kind_toml(src: &str) -> String {
    for line in src.lines() {
        let t = line.trim();
        if t.starts_with('#') { continue; }
        if let Some(rest) = t.strip_prefix("kind") {
            let rest = rest.trim();
            if let Some(rest) = rest.strip_prefix('=') {
                let v = rest.trim().trim_matches('"').trim_matches('\'');
                if !v.is_empty() { return v.to_string(); }
            }
        }
    }
    "compiler".to_string()
}

// ── Debugger template loader ────────────────────────────────────────────────────

use super::debugger::{DebuggerTemplate, LaunchConfig, DapConfig};

/// Load a debugger TOML template.
pub fn load_debugger_template_toml(src: &str, dir: Option<&Path>) -> Result<DebuggerTemplate, crate::error::FreightError> {
    let raw: TomlToolchain = toml_edit::de::from_str(src)
        .map_err(|e| crate::error::FreightError::TemplateError(format!("TOML parse error: {e}")))?;
    let merged = resolve_bases(raw, dir)?;

    if merged.kind.as_deref() != Some("debugger") {
        return Err(crate::error::FreightError::TemplateError("not a debugger template".into()));
    }

    let binary = merged.binary.clone().unwrap_or_default();
    let ctx = EvalCtx {
        arch:    std::env::consts::ARCH,
        os:      std::env::consts::OS,
        binary:  &binary,
        version: "",
        value:   "",
    };

    let separator = merged.launch.as_ref()
        .and_then(|m| m.get("separator"))
        .cloned()
        .unwrap_or_default();

    let dap_binaries = merged.dap.as_ref().map(|d| d.binaries.clone()).unwrap_or_default();
    let dap_vscode_type = merged.dap.as_ref().map(|d| d.vscode_type.clone()).unwrap_or_default();
    let dap_mi_mode = merged.dap.as_ref().map(|d| d.mi_mode.clone()).unwrap_or_default();

    let settings: HashMap<String, String> = merged.settings.as_ref()
        .map(|m| m.iter().map(|(k, v)| (k.clone(), eval_expr(v, &ctx))).collect())
        .unwrap_or_default();

    Ok(DebuggerTemplate {
        name:          merged.name.clone().unwrap_or_default(),
        binary:        binary.clone(),
        version_arg:   merged.version.as_ref().and_then(|v| v.argument.clone()).unwrap_or_default(),
        version_regex: merged.version.as_ref().and_then(|v| v.regex.clone()).unwrap_or_default(),
        launch: LaunchConfig { separator },
        dap: DapConfig {
            binaries:    dap_binaries,
            vscode_type: dap_vscode_type,
            mi_mode:     dap_mi_mode,
        },
        settings,
        default_args: vec![],
    })
}

// ── Tool template loader (formatter / linter) ─────────────────────────────────

use super::tool::ToolTemplate;

/// Load a formatter or linter TOML template.
pub fn load_tool_toml(src: &str, dir: Option<&Path>) -> Result<ToolTemplate, FreightError> {
    let raw: TomlToolchain = toml_edit::de::from_str(src)
        .map_err(|e| FreightError::TemplateError(format!("TOML parse error: {e}")))?;

    let merged = resolve_bases(raw, dir)?;

    let binary = merged.binary.clone().unwrap_or_default();
    let ctx = EvalCtx {
        arch:    std::env::consts::ARCH,
        os:      std::env::consts::OS,
        binary:  &binary,
        version: "",
        value:   "",
    };

    let kind = merged.kind.clone().unwrap_or_else(|| "formatter".to_string());
    let name = merged.name.clone().unwrap_or_default();
    let family = merged.family.clone().unwrap_or_default();
    let version_arg = merged.version.as_ref().and_then(|v| v.argument.clone()).unwrap_or_default();
    let version_regex = merged.version.as_ref().and_then(|v| v.regex.clone()).unwrap_or_default();
    let extensions = merged.extensions.clone().unwrap_or_default();

    let run: HashMap<String, String> = merged.run.as_ref()
        .map(|m| m.iter().map(|(k, v)| (k.clone(), eval_expr(v, &ctx))).collect())
        .unwrap_or_default();

    let settings: HashMap<String, String> = merged.settings.as_ref()
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();

    let values: HashMap<String, Vec<String>> = merged.values.as_ref()
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();

    Ok(ToolTemplate {
        kind,
        name,
        family,
        binary,
        version_arg,
        version_regex,
        extensions,
        run,
        settings,
        values,
    })
}


// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_expr_no_expansion() {
        let ctx = EvalCtx::default();
        assert_eq!(eval_expr("-Wall", &ctx), "-Wall");
        assert_eq!(eval_expr("", &ctx), "");
    }

    #[test]
    fn eval_expr_arch_variable() {
        let ctx = EvalCtx { arch: "x86_64", os: "linux", ..Default::default() };
        let result = eval_expr("$(arch == 'x86_64' ? '-m64' : arch == 'x86' ? '-m32' : '')", &ctx);
        assert_eq!(result, "-m64");
    }

    #[test]
    fn eval_expr_arch_x86() {
        let ctx = EvalCtx { arch: "x86", os: "linux", ..Default::default() };
        let result = eval_expr("$(arch == 'x86_64' ? '-m64' : arch == 'x86' ? '-m32' : '')", &ctx);
        assert_eq!(result, "-m32");
    }

    #[test]
    fn eval_expr_arch_other() {
        let ctx = EvalCtx { arch: "aarch64", os: "linux", ..Default::default() };
        let result = eval_expr("$(arch == 'x86_64' ? '-m64' : arch == 'x86' ? '-m32' : '')", &ctx);
        assert_eq!(result, "");
    }

    #[test]
    fn eval_expr_binary_replace() {
        let ctx = EvalCtx { binary: "clang++", ..Default::default() };
        let result = eval_expr("$(binary.replace('clang++', 'clang'))", &ctx);
        assert_eq!(result, "clang");
    }

    #[test]
    fn eval_expr_version_comparison() {
        let ctx = EvalCtx { version: "4.0.0", ..Default::default() };
        let result = eval_expr("$(version >= '3.9' ? '-flto=thin' : '-flto')", &ctx);
        assert_eq!(result, "-flto=thin");
    }

    #[test]
    fn eval_expr_version_comparison_old() {
        let ctx = EvalCtx { version: "3.8", ..Default::default() };
        let result = eval_expr("$(version >= '3.9' ? '-flto=thin' : '-flto')", &ctx);
        assert_eq!(result, "-flto");
    }

    #[test]
    fn quick_kind_toml_compiler() {
        assert_eq!(quick_kind_toml("name = \"gcc\""), "compiler");
        assert_eq!(quick_kind_toml("kind = \"compiler\"\nname = \"gcc\""), "compiler");
    }

    #[test]
    fn quick_kind_toml_debugger() {
        assert_eq!(quick_kind_toml("kind = \"debugger\"\nbinary = \"gdb\""), "debugger");
    }

    #[test]
    fn quick_kind_toml_formatter() {
        assert_eq!(quick_kind_toml("kind = \"formatter\""), "formatter");
    }

    #[test]
    fn parse_simple_toml_toolchain() {
        let src = r#"
name = "testcc"
kind = "compiler"
binary = "testcc"
extensions = [".c"]
lto = "-flto"
debug = "-g"

[version]
argument = "--version"
regex = '(\d+\.\d+)'

[structure]
include_dir  = "-I{path}"
define       = "-D{name}"
define_value = "-D{name}={value}"
output       = "-o {path}"
compile_only = "-c"
dep_file     = "-MMD -MF {path}"
target       = ""
sysroot      = ""

[optimization]
"2" = "-O2"
"0" = "-O0"

[warnings]
normal = "-Wall"
none   = ""
"#;
        let result = load_toml_toolchain(src, None).unwrap();
        assert_eq!(result.def.name, "testcc");
        assert_eq!(result.def.flags_lto, "-flto");
        assert_eq!(result.def.flags_debug, "-g");
        assert!(result.def.flags_opt.contains_key("2"));
    }

    #[test]
    fn toml_toolchain_name_required() {
        let src = r#"
binary = "testcc"
extensions = [".c"]

[version]
argument = "--version"
regex = '(\d+)'
"#;
        assert!(load_toml_toolchain(src, None).is_err());
    }

    #[test]
    fn eval_expr_value_interpolation() {
        let ctx = EvalCtx { value: "sm_80", ..Default::default() };
        let result = eval_expr("--gpu-architecture=$(value)", &ctx);
        assert_eq!(result, "--gpu-architecture=sm_80");
    }

    #[test]
    fn merge_maps_overlay_wins() {
        let base = TomlToolchain {
            optimization: Some(IndexMap::from([
                ("2".to_string(), "-O2".to_string()),
                ("0".to_string(), "-O0".to_string()),
            ])),
            warnings: Some(IndexMap::from([
                ("normal".to_string(), "-Wall".to_string()),
            ])),
            ..Default::default()
        };
        let overlay = TomlToolchain {
            optimization: Some(IndexMap::from([
                ("2".to_string(), "-O3".to_string()),  // override key "2"
                ("s".to_string(), "-Os".to_string()),   // new key
            ])),
            ..Default::default()
        };
        let merged = merge(base, overlay);
        let opt = merged.optimization.unwrap();
        assert_eq!(opt.get("2").map(String::as_str), Some("-O3"));
        assert_eq!(opt.get("0").map(String::as_str), Some("-O0"));
        assert_eq!(opt.get("s").map(String::as_str), Some("-Os"));
        // warnings preserved from base
        assert!(merged.warnings.is_some());
    }

    // ── expression evaluator — extended coverage ───────────────────────────

    #[test]
    fn eval_expr_string_concat() {
        let ctx = EvalCtx { binary: "clang++", ..Default::default() };
        let result = eval_expr("$('a' + 'b' + 'c')", &ctx);
        assert_eq!(result, "abc");
    }

    #[test]
    fn eval_expr_concat_with_variable() {
        let ctx = EvalCtx { arch: "x86_64", ..Default::default() };
        let result = eval_expr("$('-m' + arch)", &ctx);
        assert_eq!(result, "-mx86_64");
    }

    #[test]
    fn eval_expr_negation() {
        let ctx = EvalCtx { arch: "x86_64", ..Default::default() };
        assert_eq!(eval_expr("$(!(arch == 'x86_64'))", &ctx), "false");
        let ctx2 = EvalCtx { arch: "aarch64", ..Default::default() };
        assert_eq!(eval_expr("$(!(arch == 'x86_64'))", &ctx2), "true");
    }

    #[test]
    fn eval_expr_logical_and() {
        let ctx = EvalCtx { arch: "x86_64", os: "linux", ..Default::default() };
        let r = eval_expr("$(arch == 'x86_64' && os == 'linux' ? '-m64' : '')", &ctx);
        assert_eq!(r, "-m64");
        let ctx2 = EvalCtx { arch: "x86_64", os: "windows", ..Default::default() };
        let r2 = eval_expr("$(arch == 'x86_64' && os == 'linux' ? '-m64' : '')", &ctx2);
        assert_eq!(r2, "");
    }

    #[test]
    fn eval_expr_logical_or() {
        let ctx = EvalCtx { arch: "x86", ..Default::default() };
        let r = eval_expr("$(arch == 'x86_64' || arch == 'x86' ? 'yes' : 'no')", &ctx);
        assert_eq!(r, "yes");
        let ctx2 = EvalCtx { arch: "aarch64", ..Default::default() };
        let r2 = eval_expr("$(arch == 'x86_64' || arch == 'x86' ? 'yes' : 'no')", &ctx2);
        assert_eq!(r2, "no");
    }

    #[test]
    fn eval_expr_not_equal() {
        let ctx = EvalCtx { arch: "aarch64", ..Default::default() };
        let r = eval_expr("$(arch != 'x86_64' ? '-marm' : '')", &ctx);
        assert_eq!(r, "-marm");
    }

    #[test]
    fn eval_expr_multiple_expansions() {
        let ctx = EvalCtx { arch: "x86_64", os: "linux", ..Default::default() };
        let r = eval_expr("$(arch)-$(os)", &ctx);
        assert_eq!(r, "x86_64-linux");
    }

    #[test]
    fn eval_expr_version_less_than() {
        let ctx = EvalCtx { version: "3.5.0", ..Default::default() };
        assert_eq!(eval_expr("$(version < '3.9' ? 'old' : 'new')", &ctx), "old");
        let ctx2 = EvalCtx { version: "10.0.0", ..Default::default() };
        assert_eq!(eval_expr("$(version < '3.9' ? 'old' : 'new')", &ctx2), "new");
    }

    #[test]
    fn eval_expr_paren_grouping() {
        let ctx = EvalCtx { arch: "x86_64", os: "linux", ..Default::default() };
        // (a || b) && c
        let r = eval_expr("$((arch == 'x86_64' || arch == 'x86') && os == 'linux' ? 'yes' : 'no')", &ctx);
        assert_eq!(r, "yes");
    }

    // ── TOML loading ──────────────────────────────────────────────────────

    #[test]
    fn load_toml_always_flags_filters_empty() {
        let src = r#"
name = "testcc"
binary = "testcc"
extensions = [".c"]
always_flags = ["$(arch == 'x86_64' ? '-m64' : arch == 'x86' ? '-m32' : '')"]
"#;
        let result = load_toml_toolchain(src, None).unwrap();
        // On x86_64 hosts this should have -m64; on aarch64 the empty string is filtered.
        for f in &result.def.always_flags {
            assert!(!f.is_empty(), "empty string should be filtered from always_flags");
        }
    }

    #[test]
    fn load_toml_compiler_option_map() {
        let src = r#"
name = "testcc"
binary = "testcc"
extensions = [".c"]

[compiler.lto_mode]
thin = "-flto=thin"
full = "-flto=full"
"#;
        let result = load_toml_toolchain(src, None).unwrap();
        assert!(result.compiler_option_handlers.contains_key("lto_mode"),
            "lto_mode handler should be registered");
    }

    #[test]
    fn load_toml_language_std_sets_default() {
        let src = r#"
name = "testcc"
binary = "testcc"
extensions = [".cpp"]

[language.std]
"c++17" = "-std=c++17"
"c++20" = "-std=c++20"
"c++23" = "-std=c++23"
"#;
        let result = load_toml_toolchain(src, None).unwrap();
        assert_eq!(result.def.standards.get("c++17").map(String::as_str), Some("-std=c++17"));
        assert_eq!(result.def.defaults.get("std").map(String::as_str), Some("c++17"),
            "first key in [language.std] should become the default");
    }

    #[test]
    fn load_toml_optimization_first_is_default() {
        let src = r#"
name = "testcc"
binary = "testcc"
extensions = [".c"]

[optimization]
"2" = "-O2"
"0" = "-O0"
"3" = "-O3"
"#;
        let result = load_toml_toolchain(src, None).unwrap();
        assert_eq!(result.def.defaults.get("opt_level").map(String::as_str), Some("2"),
            "first key in [optimization] should be the default");
    }

    #[test]
    fn load_toml_stdlib_explicit_empty() {
        let src = r#"
name = "testcc"
binary = "testcc"
extensions = [".f90"]
stdlib = {}
"#;
        let result = load_toml_toolchain(src, None).unwrap();
        assert!(result.def.flags_stdlib.is_empty(), "explicit empty stdlib = {{}} should produce no stdlib flags");
    }

    #[test]
    fn load_toml_structure_fields() {
        let src = r#"
name = "msvctest"
binary = "cl.exe"
extensions = [".cpp"]

[structure]
include_dir   = "/I{path}"
define        = "/D{name}"
define_value  = "/D{name}={value}"
output_obj    = "/Fo{path}"
output_bin    = "/Fe{path}"
compile_only  = "/c"
dep_file      = "/showIncludes"
dep_file_mode = "stdout"
"#;
        let result = load_toml_toolchain(src, None).unwrap();
        assert_eq!(result.def.structure.get("include_dir").map(String::as_str), Some("/I{path}"));
        assert_eq!(result.def.structure.get("dep_file_mode").map(String::as_str), Some("stdout"));
        assert_eq!(result.def.structure.get("output_obj").map(String::as_str), Some("/Fo{path}"));
    }

    #[test]
    fn load_toml_linking_fields() {
        let src = r#"
name = "testcc"
binary = "testcc"
extensions = [".cpp"]

[linking.cpp]
abi        = "c++"
compatible = ["c", "fortran"]
linker     = ""
extensions = [".cpp", ".cc"]
"#;
        let result = load_toml_toolchain(src, None).unwrap();
        let cpp_link = result.def.linking.iter().find(|(k, _)| k == "cpp");
        assert!(cpp_link.is_some(), "linking.cpp should be present");
        let (_, lp) = cpp_link.unwrap();
        assert_eq!(lp.abi, "c++");
        assert!(lp.compatible.contains(&"c".to_string()));
    }

    #[test]
    fn load_debugger_toml_gdb() {
        let src = r#"
kind          = "debugger"
name          = "gdb"
binary        = "gdb"
version_arg   = "--version"
version_regex = 'GNU gdb[^\d]+(\d+\.\d+)'

[launch]
separator = "--args"

[dap]
binaries    = []
vscode_type = "cppdbg"
mi_mode     = "gdb"

[settings]
tui   = "--tui"
quiet = "-q"
"#;
        let result = load_debugger_template_toml(src, None).unwrap();
        assert_eq!(result.name, "gdb");
        assert_eq!(result.launch.separator, "--args");
        assert_eq!(result.dap.vscode_type, "cppdbg");
        assert_eq!(result.settings.get("tui").map(String::as_str), Some("--tui"));
    }

    #[test]
    fn load_tool_toml_formatter() {
        let src = r#"
kind          = "formatter"
name          = "clang-format"
family        = "llvm"
binary        = "clang-format"
version_arg   = "--version"
version_regex = 'clang-format version (\d+\.\d+\.\d+)'
extensions    = [".cpp", ".c", ".h"]

[run]
fix   = "-i"
check = "--dry-run --Werror"

[settings]
style  = "--style={value}"
config = "--style=file:{value}"

[values]
style = ["Google", "LLVM", "Mozilla"]
"#;
        let result = load_tool_toml(src, None).unwrap();
        assert_eq!(result.name, "clang-format");
        assert_eq!(result.kind, "formatter");
        assert_eq!(result.run.get("fix").map(String::as_str), Some("-i"));
        assert_eq!(result.run.get("check").map(String::as_str), Some("--dry-run --Werror"));
        assert!(result.values.get("style").map(|v| v.contains(&"Google".to_string())).unwrap_or(false));
    }

    #[test]
    fn load_tool_toml_linter() {
        let src = r#"
kind          = "linter"
name          = "cppcheck"
binary        = "cppcheck"
version_arg   = "--version"
version_regex = 'Cppcheck (\d+\.\d+)'
extensions    = [".cpp", ".c"]

[run]
check = "--error-exitcode=1"
fix   = "--error-exitcode=1"

[settings]
enable = "--enable={value}"
"#;
        let result = load_tool_toml(src, None).unwrap();
        assert_eq!(result.name, "cppcheck");
        assert_eq!(result.kind, "linter");
        assert_eq!(result.run.get("check").map(String::as_str), Some("--error-exitcode=1"));
    }

    // ── Real TOML file loading ────────────────────────────────────────────

    fn toolchains_dir() -> std::path::PathBuf {
        // From the crate root, toolchains/ is two levels up (crates/freight-core → repo root).
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../toolchains")
    }

    fn load_real(rel: &str) -> EvalResult {
        let path = toolchains_dir().join(rel);
        let src = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let dir = path.parent();
        load_toml_toolchain(&src, dir).unwrap_or_else(|e| panic!("load failed for {rel}: {e}"))
    }

    #[test]
    fn real_clangpp_loads() {
        let r = load_real("llvm/clang++.toml");
        assert_eq!(r.def.name, "clang++");
        assert_eq!(r.def.family, "llvm");
        // Standards from _cpp.toml base
        assert!(r.def.standards.contains_key("c++17"));
        assert!(r.def.standards.contains_key("c++20"));
        // C++ extensions from base
        assert!(r.def.extensions.iter().any(|e| e == ".cpp"));
        // lto_mode compiler option
        assert!(r.compiler_option_handlers.contains_key("lto_mode"));
        // Toolset
        assert!(r.def.toolset.contains_key("ar"));
    }

    #[test]
    fn real_gcc_loads() {
        let r = load_real("gnu/gcc.toml");
        assert_eq!(r.def.name, "gcc");
        assert_eq!(r.def.family, "gnu");
        assert!(!r.def.flags_warnings.is_empty());
        assert!(r.def.extensions.iter().any(|e| e == ".c"));
        // linking.c from the file
        assert!(r.def.linking.iter().any(|(k, _)| k == "c"));
    }

    #[test]
    fn real_gfortran_loads() {
        let r = load_real("gnu/gfortran.toml");
        assert_eq!(r.def.name, "gfortran");
        // Fortran extensions from _fortran.toml base
        assert!(r.def.extensions.iter().any(|e| e == ".f90"));
        // Fortran standards
        assert!(r.def.standards.contains_key("f2018"));
        // dep_file override from gfortran.toml
        assert!(r.def.structure.get("dep_file").map(|s| s.contains("-cpp")).unwrap_or(false));
    }

    #[test]
    fn real_nvcc_loads() {
        let r = load_real("nvidia/nvcc.toml");
        assert_eq!(r.def.name, "nvcc");
        assert!(r.def.always_flags.contains(&"--expt-relaxed-constexpr".to_string()));
        assert!(r.def.always_flags.contains(&"--extended-lambda".to_string()));
        assert!(r.compiler_option_handlers.contains_key("sm_arch"));
        assert!(r.def.requires_toolchain.contains(&"cpp".to_string()));
        assert_eq!(r.def.required_tools, vec!["ptxas", "fatbinary"]);
    }

    #[test]
    fn real_msvc_loads() {
        let r = load_real("msvc.toml");
        assert_eq!(r.def.name, "msvc");
        assert_eq!(r.def.binary, "cl.exe");
        assert_eq!(r.def.flags_lto, "/GL");
        assert_eq!(r.def.flags_lto_link, "/LTCG");
        assert_eq!(r.def.structure.get("dep_file_mode").map(String::as_str), Some("stdout"));
        assert_eq!(r.def.structure.get("output_obj").map(String::as_str), Some("/Fo{path}"));
    }

    #[test]
    fn real_nasm_loads() {
        let r = load_real("asm/nasm.toml");
        assert_eq!(r.def.name, "nasm");
        // arch_flags from _asm-base.toml
        assert!(r.def.arch_flags.contains_key("x86_64.linux"));
        assert_eq!(r.def.arch_flags.get("x86_64.linux").map(String::as_str), Some("-f elf64"));
    }

    #[test]
    fn real_tcc_loads() {
        let r = load_real("tcc.toml");
        assert_eq!(r.def.name, "tcc");
        assert!(r.def.flags_lto.is_empty());
        assert!(r.def.linking.iter().any(|(k, _)| k == "c"));
    }

    #[test]
    fn real_dmd_loads() {
        let r = load_real("dmd.toml");
        assert_eq!(r.def.name, "dmd");
        assert!(r.compiler_option_handlers.contains_key("dip1000"));
    }

    #[test]
    fn real_clang_format_loads() {
        let path = toolchains_dir().join("llvm/clang-format.toml");
        let src = std::fs::read_to_string(&path).unwrap();
        let dir = path.parent();
        let r = load_tool_toml(&src, dir).unwrap();
        assert_eq!(r.name, "clang-format");
        assert_eq!(r.kind, "formatter");
        assert!(r.values.get("style").map(|v| !v.is_empty()).unwrap_or(false));
    }

    #[test]
    fn real_gdb_loads() {
        let path = toolchains_dir().join("gnu/gdb.toml");
        let src = std::fs::read_to_string(&path).unwrap();
        let dir = path.parent();
        let r = load_debugger_template_toml(&src, dir).unwrap();
        assert_eq!(r.name, "gdb");
        assert_eq!(r.launch.separator, "--args");
        assert_eq!(r.dap.mi_mode, "gdb");
    }

    #[test]
    fn real_lldb_loads() {
        let path = toolchains_dir().join("llvm/lldb.toml");
        let src = std::fs::read_to_string(&path).unwrap();
        let dir = path.parent();
        let r = load_debugger_template_toml(&src, dir).unwrap();
        assert_eq!(r.name, "lldb");
        assert_eq!(r.launch.separator, "--");
        assert!(r.dap.binaries.contains(&"lldb-dap".to_string()));
    }
}
