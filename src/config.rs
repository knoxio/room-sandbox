use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const CONFIG_FILE: &str = "sandbox.toml";
const SANDBOX_DIR: &str = ".room-sandbox";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub project: ProjectConfig,
    #[serde(rename = "agent")]
    pub agents: Vec<AgentDef>,
    pub room: RoomConfig,
    pub auth: AuthConfig,
    pub environment: EnvironmentConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub repo: String,
    pub container_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDef {
    pub name: String,
    #[serde(default)]
    pub role: AgentRole,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum AgentRole {
    #[default]
    Coder,
    Reviewer,
    Manager,
}

impl std::fmt::Display for AgentRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentRole::Coder => write!(f, "coder"),
            AgentRole::Reviewer => write!(f, "reviewer"),
            AgentRole::Manager => write!(f, "manager"),
        }
    }
}

impl Config {
    /// Get all agent names.
    pub fn agent_names(&self) -> Vec<&str> {
        self.agents.iter().map(|a| a.name.as_str()).collect()
    }

    /// Check if an agent name exists.
    pub fn has_agent(&self, name: &str) -> bool {
        self.agents.iter().any(|a| a.name == name)
    }

    /// Get an agent def by name.
    pub fn get_agent(&self, name: &str) -> Option<&AgentDef> {
        self.agents.iter().find(|a| a.name == name)
    }

    /// Formatted list of agent names for error messages.
    pub fn agent_names_joined(&self) -> String {
        self.agent_names().join(", ")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomConfig {
    pub default: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub method: AuthMethod,
    pub mount_ssh: bool,
    /// GitHub account username (for gh-cli multi-account selection)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gh_account: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum AuthMethod {
    GhCli,
    Pat,
    Ssh,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentConfig {
    #[serde(default)]
    pub languages: Vec<Language>,
    #[serde(default)]
    pub utilities: Vec<Utility>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Rust,
    Node,
    Python,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Utility {
    Glow,
    Playwright,
    Just,
    Mise,
    Proto,
    Pulumi,
    Ansible,
    AwsCli,
    Terraform,
    Docker,
    Kubectl,
    Yq,
}

impl Config {
    /// Load config from `sandbox.toml` in the current directory.
    pub fn load() -> Result<Self> {
        let path = config_path();
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        toml::from_str(&content).context("failed to parse sandbox.toml")
    }

    /// Write config to `sandbox.toml` in the current directory.
    pub fn save(&self) -> Result<()> {
        let content = toml::to_string_pretty(self).context("failed to serialize config")?;
        std::fs::write(config_path(), content).context("failed to write sandbox.toml")
    }

    /// Check if a sandbox.toml exists in the current directory.
    pub fn exists() -> bool {
        config_path().exists()
    }
}

/// Normalize a repo input to a full git URL.
///
/// Accepts:
/// - `org/repo` → resolves based on auth method
/// - `git@github.com:org/repo.git` → as-is
/// - `https://github.com/org/repo.git` → as-is
pub fn normalize_repo_url(input: &str, auth: &AuthMethod) -> String {
    if input.starts_with("git@") || input.starts_with("https://") || input.starts_with("http://") {
        return input.to_string();
    }

    // Short form: org/repo
    let clean = input.trim_end_matches(".git");
    match auth {
        AuthMethod::Ssh => format!("git@github.com:{clean}.git"),
        AuthMethod::GhCli | AuthMethod::Pat => format!("https://github.com/{clean}.git"),
    }
}

/// Derive a container name from the current directory name.
pub fn default_container_name() -> Result<String> {
    let dir_name = std::env::current_dir()?
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "sandbox".to_string());
    Ok(format!("sandbox-{dir_name}"))
}

/// Detect languages from repo marker files.
pub fn detect_languages(path: &Path) -> Vec<Language> {
    let mut langs = Vec::new();
    if path.join("Cargo.toml").exists() {
        langs.push(Language::Rust);
    }
    if path.join("package.json").exists() {
        langs.push(Language::Node);
    }
    if path.join("requirements.txt").exists()
        || path.join("pyproject.toml").exists()
        || path.join("setup.py").exists()
    {
        langs.push(Language::Python);
    }
    langs
}

/// Path to sandbox.toml
pub fn config_path() -> PathBuf {
    PathBuf::from(CONFIG_FILE)
}

/// Path to the .room-sandbox directory
pub fn sandbox_dir() -> PathBuf {
    PathBuf::from(SANDBOX_DIR)
}

/// Path to the workspaces directory inside .room-sandbox
pub fn workspaces_dir() -> PathBuf {
    sandbox_dir().join("workspaces")
}

/// Path to a specific agent's workspace
pub fn agent_workspace(name: &str) -> PathBuf {
    workspaces_dir().join(name)
}

/// Detect if the current directory is a git repo and return the remote URL.
pub fn detect_git_repo() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

/// Check if the current directory is a git repository.
pub fn is_git_repo() -> bool {
    std::process::Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Validate that we're in a valid directory for init.
/// Returns Ok(Some(remote)) if inside a git repo, Ok(None) if empty dir.
pub fn validate_init_dir() -> Result<Option<String>> {
    if Config::exists() {
        bail!("sandbox.toml already exists — use `room-sandbox apply` instead");
    }

    if is_git_repo() {
        return Ok(detect_git_repo());
    }

    // Check if directory is empty (ignoring DESIGN.md and hidden files we might have created)
    let entries: Vec<_> = std::fs::read_dir(".")?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            !name.starts_with('.') && name != "DESIGN.md"
        })
        .collect();

    if !entries.is_empty() {
        bail!(
            "directory is not empty and not a git repo — init in an empty directory or inside a git repo"
        );
    }

    Ok(None)
}
