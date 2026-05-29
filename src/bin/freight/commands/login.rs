use freight_core::registry::DEFAULT_REGISTRY_URL;
use freight_core::toolchain::cache::{freight_home, GlobalConfig};

use crate::output::{print_error, print_success};

#[derive(clap::Args)]
pub struct Args {
    /// Registry base URL (default: first configured registry or https://freight.dev)
    #[arg(long, value_name = "URL")]
    pub registry: Option<String>,
    /// API token — skip username/password and save this token directly
    #[arg(long, value_name = "TOKEN")]
    pub token: Option<String>,
    /// Username (skips the TUI and calls the login API directly when combined with --password)
    #[arg(long, value_name = "NAME")]
    pub username: Option<String>,
    /// Password (skips the TUI when combined with --username)
    #[arg(long, value_name = "PASS")]
    pub password: Option<String>,
    /// Skip the interactive TUI and use plain CLI prompts instead
    #[arg(long)]
    pub notui: bool,
}

impl Args {
    pub fn run(self) {
        if self.token.is_some() || self.notui {
            cmd_login(self.registry.as_deref(), self.token.as_deref());
        } else if self.username.is_some() || self.password.is_some() {
            super::common::login_with_credentials(
                self.registry.as_deref(),
                self.username.as_deref(),
                self.password.as_deref(),
            );
        } else {
            let url = super::common::resolve_registry_url(self.registry.as_deref());
            match crate::tui::login::run(url.clone(), None) {
                Ok((uname, token)) => {
                    let name = super::common::registry_name_for(&url);
                    match freight_core::toolchain::cache::GlobalConfig::save_credential(
                        &url, &name, &token,
                    ) {
                        Ok(()) => crate::output::print_success(&format!(
                            "logged in as `{uname}` — token saved to ~/.freight/credentials.toml"
                        )),
                        Err(e) => {
                            crate::output::print_error(&e.to_string());
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) if e.to_string() == "cancelled" => {}
                Err(e) => {
                    crate::output::print_error(&e.to_string());
                    std::process::exit(1);
                }
            }
        }
    }
}

fn cmd_login(registry_url: Option<&str>, token_arg: Option<&str>) {
    let config = GlobalConfig::load();

    let url = registry_url
        .map(str::to_string)
        .or_else(|| config.registries.first().map(|r| r.url.clone()))
        .unwrap_or_else(|| DEFAULT_REGISTRY_URL.to_string());

    let name = config
        .registries
        .iter()
        .find(|r| r.url == url)
        .map(|r| r.name.clone())
        .unwrap_or_else(|| "freight".to_string());

    let token = match token_arg {
        Some(t) => t.to_string(),
        None => {
            use std::io::{self, Write};
            print!("Token for {url}: ");
            io::stdout().flush().ok();
            let mut t = String::new();
            io::stdin().read_line(&mut t).ok();
            t.trim().to_string()
        }
    };

    if token.is_empty() {
        print_error("token cannot be empty");
        std::process::exit(1);
    }

    match GlobalConfig::save_credential(&url, &name, &token) {
        Ok(()) => {
            let creds_path = freight_home()
                .map(|h| h.join("credentials.toml").to_string_lossy().into_owned())
                .unwrap_or_else(|| "~/.freight/credentials.toml".into());
            print_success(&format!("token saved to {creds_path}"));
        }
        Err(e) => {
            print_error(&e.to_string());
            std::process::exit(1);
        }
    }
}
