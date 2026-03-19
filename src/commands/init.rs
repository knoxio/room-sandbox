use std::fmt;

use anyhow::{Context, Result};
use clap::Args;
use inquire::{Confirm, MultiSelect, Select, Text};

use crate::config::{
    self, AgentDef, AgentRole, AuthConfig, AuthMethod, Config, EnvironmentConfig, Language,
    ProjectConfig, RoomConfig, Utility,
};
use crate::docker;
use crate::state::State;

#[derive(Clone)]
struct LangOption {
    lang: Language,
    label: &'static str,
}

impl fmt::Display for LangOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label)
    }
}

#[derive(Clone)]
struct UtilityOption {
    utility: Utility,
    label: &'static str,
}

impl fmt::Display for UtilityOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label)
    }
}

#[derive(Args)]
pub struct InitArgs {
    /// Git repo URL or org/repo shorthand
    #[arg(long)]
    repo: Option<String>,

    /// Comma-separated agent names
    #[arg(long, value_delimiter = ',')]
    agents: Option<Vec<String>>,

    /// Comma-separated languages (rust, node, python)
    #[arg(long, value_delimiter = ',')]
    languages: Option<Vec<String>>,

    /// Comma-separated utilities (glow, playwright)
    #[arg(long, value_delimiter = ',')]
    utilities: Option<Vec<String>>,

    /// Auth method (gh-cli, pat, ssh)
    #[arg(long)]
    auth: Option<String>,

    /// Default room name
    #[arg(long)]
    room: Option<String>,
}

pub fn run(args: InitArgs) -> Result<()> {
    let detected_repo = config::validate_init_dir()?;
    let is_interactive = args.repo.is_none();

    if let Some(ref remote) = detected_repo {
        eprintln!("Detected git repo: {remote}");
    }

    let config = if is_interactive {
        run_wizard(detected_repo)?
    } else {
        build_from_args(args, detected_repo)?
    };

    eprintln!("\n--- Writing sandbox.toml ---");
    config.save()?;

    eprintln!("--- Setting up .room-sandbox/ ---");
    setup_sandbox_dir(&config)?;

    eprintln!("--- Writing Docker assets ---");
    docker::write_assets(&config)?;

    eprintln!("--- Cloning agent workspaces ---");
    std::fs::create_dir_all(config::workspaces_dir())?;
    for agent in &config.agents {
        docker::clone_workspace(&config.project.repo, &agent.name)?;
    }

    eprintln!("\n--- Building container (this may take a while on first run) ---");
    docker::build()?;

    eprintln!("--- Starting container ---");
    docker::up()?;

    // Seed Claude auth from host
    eprintln!("--- Seeding Claude auth ---");
    docker::seed_claude_auth(&config);

    // Inject role-based instructions
    eprintln!("--- Writing agent instructions ---");
    docker::inject_agent_instructions(&config)?;

    State::save_from_config(&config)?;

    eprintln!("\n=== Sandbox ready ===");
    eprintln!("  Agents: {}", config.agent_names_joined());
    eprintln!("  Room:   {}", config.room.default);
    eprintln!();
    eprintln!("Next steps:");
    eprintln!("  room-sandbox tui                  Open the room TUI");
    eprintln!("  room-sandbox agent start <name>    Start an agent");
    eprintln!("  room-sandbox shell <name>          Shell into a workspace");

    Ok(())
}

