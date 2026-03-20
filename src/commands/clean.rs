use anyhow::{Result, bail};
use inquire::Confirm;
use std::process::Command;

use crate::config::{self, Config};

pub fn run() -> Result<()> {
    if !config::sandbox_dir().exists() {
        bail!("nothing to clean — .room-sandbox/ does not exist");
    }

    let config = Config::load().ok();

    eprintln!("This will remove:");
    eprintln!("  .room-sandbox/     (workspaces, Docker assets, state, .env)");
    eprintln!("  Docker volumes     (room data, claude data, cargo cache)");

    if let Some(ref config) = config {
        eprintln!(
            "  container '{}'  (will be stopped and removed)",
            config.project.container_name
        );
    }

    eprintln!("\n  sandbox.toml will be kept.");

    let confirm = Confirm::new("Proceed?").with_default(false).prompt()?;

    if !confirm {
        eprintln!("Aborted.");
        return Ok(());
    }

    if let Some(ref config) = config {
        let container = &config.project.container_name;

        // Force remove container (works whether running, stopped, or absent)
        eprintln!("Removing container...");
        let _ = Command::new("docker")
            .args(["rm", "-f", container])
            .output();

        // Remove associated Docker volumes
        eprintln!("Removing Docker volumes...");
        let project = container;
        for vol in ["claude-data", "cargo-cache", "room-data"] {
            let vol_name = format!("{project}_{vol}");
            let _ = Command::new("docker")
                .args(["volume", "rm", "-f", &vol_name])
                .output();
        }

        // Remove the compose network
        let network = format!("{project}_default");
        let _ = Command::new("docker")
            .args(["network", "rm", &network])
            .output();
    }

    eprintln!("Removing .room-sandbox/...");
    std::fs::remove_dir_all(config::sandbox_dir())?;

    eprintln!("Clean complete. Run `room-sandbox apply` to rebuild.");

    Ok(())
}
