use anyhow::{Result, bail};

use crate::config::{self, Config};
use crate::docker;
use crate::state;

pub fn run(name: Option<&str>, root: bool) -> Result<()> {
    let config = Config::load()?;

    // Check drift — error if the named agent is missing due to drift
    let drift = state::check_state()?;

    if let Some(agent_name) = name {
        if !config.has_agent(agent_name) {
            bail!(
                "agent '{agent_name}' not found. Available: {}",
                config.agent_names_joined()
            );
        }
        if !config::agent_workspace(agent_name).exists() {
            if drift.agent_missing(agent_name) {
                bail!(
                    "agent '{agent_name}' was added but not yet cloned — run `room-sandbox apply`"
                );
            }
            bail!("agent '{agent_name}' workspace not found");
        }
    }

    if !drift.is_empty() {
        eprintln!("warning: sandbox.toml has unapplied changes — run `room-sandbox apply`");
    }

    // Ensure container is running
    docker::ensure_running(&config)?;

    let user = if root { "root" } else { "agent" };
    let workdir = match name {
        Some(agent_name) => format!("/workspaces/{agent_name}"),
        None => "/workspaces".to_string(),
    };

    docker::exec(&config, user, &workdir, &["bash"])
}
