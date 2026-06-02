use std::path::PathBuf;

/// A dependency whose source tree can be extracted for documentation.
#[derive(Debug, Clone)]
pub struct DocDependency {
    pub name: String,
    /// "local" | "local-dev" | "global"
    pub scope: &'static str,
    /// "path" | "git" | "url" | "registry" | "cached" | "platform"
    pub kind: String,
    pub version: String,
    pub source: String,
    /// Local directory where the dep's sources live, if available.
    pub path: Option<PathBuf>,
    /// Pre-built doc files found inside `path` (index.md, README.md, …).
    pub docs: Vec<PathBuf>,
}