fn run_wizard(detected_repo: Option<String>) -> Result<Config> {
    eprintln!("\n=== room-sandbox init ===\n");

    // 1. Repo
    let repo_input: String = if let Some(ref detected) = detected_repo {
        let use_detected = Confirm::new(&format!("Use detected repo ({detected})?"))
            .with_default(true)
            .prompt()?;
        if use_detected {
            detected.clone()
        } else {
            Text::new("Git repo (org/repo, SSH, or HTTPS URL):").prompt()?
        }
    } else {
        Text::new("Git repo (org/repo, SSH, or HTTPS URL):").prompt()?
    };

    // 2. Auto-detect languages (shallow clone if needed)
    let detected_languages = if detected_repo.is_some() {
        config::detect_languages(&std::env::current_dir()?)
    } else {
        detect_from_shallow_clone(&repo_input)
    };

    // 3. Languages
    eprintln!();
    let all_languages = vec![
        LangOption { lang: Language::Rust, label: "rust" },
        LangOption { lang: Language::Node, label: "node" },
        LangOption { lang: Language::Python, label: "python" },
    ];
    let defaults: Vec<usize> = all_languages
        .iter()
        .enumerate()
        .filter(|(_, l)| detected_languages.contains(&l.lang))
        .map(|(i, _)| i)
        .collect();

    let lang_selections = MultiSelect::new("Languages:", all_languages.clone())
        .with_default(&defaults)
        .prompt()?;

    let languages: Vec<Language> = lang_selections.into_iter().map(|l| l.lang).collect();

    // 4. Utilities
    eprintln!();
    let all_utilities = vec![
        UtilityOption { utility: Utility::Glow, label: "glow (markdown reader)" },
        UtilityOption { utility: Utility::Playwright, label: "playwright (browser automation)" },
        UtilityOption { utility: Utility::Just, label: "just (command runner)" },
        UtilityOption { utility: Utility::Mise, label: "mise (tool version manager)" },
        UtilityOption { utility: Utility::Proto, label: "proto (toolchain manager)" },
        UtilityOption { utility: Utility::Pulumi, label: "pulumi (infrastructure as code)" },
        UtilityOption { utility: Utility::Ansible, label: "ansible (automation, requires python)" },
        UtilityOption { utility: Utility::AwsCli, label: "aws-cli (AWS command line)" },
        UtilityOption { utility: Utility::Terraform, label: "terraform (infrastructure as code)" },
        UtilityOption { utility: Utility::Docker, label: "docker (Docker-in-Docker CLI)" },
        UtilityOption { utility: Utility::Kubectl, label: "kubectl (Kubernetes CLI)" },
        UtilityOption { utility: Utility::Yq, label: "yq (YAML processor)" },
    ];

    let utility_selections = MultiSelect::new("Utilities:", all_utilities.clone())
        .prompt()?;

    let utilities: Vec<Utility> = utility_selections.into_iter().map(|u| u.utility).collect();

    // 5. Agents
    eprintln!();
    let agents_input = Text::new("Agent names (comma-separated):")
        .with_default("r2d2, bumblebee, saphire")
        .prompt()?;
    let agent_name_list: Vec<String> = agents_input
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let roles = vec!["coder", "reviewer", "manager"];
    let mut agent_defs: Vec<AgentDef> = Vec::new();
    for name in &agent_name_list {
        let role_selection = Select::new(
            &format!("Role for '{name}':"),
            roles.clone(),
        )
        .prompt()?;
        let role = match role_selection {
            "reviewer" => AgentRole::Reviewer,
            "manager" => AgentRole::Manager,
            _ => AgentRole::Coder,
        };
        agent_defs.push(AgentDef {
            name: name.clone(),
            role,
        });
    }

    // 6. Auth
    eprintln!();
    let gh_accounts = detect_gh_accounts();
    let auth_method;
    let mut selected_gh_account: Option<String> = None;

    if gh_accounts.is_empty() {
        let auth_options = vec!["PAT (provide a token)", "SSH only"];
        eprintln!("No gh CLI accounts detected.");
        let selection = Select::new("Auth method:", auth_options).prompt()?;
        auth_method = match selection {
            "PAT (provide a token)" => AuthMethod::Pat,
            _ => AuthMethod::Ssh,
        };
    } else {
        let mut auth_options: Vec<String> = gh_accounts
            .iter()
            .map(|a| {
                let marker = if a.active { " (active)" } else { "" };
                format!("gh-cli: {}{marker}", a.username)
            })
            .collect();
        auth_options.push("PAT (provide a token)".to_string());
        auth_options.push("SSH only".to_string());

        let selection = Select::new("Auth method:", auth_options.clone()).prompt()?;

        let selected_idx = auth_options.iter().position(|o| *o == selection).unwrap_or(0);
        if selected_idx < gh_accounts.len() {
            auth_method = AuthMethod::GhCli;
            selected_gh_account = Some(gh_accounts[selected_idx].username.clone());
        } else if selection == "PAT (provide a token)" {
            auth_method = AuthMethod::Pat;
        } else {
            auth_method = AuthMethod::Ssh;
        }
    };

    // 7. SSH mount
    eprintln!();
    let mount_ssh = Confirm::new("Mount ~/.ssh into container?")
        .with_default(auth_method == AuthMethod::Ssh)
        .prompt()?;

    // 8. Room name
    eprintln!();
    let room_name = Text::new("Default room name:")
        .with_default("dev")
        .prompt()?;

    // Normalize repo URL now that we know auth method
    let repo = config::normalize_repo_url(&repo_input, &auth_method);
    let container_name = config::default_container_name()?;
    // Stash the selected gh account for .env generation
    let gh_account_for_env = selected_gh_account;

    Ok(Config {
        project: ProjectConfig {
            repo,
            container_name,
        },
        agents: agent_defs,
        room: RoomConfig { default: room_name },
        auth: AuthConfig {
            method: auth_method,
            mount_ssh,
            gh_account: gh_account_for_env,
        },
        environment: EnvironmentConfig { languages, utilities },
    })
}

