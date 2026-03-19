use anyhow::{Result, bail};

use crate::config::{self, Config};
use crate::docker;
use crate::state;

pub fn run(name: &str, claude_args: &[String]) -> Result<()> {
    let config = Config::load()?;

    if !config.has_agent(name) {
        bail!(
            "agent '{name}' not found. Available: {}",
            config.agent_names_joined()
        );
    }

    if !config::agent_workspace(name).exists() {
        bail!("agent '{name}' workspace missing — run `room-sandbox apply`");
    }

    let _ = state::warn_drift();

    docker::ensure_running(&config)?;
    docker::run_claude(&config, name, claude_args)
}
