use std::path::Path;

use docify::extract::DocSet;
use docify::render;

/// Generate Markdown documentation for `set` into `out_dir`.
pub fn generate(set: DocSet, out_dir: &Path) -> anyhow::Result<()> {
    render(&set, out_dir).map_err(anyhow::Error::from)
}