fn build_from_args(args: InitArgs, detected_repo: Option<String>) -> Result<Config> {
    let auth_method = match args.auth.as_deref() {
        Some("pat") => AuthMethod::Pat,
        Some("ssh") => AuthMethod::Ssh,
        _ => AuthMethod::GhCli,
    };

    let repo_input = args
        .repo
        .or(detected_repo)
        .context("--repo is required when not in a git repo")?;
    let repo = config::normalize_repo_url(&repo_input, &auth_method);

    let agent_defs: Vec<AgentDef> = args
        .agents
        .unwrap_or_else(|| vec!["r2d2".into(), "bumblebee".into(), "saphire".into()])
        .into_iter()
        .map(|name| AgentDef { name, role: AgentRole::default() })
        .collect();

    let languages = args
        .languages
        .map(|langs| {
            langs
                .into_iter()
                .filter_map(|l| match l.as_str() {
                    "rust" => Some(Language::Rust),
                    "node" => Some(Language::Node),
                    "python" => Some(Language::Python),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();

    let utilities = args
        .utilities
        .map(|us| {
            us.into_iter()
                .filter_map(|u| match u.as_str() {
                    "glow" => Some(Utility::Glow),
                    "playwright" => Some(Utility::Playwright),
                    "just" => Some(Utility::Just),
                    "mise" => Some(Utility::Mise),
                    "proto" => Some(Utility::Proto),
                    "pulumi" => Some(Utility::Pulumi),
                    "ansible" => Some(Utility::Ansible),
                    "aws-cli" | "aws" => Some(Utility::AwsCli),
                    "terraform" => Some(Utility::Terraform),
                    "docker" => Some(Utility::Docker),
                    "kubectl" => Some(Utility::Kubectl),
                    "yq" => Some(Utility::Yq),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();

    let container_name = config::default_container_name()?;

    Ok(Config {
        project: ProjectConfig {
            repo,
            container_name,
        },
        agents: agent_defs,
        room: RoomConfig {
            default: args.room.unwrap_or_else(|| "dev".to_string()),
        },
        auth: AuthConfig {
            method: auth_method,
            mount_ssh: auth_method == AuthMethod::Ssh,
            gh_account: None,
        },
        environment: EnvironmentConfig { languages, utilities },
    })
}

fn setup_sandbox_dir(config: &Config) -> Result<()> {
    let dir = config::sandbox_dir();
    std::fs::create_dir_all(&dir)?;

    // Write .env
    let env_content = generate_env(config)?;
    std::fs::write(dir.join(".env"), env_content)?;

    // Add .room-sandbox/ to .gitignore if not already there
    let gitignore_path = std::path::PathBuf::from(".gitignore");
    let gitignore_entry = ".room-sandbox/";
    if gitignore_path.exists() {
        let content = std::fs::read_to_string(&gitignore_path)?;
        if !content.lines().any(|l| l.trim() == gitignore_entry) {
            std::fs::write(&gitignore_path, format!("{content}\n{gitignore_entry}\n"))?;
        }
    } else {
        std::fs::write(&gitignore_path, format!("{gitignore_entry}\n"))?;
    }

    Ok(())
}

fn generate_env(config: &Config) -> Result<String> {
    let mut lines = vec![
        "# === Required ===".to_string(),
        String::new(),
        "# Anthropic API key for Claude Code".to_string(),
        "ANTHROPIC_API_KEY=".to_string(),
    ];

    match config.auth.method {
        AuthMethod::GhCli => {
            let mut cmd = std::process::Command::new("gh");
            cmd.args(["auth", "token"]);
            if let Some(ref account) = config.auth.gh_account {
                cmd.args(["-u", account]);
            }
            let token = cmd
                .output()
                .ok()
                .and_then(|o| {
                    if o.status.success() {
                        Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                    } else {
                        None
                    }
                })
                .unwrap_or_default();
            let account_label = config
                .auth
                .gh_account
                .as_deref()
                .unwrap_or("default");
            lines.push(String::new());
            lines.push(format!("# GitHub token (from gh CLI, account: {account_label})"));
            lines.push(format!("GH_TOKEN={token}"));
        }
        AuthMethod::Pat => {
            lines.push(String::new());
            lines.push("# GitHub personal access token".to_string());
            lines.push("GH_TOKEN=".to_string());
        }
        AuthMethod::Ssh => {}
    }

    lines.push(String::new());
    lines.push("# === Optional ===".to_string());
    lines.push(String::new());
    lines.push("# App .env to distribute to each workspace".to_string());
    lines.push("APP_ENV=".to_string());

    Ok(lines.join("\n"))
}

struct GhAccount {
    username: String,
    active: bool,
}

fn detect_gh_accounts() -> Vec<GhAccount> {
    let output = match std::process::Command::new("gh")
        .args(["auth", "status"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    // gh auth status may output to stdout or stderr depending on version
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let text = if stdout.contains("Logged in") {
        stdout
    } else {
        stderr
    };

    let mut accounts = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        // Lines like: "✓ Logged in to github.com account knoxio (keyring)"
        if trimmed.contains("Logged in") && trimmed.contains("account") {
            if let Some(after_account) = trimmed.split("account ").nth(1) {
                let username = after_account
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_string();
                if !username.is_empty() {
                    accounts.push(GhAccount {
                        username,
                        active: false,
                    });
                }
            }
        }
        // Lines like: "- Active account: true"
        if trimmed.contains("Active account: true") {
            if let Some(last) = accounts.last_mut() {
                last.active = true;
            }
        }
    }

    accounts
}

fn detect_from_shallow_clone(repo_input: &str) -> Vec<Language> {
    let temp = std::env::temp_dir().join("room-sandbox-detect");
    let _ = std::fs::remove_dir_all(&temp);

    // Try SSH first, then HTTPS for short form
    let urls_to_try = if repo_input.starts_with("git@") || repo_input.starts_with("http") {
        vec![repo_input.to_string()]
    } else {
        vec![
            format!("https://github.com/{}.git", repo_input.trim_end_matches(".git")),
            format!(
                "git@github.com:{}.git",
                repo_input.trim_end_matches(".git")
            ),
        ]
    };

    for url in &urls_to_try {
        let result = std::process::Command::new("git")
            .args(["clone", "--depth", "1", url])
            .arg(&temp)
            .output();

        if let Ok(output) = result {
            if output.status.success() {
                let langs = config::detect_languages(&temp);
                let _ = std::fs::remove_dir_all(&temp);
                return langs;
            }
        }
    }

    let _ = std::fs::remove_dir_all(&temp);
    Vec::new()
}
