use freight_core::registry::repos::{registries_in_order, repo_by_name};
use freight_core::toolchain::cache::GlobalConfig;

use crate::output::{print_error, print_status, print_warning};
use owo_colors::OwoColorize;

#[derive(clap::Args)]
pub struct Args {
    pub query: String,
    /// Registry to search (default: all configured registries in order)
    #[arg(long, value_name = "NAME")]
    pub repo: Option<String>,
}

impl Args {
    pub fn run(self) {
        cmd_search(&self.query, self.repo.as_deref());
    }
}

fn cmd_search(query: &str, repo: Option<&str>) {
    let config = {
        let mut cfg = GlobalConfig::load();
        let cwd = std::env::current_dir().unwrap_or_default();
        if let Some(proj) = freight_core::manifest::find_manifest_dir(&cwd) {
            if let Some(local) = GlobalConfig::load_local(&proj) {
                cfg.apply_local(local);
            }
        }
        cfg
    };

    let repos: Vec<Box<dyn freight_core::registry::PackageRepo>> = if let Some(rname) = repo {
        match repo_by_name(rname, &config) {
            Ok(r) => vec![r],
            Err(e) => {
                print_error(&e.to_string());
                return;
            }
        }
    } else {
        registries_in_order(&config)
    };

    let mut any = false;
    for r in &repos {
        let label = if r.repo_key().is_empty() {
            "freight.dev"
        } else {
            r.repo_key()
        };
        match r.search(query) {
            Ok(results) if !results.is_empty() => {
                if !any {
                    println!(
                        "{:<32}  {:<12}  {}",
                        "name".bold(),
                        "latest".bold(),
                        "description".bold()
                    );
                    println!("{}", "─".repeat(72).bright_black());
                }
                for pkg in &results {
                    println!(
                        "{:<32}  {:<12}  {}",
                        pkg.name.bright_blue(),
                        pkg.latest.bright_black(),
                        pkg.description.as_deref().unwrap_or("").dimmed()
                    );
                }
                any = true;
            }
            Ok(_) => {
                print_status(label, &format!("no results for `{query}`"));
            }
            Err(e) => {
                print_warning(&format!("{label}: {e}"));
            }
        }
    }

    if !any {
        println!("no packages found matching `{query}`");
    }
}
