//! Shell completions module for Rustible
//!
//! Provides shell completion scripts for bash, zsh, fish, powershell, and elvish.

use clap::CommandFactory;
use clap_complete::{generate, Shell};
use std::io;

use crate::cli::Cli;

/// Generate shell completions and write to stdout
pub fn generate_completions(shell: Shell) {
    let mut cmd = Cli::command();
    generate(shell, &mut cmd, "rustible", &mut io::stdout());
}

/// Get completions as a string
pub fn get_completions(shell: Shell) -> String {
    let mut cmd = Cli::command();
    let mut buf = Vec::new();
    generate(shell, &mut cmd, "rustible", &mut buf);
    String::from_utf8(buf).unwrap_or_default()
}

/// Print installation instructions for completions
pub fn print_installation_instructions(shell: Shell) {
    match shell {
        Shell::Bash => {
            println!("# Bash completion installation:");
            println!("# Add the following to your ~/.bashrc or ~/.bash_profile:");
            println!();
            println!("# Option 1: Direct sourcing");
            println!("eval \"$(rustible completions bash)\"");
            println!();
            println!("# Option 2: Save to file");
            println!(
                "rustible completions bash > ~/.local/share/bash-completion/completions/rustible"
            );
            println!();
            println!("# Or for system-wide installation:");
            println!("sudo rustible completions bash > /etc/bash_completion.d/rustible");
        }
        Shell::Zsh => {
            println!("# Zsh completion installation:");
            println!("# Add the following to your ~/.zshrc:");
            println!();
            println!("# Option 1: Direct sourcing");
            println!("eval \"$(rustible completions zsh)\"");
            println!();
            println!("# Option 2: Save to fpath directory");
            println!("# First, ensure you have a completions directory in your fpath:");
            println!("mkdir -p ~/.zsh/completions");
            println!("echo 'fpath=(~/.zsh/completions $fpath)' >> ~/.zshrc");
            println!("echo 'autoload -Uz compinit && compinit' >> ~/.zshrc");
            println!();
            println!("# Then generate the completions:");
            println!("rustible completions zsh > ~/.zsh/completions/_rustible");
        }
        Shell::Fish => {
            println!("# Fish completion installation:");
            println!("# Save to the fish completions directory:");
            println!();
            println!("rustible completions fish > ~/.config/fish/completions/rustible.fish");
            println!();
            println!("# Or for system-wide installation:");
            println!("sudo rustible completions fish > /usr/share/fish/vendor_completions.d/rustible.fish");
        }
        Shell::PowerShell => {
            println!("# PowerShell completion installation:");
            println!("# Add the following to your PowerShell profile:");
            println!("# ($PROFILE or $PROFILE.CurrentUserAllHosts)");
            println!();
            println!("Invoke-Expression (& rustible completions powershell | Out-String)");
        }
        Shell::Elvish => {
            println!("# Elvish completion installation:");
            println!("# Add the following to ~/.elvish/rc.elv:");
            println!();
            println!("eval (rustible completions elvish | slurp)");
        }
        _ => {
            println!("# Unknown shell. Please refer to your shell's documentation for completion installation.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bash_completions() {
        let completions = get_completions(Shell::Bash);
        assert!(completions.contains("rustible"));
        assert!(completions.contains("complete"));
    }

    #[test]
    fn test_zsh_completions() {
        let completions = get_completions(Shell::Zsh);
        assert!(completions.contains("rustible"));
        assert!(completions.contains("compdef") || completions.contains("_rustible"));
    }

    #[test]
    fn test_fish_completions() {
        let completions = get_completions(Shell::Fish);
        assert!(completions.contains("rustible"));
        assert!(completions.contains("complete"));
    }
}
