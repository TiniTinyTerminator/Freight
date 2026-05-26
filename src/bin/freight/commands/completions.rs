use crate::completion::{print_completion, CompletionShell};

#[derive(clap::Args)]
pub struct Args {
    /// Shell to generate completions for (bash, elvish, fish, powershell, zsh)
    pub shell: CompletionShell,
}

impl Args {
    pub fn run(self, cmd: &mut clap::Command) {
        print_completion(self.shell, cmd);
    }
}
