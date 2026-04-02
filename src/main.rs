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
    Add {
        trigger: String,
        expansion: String,
    },
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
            // Daemon mode: run the tokio event loop
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?
                .block_on(async {
                    let config = config::Config::load(&config::Config::path())?;
                    daemon::run(config).await
                })
        }
        Some(cmd) => handle_cmd(cmd),
    }
}

fn handle_cmd(cmd: Cmd) -> anyhow::Result<()> {
    match cmd {
        Cmd::List => {
            let cfg = config::Config::load(&config::Config::path())?;
            if cfg.expansions.is_empty() {
                println!("No expansions configured.");
                println!("Add one with: srkt add /trigger \"expansion text\"");
            } else {
                let mut pairs: Vec<_> = cfg.expansions.iter().collect();
                pairs.sort_by_key(|(k, _)| k.as_str());
                for (trigger, expansion) in pairs {
                    println!("{:<20} => {}", trigger, expansion.replace('\n', "\\n"));
                }
            }
        }

        Cmd::Add { trigger, expansion } => {
            let path = config::Config::path();
            let mut cfg = config::Config::load(&path)?;
            let expansion = expansion.replace("\\n", "\n");
            cfg.add(&trigger, &expansion)?;
            cfg.save(&path)?;
            println!("Added: {} => {}", trigger, expansion.replace('\n', "\\n"));
            // Signal daemon to reload if running
            let _ = signal_daemon_reload();
        }

        Cmd::Remove { trigger } => {
            let path = config::Config::path();
            let mut cfg = config::Config::load(&path)?;
            if cfg.remove(&trigger) {
                cfg.save(&path)?;
                println!("Removed: {}", trigger);
                let _ = signal_daemon_reload();
            } else {
                anyhow::bail!("Trigger '{}' not found", trigger);
            }
        }

        Cmd::Reload => {
            let sock = ipc::socket_path()?;
            if !sock.exists() {
                anyhow::bail!("srkt daemon is not running");
            }
            signal_daemon_reload().map_err(|e| {
                anyhow::anyhow!("Could not reach daemon: {}", e)
            })?;
            println!("Reloaded");
        }

        Cmd::Status => {
            let sock = ipc::socket_path()?;
            if sock.exists() {
                println!("srkt daemon is running (socket: {})", sock.display());
            } else {
                println!("srkt daemon is NOT running");
                std::process::exit(1);
            }
        }

        Cmd::Install => {
            install_service()?;
        }
    }
    Ok(())
}

fn signal_daemon_reload() -> anyhow::Result<()> {
    let sock = ipc::socket_path()?;
    if !sock.exists() {
        return Ok(()); // Daemon not running, config will be read on next start
    }
    use std::io::Write;
    let mut stream = std::os::unix::net::UnixStream::connect(&sock)?;
    stream.write_all(b"reload\n")?;
    let _ = stream.shutdown(std::net::Shutdown::Write);
    Ok(())
}

fn install_service() -> anyhow::Result<()> {
    // Find the binary path
    let binary = std::env::current_exe()?;

    // Build service file content
    let service = format!(
        r#"[Unit]
Description=srkt Wayland text expander
Documentation=https://github.com/jakub/srkt
After=graphical-session.target

[Service]
Type=simple
ExecStart={}
Restart=on-failure
RestartSec=2s
StandardOutput=journal
StandardError=journal
ImportEnvironment=WAYLAND_DISPLAY XDG_RUNTIME_DIR

[Install]
WantedBy=graphical-session.target
"#,
        binary.display()
    );

    // Write service file
    let service_dir = {
        let config_home = match std::env::var("XDG_CONFIG_HOME") {
            Ok(v) => std::path::PathBuf::from(v),
            Err(_) => {
                let home = std::env::var("HOME")
                    .map_err(|_| anyhow::anyhow!("HOME environment variable is not set"))?;
                std::path::PathBuf::from(home).join(".config")
            }
        };
        config_home.join("systemd").join("user")
    };
    std::fs::create_dir_all(&service_dir)?;
    let service_path = service_dir.join("srkt.service");
    std::fs::write(&service_path, &service)?;
    println!("Wrote: {:?}", service_path);

    // Enable and start the service
    let status = std::process::Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status()?;
    if !status.success() {
        anyhow::bail!("systemctl daemon-reload failed");
    }

    let status = std::process::Command::new("systemctl")
        .args(["--user", "enable", "--now", "srkt"])
        .status()?;
    if !status.success() {
        anyhow::bail!("systemctl enable --now srkt failed");
    }

    println!("srkt service installed and started.");
    println!();
    println!("IMPORTANT: You still need to join the 'input' group (requires re-login):");
    println!("  sudo usermod -a -G input $USER");
    println!("  # Then log out and back in");
    Ok(())
}
