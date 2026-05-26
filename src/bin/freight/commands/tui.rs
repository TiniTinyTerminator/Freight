#[derive(clap::Args)]
pub struct Args {
    /// Registry base URL (default: http://localhost:7878 or configured registry)
    #[arg(long, env = "FREIGHT_REGISTRY_URL", default_value = "http://localhost:7878")]
    pub url: String,
    /// API token — omit to use saved credentials or the interactive login screen
    #[arg(long, env = "FREIGHT_REGISTRY_TOKEN")]
    pub token: Option<String>,
}

impl Args {
    pub fn run(self) {
        if let Err(e) = crate::tui::registry::run(self.url, self.token) {
            eprintln!("error: {e:#}");
            std::process::exit(1);
        }
    }
}
