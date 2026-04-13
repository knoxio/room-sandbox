use anyhow::{Result, bail};

use crate::config::{self, Config};
use crate::docker;

pub fn run() -> Result<()> {
    if !config::sandbox_dir().exists() {
        bail!("not initialized — run `room-sandbox init` first");
    }

    let config = Config::load()?;

    eprintln!("Rebuilding container with --no-cache to pull latest packages...");
    eprintln!("This will update room, room-ralph, Claude Code, and all utilities.");
    eprintln!();

    docker::write_assets(&config)?;
    docker::build_no_cache()?;

    // Restart container with new image
    if docker::is_running(&config) {
        eprintln!("Restarting container...");
        docker::down()?;
    }
    docker::up()?;

    // Re-inject instructions and symlinks
    docker::ensure_workspace_symlinks(&config)?;
    docker::inject_agent_instructions(&config)?;

    eprintln!("\nUpgrade complete.");

    Ok(())
}
