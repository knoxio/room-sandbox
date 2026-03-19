use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt;

use crate::config::Config;

const STATE_FILE: &str = ".room-sandbox/.sandbox-state.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub applied_at: String,
    pub config_hashes: HashMap<String, String>,
}

/// What kind of drift was detected per config section.
#[derive(Debug)]
pub struct Drift {
    pub sections: Vec<DriftedSection>,
}

#[derive(Debug)]
pub struct DriftedSection {
    pub name: String,
    pub impact: DriftImpact,
}

/// How a drift affects operations.
#[derive(Debug, Clone, PartialEq)]
pub enum DriftImpact {
    /// Only affects agent instructions (room.default changed)
    InstructionsOnly,
    /// Agent list changed — workspaces need clone/removal
    Agents,
    /// Container needs rebuild (environment changed)
    ContainerRebuild,
    /// Container needs restart (auth changed, env vars)
    ContainerRestart,
    /// Compose file needs regeneration (mount changes)
    ComposeRegenerate,
    /// Destructive — repo changed, all workspaces wiped
    Destructive,
    /// Container name changed
    ContainerRename,
}

impl Drift {
    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }

    /// Whether this drift requires a container rebuild or more.
    pub fn needs_container_rebuild(&self) -> bool {
        self.sections.iter().any(|s| {
            matches!(
                s.impact,
                DriftImpact::ContainerRebuild
                    | DriftImpact::ComposeRegenerate
                    | DriftImpact::Destructive
                    | DriftImpact::ContainerRename
            )
        })
    }

    /// Whether this drift is destructive (repo change).
    pub fn is_destructive(&self) -> bool {
        self.sections
            .iter()
            .any(|s| matches!(s.impact, DriftImpact::Destructive))
    }

    /// Whether a specific agent is affected by drift (added but not yet cloned).
    pub fn agent_missing(&self, _name: &str) -> bool {
        self.sections
            .iter()
            .any(|s| matches!(s.impact, DriftImpact::Agents | DriftImpact::Destructive))
    }
}

impl fmt::Display for Drift {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return write!(f, "No changes detected.");
        }
        writeln!(f, "Changes detected:")?;
        for section in &self.sections {
            writeln!(f, "  [{}] — {:?}", section.name, section.impact)?;
        }
        Ok(())
    }
}

impl State {
    /// Load state from .room-sandbox/.sandbox-state.json
    pub fn load() -> Result<Option<Self>> {
        let path = std::path::PathBuf::from(STATE_FILE);
        if !path.exists() {
            return Ok(None);
        }
        let content =
            std::fs::read_to_string(&path).context("failed to read .sandbox-state.json")?;
        let state: State =
            serde_json::from_str(&content).context("failed to parse .sandbox-state.json")?;
        Ok(Some(state))
    }

    /// Save state after a successful apply.
    pub fn save_from_config(config: &Config) -> Result<()> {
        let state = State {
            applied_at: chrono::Utc::now().to_rfc3339(),
            config_hashes: compute_hashes(config),
        };
        let path = std::path::PathBuf::from(STATE_FILE);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(&state)?;
        std::fs::write(path, content).context("failed to write .sandbox-state.json")
    }
}

/// Compare current config against last-applied state and return drift.
pub fn check_state() -> Result<Drift> {
    let config = Config::load()?;
    let state = State::load()?;

    let Some(state) = state else {
        // No state file — everything needs to be applied
        return Ok(Drift {
            sections: vec![DriftedSection {
                name: "all".to_string(),
                impact: DriftImpact::ContainerRebuild,
            }],
        });
    };

    let current_hashes = compute_hashes(&config);
    let mut drifted = Vec::new();

    let section_impacts = [
        ("project.repo", DriftImpact::Destructive),
        ("project.container_name", DriftImpact::ContainerRename),
        ("agents", DriftImpact::Agents),
        ("room", DriftImpact::InstructionsOnly),
        ("auth.method", DriftImpact::ContainerRestart),
        ("auth.mount_ssh", DriftImpact::ComposeRegenerate),
        ("environment", DriftImpact::ContainerRebuild),
    ];

    for (section, impact) in section_impacts {
        let current = current_hashes.get(section);
        let applied = state.config_hashes.get(section);
        if current != applied {
            drifted.push(DriftedSection {
                name: section.to_string(),
                impact,
            });
        }
    }

    Ok(Drift { sections: drifted })
}

/// Warn the user about drift. Returns the drift for further inspection.
pub fn warn_drift() -> Result<Drift> {
    let drift = check_state()?;
    if !drift.is_empty() {
        eprintln!(
            "warning: sandbox.toml has unapplied changes — run `room-sandbox apply`\n{drift}"
        );
    }
    Ok(drift)
}

fn compute_hashes(config: &Config) -> HashMap<String, String> {
    let mut hashes = HashMap::new();

    hashes.insert("project.repo".to_string(), hash_str(&config.project.repo));
    hashes.insert(
        "project.container_name".to_string(),
        hash_str(&config.project.container_name),
    );
    hashes.insert(
        "agents".to_string(),
        hash_str(&format!("{:?}", config.agents)),
    );
    hashes.insert("room".to_string(), hash_str(&config.room.default));
    hashes.insert(
        "auth.method".to_string(),
        hash_str(&format!("{:?}", config.auth.method)),
    );
    hashes.insert(
        "auth.mount_ssh".to_string(),
        hash_str(&format!("{}", config.auth.mount_ssh)),
    );
    hashes.insert(
        "environment".to_string(),
        hash_str(&format!(
            "{:?}{:?}",
            config.environment.languages, config.environment.utilities
        )),
    );

    hashes
}

fn hash_str(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    hex::encode(hasher.finalize())
}
