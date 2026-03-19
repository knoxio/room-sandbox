use anyhow::{Result, bail};
use inquire::Confirm;

use crate::config::{self, Config};
use crate::docker;

pub fn run() -> Result<()> {
    if !config::sandbox_dir().exists() {
        bail!("nothing to clean — .room-sandbox/ does not exist");
    }

    let config = Config::load().ok();

    eprintln!("This will remove:");
    eprintln!("  .room-sandbox/     (workspaces, Docker assets, state, .env)");

    if let Some(ref config) = config {
        if docker::is_running(config) {
            eprintln!("  container '{}'  (will be stopped and removed)", config.project.container_name);
        }
    }

    eprintln!("\n  sandbox.toml will be kept.");

    let confirm = Confirm::new("Proceed?")
        .with_default(false)
        .prompt()?;

    if !confirm {
        eprintln!("Aborted.");
        return Ok(());
    }

    // Stop container if running
    if let Some(ref config) = config {
        if docker::is_running(config) {
            eprintln!("Stopping container...");
            docker::down()?;
        }
    }

    eprintln!("Removing .room-sandbox/...");
    std::fs::remove_dir_all(config::sandbox_dir())?;

    eprintln!("Clean complete. Run `room-sandbox apply` to rebuild.");

    Ok(())
}
