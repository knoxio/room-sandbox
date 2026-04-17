use anyhow::{Result, bail};
use inquire::{Confirm, Select};

use crate::config::{self, AgentDef, AgentRole, Config};
use crate::docker;
use crate::state;

pub fn add(name: &str) -> Result<()> {
    config::validate_agent_name(name)?;
    let mut config = Config::load()?;

    if config.has_agent(name) {
        bail!("agent '{name}' already exists");
    }

    let roles = vec!["coder", "reviewer", "manager"];
    let role_selection = Select::new(&format!("Role for '{name}':"), roles).prompt()?;
    let role = match role_selection {
        "reviewer" => AgentRole::Reviewer,
        "manager" => AgentRole::Manager,
        _ => AgentRole::Coder,
    };

    config.agents.push(AgentDef {
        name: name.to_string(),
        role,
    });
    config.save()?;

    eprintln!("Added agent '{name}' ({role}) to sandbox.toml");

    // Auto-apply (handles drift confirmation)
    crate::commands::apply::apply_with_config(&config)?;

    Ok(())
}

pub fn remove(name: &str) -> Result<()> {
    let mut config = Config::load()?;

    if !config.has_agent(name) {
        bail!(
            "agent '{name}' not found. Available: {}",
            config.agent_names_joined()
        );
    }

    let confirm = Confirm::new(&format!(
        "Remove agent '{name}'? This will delete its workspace."
    ))
    .with_default(false)
    .prompt()?;

    if !confirm {
        eprintln!("Aborted.");
        return Ok(());
    }

    config.agents.retain(|a| a.name != name);
    config.save()?;

    // Directly remove workspace if it exists
    let workspace = config::agent_workspace(name);
    if workspace.exists() {
        eprintln!("Removing workspace...");
        std::fs::remove_dir_all(workspace)?;
    }

    eprintln!("Removed agent '{name}'");

    // Auto-apply (handles any other pending drift)
    crate::commands::apply::apply_with_config(&config)?;

    Ok(())
}

pub fn list() -> Result<()> {
    let config = Config::load()?;

    // Warn about drift but don't block
    let _ = state::warn_drift();

    let container_running = docker::is_running(&config);

    println!("{:<16} {:<12} STATUS", "NAME", "ROLE");
    for agent in &config.agents {
        let name = &agent.name;
        let workspace_exists = config::agent_workspace(name).exists();
        let status = if !workspace_exists {
            "missing   (run apply)".to_string()
        } else if container_running && docker::is_agent_running(&config, name) {
            let pid = get_agent_pid(&config, name).unwrap_or_default();
            if pid.is_empty() {
                "running".to_string()
            } else {
                format!("running   (pid {pid})")
            }
        } else {
            "ready".to_string()
        };
        println!("{:<16} {:<12} {status}", name, agent.role);
    }

    Ok(())
}

pub fn start(names: &[String], all: bool, tail: bool, ralph_args: &[String]) -> Result<()> {
    let config = Config::load()?;

    let names = resolve_names(&config, names, all)?;

    // Check drift — apply if needed
    let drift = state::check_state()?;
    if !drift.is_empty() {
        eprintln!("Unapplied changes detected.\n{drift}");
        crate::commands::apply::apply_with_config(&config)?;
    }

    // Validate all agents
    for name in &names {
        if !config.has_agent(name) {
            bail!(
                "agent '{name}' not found. Available: {}",
                config.agent_names_joined()
            );
        }
        if !config::agent_workspace(name).exists() {
            bail!("agent '{name}' workspace missing — run `room-sandbox apply`");
        }
    }

    // Ensure container is running
    docker::ensure_running(&config)?;

    // Filter out already running agents
    let names: Vec<String> = names
        .into_iter()
        .filter(|name| {
            if docker::is_agent_running(&config, name) {
                eprintln!("  {name} already running — skipping");
                false
            } else {
                true
            }
        })
        .collect();

    if names.is_empty() {
        eprintln!("All agents already running.");
        return Ok(());
    }

    // Ensure room daemon + room exist
    docker::ensure_room(&config)?;

    // Ensure personality files exist for agents being started
    docker::inject_instructions_for(&config, &names)?;

    if tail {
        docker::start_agents_tailed(&config, &names, ralph_args)?;
    } else {
        docker::start_agents_background(&config, &names, ralph_args)?;
    }

    Ok(())
}

pub fn stop(names: &[String], all: bool) -> Result<()> {
    let config = Config::load()?;

    let names = resolve_names(&config, names, all)?;

    for name in &names {
        if !config.has_agent(name) {
            bail!(
                "agent '{name}' not found. Available: {}",
                config.agent_names_joined()
            );
        }

        if !docker::is_agent_running(&config, name) {
            eprintln!("agent '{name}' is not running");
            continue;
        }

        eprintln!("Stopping agent '{name}'...");
        docker::kill_agent(&config, name)?;
    }

    Ok(())
}

pub fn restart(names: &[String], all: bool, tail: bool, ralph_args: &[String]) -> Result<()> {
    let config = Config::load()?;

    let names = resolve_names(&config, names, all)?;

    for name in &names {
        if docker::is_agent_running(&config, name) {
            eprintln!("Stopping {name}...");
            docker::kill_agent(&config, name)?;

            // Poll until the process is actually gone (up to 15s)
            let mut alive = true;
            for _ in 0..30 {
                if !docker::is_agent_running(&config, name) {
                    alive = false;
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
            if alive {
                bail!("agent '{name}' still running after kill — check manually");
            }
        }
    }

    // Wait for processes to fully die before restarting
    std::thread::sleep(std::time::Duration::from_secs(3));

    start(&names, false, tail, ralph_args)
}

/// Resolve agent names: use --all to select all agents, or validate provided names.
fn resolve_names(config: &Config, names: &[String], all: bool) -> Result<Vec<String>> {
    if all {
        return Ok(config
            .agent_names()
            .into_iter()
            .map(|s| s.to_string())
            .collect());
    }
    if names.is_empty() {
        bail!("provide at least one agent name, or use --all");
    }
    Ok(names.to_vec())
}

fn get_agent_pid(config: &Config, name: &str) -> Option<String> {
    let pattern = format!("room-ralph.*{name}");
    let output = docker::exec_output(config, "agent", &["pgrep", "-f", &pattern]).ok()?;
    let pid = output.trim().lines().next()?.trim().to_string();
    Some(pid)
}
