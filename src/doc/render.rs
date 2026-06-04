use std::path::Path;

use crate::doc::docify::extract::DocSet;
use crate::doc::docify::render;

/// Generate Markdown documentation for `set` into `out_dir`.
pub fn generate(set: DocSet, out_dir: &Path) -> anyhow::Result<()> {
    render(&set, out_dir).map_err(anyhow::Error::from)
}
