mod config;
mod daemon;
mod expander;
mod injector;
mod ipc;
mod keyboard;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "srkt", about = "Wayland-native text expander", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Add or overwrite an expansion (use \\n for newlines)
    Add { trigger: String, expansion: String },
    /// Remove an expansion
    Remove { trigger: String },
    /// List all expansions
    List,
    /// Signal running daemon to reload config
    Reload,
    /// Show daemon status
    Status,
    /// Install and enable systemd user service
    Install,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        None => {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?
                .block_on(async {
                    let config = config::Config::load(&config::Config::path())?;
                    daemon::run(config).await
                })
        }
        Some(_cmd) => {
            eprintln!("CLI subcommands not yet implemented");
            Ok(())
        }
    }
}
