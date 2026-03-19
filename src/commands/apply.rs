use anyhow::{Result, bail};
use inquire::Confirm;

use crate::config::{self, Config};
use crate::docker;
use crate::state::{self, DriftImpact, State};

pub fn run() -> Result<()> {
    let config = Config::load()?;
    let drift = state::check_state()?;

    if drift.is_empty() {
        eprintln!("Everything is up to date — nothing to apply.");
        return Ok(());
    }

    // Display the plan
    eprintln!("{drift}");
    display_actions(&config, &drift)?;

    if drift.is_destructive() {
        eprintln!(
            "\n  WARNING: This includes destructive changes that will delete existing workspaces.\n"
        );
    }

    let confirm = Confirm::new("Apply these changes?")
        .with_default(false)
        .prompt()?;

    if !confirm {
        eprintln!("Aborted.");
        return Ok(());
    }

    apply_changes(&config, &drift)?;
    State::save_from_config(&config)?;
    eprintln!("\nChanges applied.");

    Ok(())
}

/// Apply changes and show the plan, used by both `apply` and `agent add/remove`.
pub fn apply_with_config(config: &Config) -> Result<()> {
    let drift = state::check_state()?;

    if drift.is_empty() {
        // Even if no drift detected, still save state (agent add/remove already updated toml)
        State::save_from_config(config)?;
        return Ok(());
    }

    eprintln!("{drift}");
    display_actions(config, &drift)?;

    if drift.is_destructive() {
        eprintln!(
            "\n  WARNING: This includes destructive changes that will delete existing workspaces.\n"
        );
    }

    let confirm = Confirm::new("Apply these changes?")
        .with_default(false)
        .prompt()?;

    if !confirm {
        eprintln!("Aborted.");
        bail!("user cancelled apply");
    }

    apply_changes(config, &drift)?;
    State::save_from_config(config)?;
    eprintln!("\nChanges applied.");

    Ok(())
}

fn display_actions(config: &Config, drift: &state::Drift) -> Result<()> {
    eprintln!("  Actions:");
    let mut step = 1;

    for section in &drift.sections {
        match section.impact {
            DriftImpact::Agents => {
                // Figure out added/removed by comparing state vs config
                let current_names = config.agent_names();

                // Check which workspaces exist
                for name in &current_names {
                    if !config::agent_workspace(name).exists() {
                        eprintln!("    {step}. Clone workspace for {name}");
                        step += 1;
                        eprintln!("    {step}. Write project-scoped CLAUDE.md for {name}");
                        step += 1;
                    }
                }

                // Check for removed agents (workspaces that exist but aren't in config)
                if let Ok(entries) = std::fs::read_dir(config::workspaces_dir()) {
                    for entry in entries.flatten() {
                        let dir_name = entry.file_name().to_string_lossy().to_string();
                        if !current_names.contains(&dir_name.as_str()) {
                            eprintln!("    {step}. Remove workspace {dir_name}");
                            step += 1;
                        }
                    }
                }
            }
            DriftImpact::ContainerRebuild => {
                eprintln!("    {step}. Rebuild container image (environment changed)");
                step += 1;
                eprintln!("    {step}. Recreate container");
                step += 1;
            }
            DriftImpact::ContainerRestart => {
                eprintln!("    {step}. Update .env (auth changed)");
                step += 1;
                eprintln!("    {step}. Restart container");
                step += 1;
            }
            DriftImpact::ComposeRegenerate => {
                eprintln!("    {step}. Regenerate docker-compose.yml");
                step += 1;
                eprintln!("    {step}. Recreate container");
                step += 1;
            }
            DriftImpact::Destructive => {
                eprintln!("    {step}. Remove ALL existing workspaces");
                step += 1;
                eprintln!(
                    "    {step}. Clone {} workspaces from new repo",
                    config.agents.len()
                );
                step += 1;
            }
            DriftImpact::ContainerRename => {
                eprintln!("    {step}. Stop old container");
                step += 1;
                eprintln!("    {step}. Start container with new name");
                step += 1;
            }
            DriftImpact::InstructionsOnly => {
                eprintln!("    {step}. Update agent CLAUDE.md templates");
                step += 1;
            }
        }
    }

    Ok(())
}

fn apply_changes(config: &Config, drift: &state::Drift) -> Result<()> {
    let needs_rebuild = drift.needs_container_rebuild();
    let needs_restart = drift.sections.iter().any(|s| {
        matches!(
            s.impact,
            DriftImpact::ContainerRestart | DriftImpact::ContainerRename
        )
    });

    for section in &drift.sections {
        match section.impact {
            DriftImpact::Destructive => {
                eprintln!("Removing all workspaces...");
                if config::workspaces_dir().exists() {
                    std::fs::remove_dir_all(config::workspaces_dir())?;
                }
                std::fs::create_dir_all(config::workspaces_dir())?;
                for agent in &config.agents {
                    docker::clone_workspace(&config.project.repo, &agent.name)?;
                }
            }
            DriftImpact::Agents => {
                std::fs::create_dir_all(config::workspaces_dir())?;

                // Clone missing workspaces
                for agent in &config.agents {
                    if !config::agent_workspace(&agent.name).exists() {
                        docker::clone_workspace(&config.project.repo, &agent.name)?;
                    }
                }

                // Remove stale workspaces
                if let Ok(entries) = std::fs::read_dir(config::workspaces_dir()) {
                    for entry in entries.flatten() {
                        let dir_name = entry.file_name().to_string_lossy().to_string();
                        if !config.has_agent(&dir_name) {
                            eprintln!("  [remove] {dir_name}");
                            std::fs::remove_dir_all(entry.path())?;
                        }
                    }
                }
            }
            DriftImpact::InstructionsOnly => {
                eprintln!("Updating agent instructions...");
            }
            _ => {}
        }
    }

    // Regenerate Docker assets if needed
    if needs_rebuild {
        eprintln!("Regenerating Docker assets...");
        docker::write_assets(config)?;
        eprintln!("Building container...");
        docker::build()?;
        eprintln!("Starting container...");
        docker::up()?;
    } else if needs_restart {
        docker::write_assets(config)?;
        eprintln!("Restarting container...");
        docker::down()?;
        docker::up()?;
    }

    // Inject agent instructions if agents or room changed
    let needs_instructions = drift.sections.iter().any(|s| {
        matches!(
            s.impact,
            DriftImpact::Agents | DriftImpact::InstructionsOnly | DriftImpact::Destructive
        )
    });
    if needs_instructions && docker::is_running(config) {
        eprintln!("Writing agent instructions...");
        docker::inject_agent_instructions(config)?;
    }

    Ok(())
}
