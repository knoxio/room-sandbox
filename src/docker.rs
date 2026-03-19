use anyhow::{Context, Result, bail};
use std::process::Command;

use crate::config::{self, Config, Language, Utility};

/// Generate the Dockerfile content from config.
pub fn generate_dockerfile(config: &Config) -> String {
    let languages = &config.environment.languages;
    let utilities = &config.environment.utilities;

    let mut sections = vec![
        r#"FROM rust:bookworm

# Base dependencies (always installed)
RUN apt-get update && apt-get install -y \
    git openssh-client curl jq tmux pkg-config libssl-dev gosu sqlite3 unzip \
    && rm -rf /var/lib/apt/lists/*"#
            .to_string(),
    ];

    // Node.js — always installed (required for Claude Code), extras if user selected node
    if languages.contains(&Language::Node) {
        sections.push(
            r#"# Node.js
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y nodejs \
    && rm -rf /var/lib/apt/lists/* \
    && corepack enable && corepack prepare yarn@1.22.22 --activate \
    && corepack prepare pnpm@latest --activate \
    && npm install -g turbo"#
                .to_string(),
        );
    } else {
        sections.push(
            r#"# Node.js (required for Claude Code)
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y nodejs \
    && rm -rf /var/lib/apt/lists/*"#
                .to_string(),
        );
    }

    if languages.contains(&Language::Python) {
        sections.push(
            r#"# Python
RUN apt-get update && apt-get install -y \
    python3 python3-pip python3-venv \
    && rm -rf /var/lib/apt/lists/*"#
                .to_string(),
        );
    }

    // Claude Code (always installed — room-ralph depends on it)
    sections.push(
        r#"# Claude Code
RUN npm install -g @anthropic-ai/claude-code \
    && mv "$(which claude)" "$(which claude)-real"
COPY claude-wrapper.sh /usr/local/bin/claude
RUN chmod +x /usr/local/bin/claude"#
            .to_string(),
    );

    // Utilities
    if utilities.contains(&Utility::Glow) {
        sections.push(
            r#"# glow (markdown reader)
RUN mkdir -p /etc/apt/keyrings \
    && curl -fsSL https://repo.charm.sh/apt/gpg.key | gpg --dearmor -o /etc/apt/keyrings/charm.gpg \
    && echo "deb [signed-by=/etc/apt/keyrings/charm.gpg] https://repo.charm.sh/apt/ * *" \
      > /etc/apt/sources.list.d/charm.list \
    && apt-get update && apt-get install -y glow \
    && rm -rf /var/lib/apt/lists/*"#
                .to_string(),
        );
    }

    if utilities.contains(&Utility::Playwright) {
        sections.push(
            r#"# Playwright (browser automation)
ENV PLAYWRIGHT_BROWSERS_PATH=/ms-playwright
RUN npm install -g playwright@latest \
    && npx playwright install --with-deps chromium \
    && chmod -R 1777 /ms-playwright"#
                .to_string(),
        );
    }

    // Ansible requires python — install it if not already selected
    if utilities.contains(&Utility::Ansible) && !languages.contains(&Language::Python) {
        sections.push(
            r#"# Python (required for Ansible)
RUN apt-get update && apt-get install -y \
    python3 python3-pip python3-venv \
    && rm -rf /var/lib/apt/lists/*"#
                .to_string(),
        );
    }

    if utilities.contains(&Utility::Just) {
        sections.push(
            r#"# just (command runner)
RUN curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh | bash -s -- --to /usr/local/bin"#
                .to_string(),
        );
    }

    if utilities.contains(&Utility::Mise) {
        sections.push(
            r#"# mise (tool version manager)
RUN curl https://mise.run | sh \
    && mv /root/.local/bin/mise /usr/local/bin/mise"#
                .to_string(),
        );
    }

    if utilities.contains(&Utility::Proto) {
        sections.push(
            r#"# proto (toolchain manager)
RUN curl -fsSL https://moonrepo.dev/install/proto.sh | bash -s -- --yes \
    && mv /root/.proto/bin/proto /usr/local/bin/proto"#
                .to_string(),
        );
    }

    if utilities.contains(&Utility::Pulumi) {
        sections.push(
            r#"# Pulumi (infrastructure as code)
RUN curl -fsSL https://get.pulumi.com | sh \
    && mv /root/.pulumi/bin/* /usr/local/bin/"#
                .to_string(),
        );
    }

    if utilities.contains(&Utility::Ansible) {
        sections.push(
            r#"# Ansible (automation)
RUN pip3 install --break-system-packages ansible"#
                .to_string(),
        );
    }

    if utilities.contains(&Utility::AwsCli) {
        sections.push(
            r#"# AWS CLI
RUN curl -fsSL "https://awscli.amazonaws.com/awscli-exe-linux-$(uname -m).zip" -o /tmp/awscliv2.zip \
    && unzip -q /tmp/awscliv2.zip -d /tmp \
    && /tmp/aws/install \
    && rm -rf /tmp/aws /tmp/awscliv2.zip"#
                .to_string(),
        );
    }

    if utilities.contains(&Utility::Terraform) {
        sections.push(
            r#"# Terraform
RUN curl -fsSL https://apt.releases.hashicorp.com/gpg | gpg --dearmor -o /usr/share/keyrings/hashicorp-archive-keyring.gpg \
    && echo "deb [signed-by=/usr/share/keyrings/hashicorp-archive-keyring.gpg] https://apt.releases.hashicorp.com bookworm main" \
      > /etc/apt/sources.list.d/hashicorp.list \
    && apt-get update && apt-get install -y terraform \
    && rm -rf /var/lib/apt/lists/*"#
                .to_string(),
        );
    }

    if utilities.contains(&Utility::Docker) {
        sections.push(
            r#"# Docker CLI + Compose plugin (Docker-outside-of-Docker via mounted socket)
RUN install -m 0755 -d /etc/apt/keyrings \
    && curl -fsSL https://download.docker.com/linux/debian/gpg -o /etc/apt/keyrings/docker.asc \
    && chmod a+r /etc/apt/keyrings/docker.asc \
    && echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/debian bookworm stable" \
      > /etc/apt/sources.list.d/docker.list \
    && apt-get update && apt-get install -y docker-ce-cli docker-compose-plugin \
    && rm -rf /var/lib/apt/lists/*"#
                .to_string(),
        );
    }

    if utilities.contains(&Utility::Kubectl) {
        sections.push(
            r#"# kubectl (Kubernetes CLI)
RUN curl -fsSL "https://dl.k8s.io/release/$(curl -fsSL https://dl.k8s.io/release/stable.txt)/bin/linux/$(dpkg --print-architecture)/kubectl" \
      -o /usr/local/bin/kubectl \
    && chmod +x /usr/local/bin/kubectl"#
                .to_string(),
        );
    }

    if utilities.contains(&Utility::Yq) {
        sections.push(
            r#"# yq (YAML processor)
RUN curl -fsSL "https://github.com/mikefarah/yq/releases/latest/download/yq_linux_$(dpkg --print-architecture)" \
      -o /usr/local/bin/yq \
    && chmod +x /usr/local/bin/yq"#
                .to_string(),
        );
    }

    if languages.contains(&Language::Rust) {
        sections.push(
            r#"# Rust components
RUN rustup component add clippy rustfmt"#
                .to_string(),
        );
    }

    // room + room-ralph are always installed
    sections.push(
        r#"# room + room-ralph (always installed)
RUN cargo install room-cli room-ralph

# GitHub CLI
RUN curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg \
      | dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg \
    && echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" \
      > /etc/apt/sources.list.d/github-cli.list \
    && apt-get update && apt-get install -y gh \
    && rm -rf /var/lib/apt/lists/*

# Non-root user
RUN useradd -m -s /bin/bash -u 1000 agent

WORKDIR /workspaces

COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
CMD ["sleep", "infinity"]"#
            .to_string(),
    );

    sections.join("\n\n")
}

/// Generate docker-compose.yml content from config.
pub fn generate_compose(config: &Config) -> String {
    let mut volumes = vec![
        "      - ./workspaces:/mnt/sandbox-root".to_string(),
        "      - claude-data:/home/agent/.claude".to_string(),
        "      - cargo-cache:/usr/local/cargo/registry".to_string(),
        "      - room-data:/home/agent/.room".to_string(),
    ];

    if config.auth.mount_ssh {
        volumes.push("      - ~/.ssh:/home/agent/.ssh-host:ro".to_string());
    }

    if config.environment.utilities.contains(&Utility::Docker) {
        volumes.push("      - /var/run/docker.sock:/var/run/docker.sock".to_string());
    }

    let volumes_str = volumes.join("\n");

    format!(
        r#"services:
  sandbox:
    build:
      context: .
    container_name: {container}
    volumes:
{volumes_str}
    env_file: .env
    restart: unless-stopped

volumes:
  claude-data:
  cargo-cache:
  room-data:
"#,
        container = config.project.container_name,
    )
}

/// Generate the entrypoint.sh content.
pub fn generate_entrypoint(config: &Config) -> String {
    let room_name = &config.room.default;

    format!(
        r#"#!/bin/bash
set -e

# === Phase 1: Root setup ===

# Symlink each workspace into /workspaces
mkdir -p /workspaces
for dir in /mnt/sandbox-root/*/; do
    [ -d "$dir" ] || continue
    name=$(basename "$dir")
    if [ -L "/workspaces/$name" ]; then
        echo "[entrypoint] WARNING: duplicate workspace name '$name' — skipping ${{dir}}"
        continue
    fi
    ln -sfn "$dir" "/workspaces/$name"
done
echo "[entrypoint] Linked workspaces: $(ls /workspaces 2>/dev/null | tr '\n' ' ')"

# SSH: copy host keys to ~/.ssh
if [ -d /home/agent/.ssh-host ]; then
    rm -rf /home/agent/.ssh
    cp -r /home/agent/.ssh-host /home/agent/.ssh
    chmod 700 /home/agent/.ssh
    chmod 600 /home/agent/.ssh/* 2>/dev/null || true
    chmod 644 /home/agent/.ssh/*.pub 2>/dev/null || true
    chmod 644 /home/agent/.ssh/known_hosts 2>/dev/null || true
    chown -R agent:agent /home/agent/.ssh
fi

# Docker socket: match GID so agent can use it
if [ -S /var/run/docker.sock ]; then
    DOCKER_SOCK_GID=$(stat -c '%g' /var/run/docker.sock)
    if getent group "$DOCKER_SOCK_GID" >/dev/null 2>&1; then
        DOCKER_GROUP=$(getent group "$DOCKER_SOCK_GID" | cut -d: -f1)
    else
        groupadd -g "$DOCKER_SOCK_GID" dockerhost
        DOCKER_GROUP=dockerhost
    fi
    usermod -aG "$DOCKER_GROUP" agent
    echo "[entrypoint] Docker socket available (gid=$DOCKER_SOCK_GID, group=$DOCKER_GROUP)"
fi

# Distribute app .env to each workspace if APP_ENV points to a file
if [ -n "$APP_ENV" ] && [ -f "$APP_ENV" ]; then
    for dir in /workspaces/*/; do
        cp "$APP_ENV" "${{dir}}.env" 2>/dev/null || true
    done
    echo "[entrypoint] Distributed .env to all workspaces"
fi

# Fix ownership
chown -R agent:agent /home/agent/.claude /home/agent/.room 2>/dev/null || true
chown -R agent:agent /usr/local/cargo/registry /usr/local/cargo/git 2>/dev/null || true
chown agent:agent /workspaces

# Git config
gosu agent git config --global init.defaultBranch main
gosu agent git config --global user.email "agent@sandbox.dev"
gosu agent git config --global user.name "sandbox-agent"
if [ -d /home/agent/.ssh ]; then
    gosu agent git config --global core.sshCommand \
        "ssh -o StrictHostKeyChecking=accept-new"
fi

# Ensure ~/.local/bin is in PATH
grep -q '.local/bin' /home/agent/.bashrc 2>/dev/null || \
    echo 'export PATH="$HOME/.local/bin:$PATH"' >> /home/agent/.bashrc
chown agent:agent /home/agent/.bashrc

# Start room daemon and create default room
gosu agent room daemon 2>/dev/null &
sleep 1
TOKEN=$(gosu agent room join "system" 2>/dev/null \
    | python3 -c "import sys,json; print(json.load(sys.stdin)['token'])" 2>/dev/null || true)
if [ -n "$TOKEN" ]; then
    gosu agent room create "{room_name}" -t "$TOKEN" 2>/dev/null || true
fi

echo "[entrypoint] Ready."

# === Phase 2: Drop to agent user ===
exec gosu agent "$@"
"#
    )
}

/// Generate the claude-wrapper.sh content.
pub fn generate_claude_wrapper() -> &'static str {
    r#"#!/bin/bash
# Wrapper that injects --dangerously-skip-permissions for headless (-p) mode only.
for arg in "$@"; do
    if [ "$arg" = "-p" ] || [ "$arg" = "--print" ]; then
        exec claude-real --dangerously-skip-permissions "$@"
    fi
done
exec claude-real "$@"
"#
}

/// Write all Docker assets to .room-sandbox/
pub fn write_assets(config: &Config) -> Result<()> {
    let dir = config::sandbox_dir();
    std::fs::create_dir_all(&dir)?;

    std::fs::write(dir.join("Dockerfile"), generate_dockerfile(config))
        .context("failed to write Dockerfile")?;
    std::fs::write(dir.join("docker-compose.yml"), generate_compose(config))
        .context("failed to write docker-compose.yml")?;
    std::fs::write(dir.join("entrypoint.sh"), generate_entrypoint(config))
        .context("failed to write entrypoint.sh")?;
    std::fs::write(dir.join("claude-wrapper.sh"), generate_claude_wrapper())
        .context("failed to write claude-wrapper.sh")?;

    Ok(())
}

/// Run docker compose with the given subcommand args.
fn compose(args: &[&str]) -> Result<()> {
    let dir = config::sandbox_dir();
    let status = Command::new("docker")
        .args(["compose", "-f"])
        .arg(dir.join("docker-compose.yml"))
        .args(args)
        .status()
        .with_context(|| format!("failed to run docker compose {}", args.join(" ")))?;
    if !status.success() {
        bail!("docker compose {} failed", args.join(" "));
    }
    Ok(())
}

/// Run docker compose build.
pub fn build() -> Result<()> {
    compose(&["build"])
}

/// Run docker compose up -d.
pub fn up() -> Result<()> {
    compose(&["up", "-d"])
}

/// Run docker compose down.
pub fn down() -> Result<()> {
    compose(&["down"])
}

/// Run docker compose logs -f.
pub fn logs() -> Result<()> {
    compose(&["logs", "-f"])
}

/// Check if the sandbox container is running.
pub fn is_running(config: &Config) -> bool {
    Command::new("docker")
        .args(["inspect", "-f", "{{.State.Running}}"])
        .arg(&config.project.container_name)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "true")
        .unwrap_or(false)
}

/// Write role-based CLAUDE.md for each agent into the container's project-scoped config.
pub fn inject_agent_instructions(config: &Config) -> Result<()> {
    let container = &config.project.container_name;
    let room = &config.room.default;

    for agent in &config.agents {
        let project_dir = format!(
            "/home/agent/.claude/projects/-mnt-sandbox-root-{}",
            agent.name
        );

        // Ensure directory exists
        let _ = Command::new("docker")
            .args(["exec", "-u", "agent"])
            .arg(container)
            .args(["mkdir", "-p", &project_dir])
            .status();

        let instructions = generate_role_instructions(&agent.name, &agent.role, room);
        let claude_md_path = format!("{project_dir}/CLAUDE.md");

        // Write via docker exec sh -c 'cat > file'
        let mut child = Command::new("docker")
            .args(["exec", "-i", "-u", "agent"])
            .arg(container)
            .args(["sh", "-c", &format!("cat > '{claude_md_path}'")])
            .stdin(std::process::Stdio::piped())
            .spawn()
            .context("failed to write agent CLAUDE.md")?;

        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin.write_all(instructions.as_bytes())?;
        }

        let status = child.wait()?;
        if status.success() {
            eprintln!("  [{}] {} instructions written", agent.role, agent.name);
        }
    }

    Ok(())
}

fn generate_role_instructions(name: &str, role: &crate::config::AgentRole, room: &str) -> String {
    use crate::config::AgentRole;

    let role_section = match role {
        AgentRole::Coder => {
            r#"## Role: Coder

You are a **coder** agent. Your primary responsibilities:

- Pick up tasks from the taskboard and implement them
- Write clean, tested, production-quality code
- Create feature branches for your work
- Push changes and create pull requests when work is complete
- Report progress and blockers to the room

### Workflow
1. Check the taskboard for assigned or unassigned tasks
2. Claim a task before starting work
3. Implement the task on a feature branch
4. Run tests and ensure CI passes
5. Create a PR and notify the room
6. Move on to the next task"#
        }

        AgentRole::Reviewer => {
            r#"## Role: Reviewer

You are a **reviewer** agent. Your primary responsibilities:

- Review pull requests created by other agents
- Check code quality, correctness, and test coverage
- Leave constructive, specific feedback on PRs
- Approve PRs that meet quality standards
- Flag security issues, bugs, or architectural concerns

### Workflow
1. Monitor the room for PR review requests
2. Review the diff carefully — check logic, edge cases, tests
3. Leave inline comments on specific issues
4. Approve or request changes with clear reasoning
5. Notify the room when review is complete

### Guidelines
- Do NOT write code or implement features yourself
- Focus on catching bugs, not style preferences
- If a PR is good, approve it quickly — don't block unnecessarily"#
        }

        AgentRole::Manager => {
            r#"## Role: Manager

You are a **manager/orchestrator** agent. Your primary responsibilities:

- Break down high-level goals into concrete tasks
- Post tasks to the taskboard for agents to pick up
- Optionally assign tasks, or let agents self-assign
- Track progress and unblock stuck agents
- Coordinate between agents working on related features
- Prioritize work and manage the taskboard

### Workflow
1. Receive goals or feature requests from the human operator
2. Break them into well-defined, independent tasks
3. Post tasks to the taskboard with clear descriptions
4. Let agents pick tasks, or assign directly when needed
5. Monitor progress and help resolve blockers
6. Request reviews when PRs are ready

### Guidelines
- Do NOT write code yourself — delegate to coders
- Keep tasks small and independently testable
- Ensure agents aren't working on conflicting changes
- Prefer letting agents self-assign — only assign directly when coordinating dependent work
- Escalate to the human operator when decisions are needed"#
        }
    };

    format!(
        r#"# Agent Instructions

You are **{name}**, operating in room **{room}**.

{role_section}

## Communication

Use the room to coordinate with other agents and the human operator:
- Send updates when you start/finish tasks
- Ask for help if you're blocked
- Respond to messages directed at you (@{name})
"#
    )
}

/// Ensure the container is running, auto-starting if needed.
pub fn ensure_running(config: &Config) -> Result<()> {
    if !is_running(config) {
        eprintln!("Container not running — starting...");
        up()?;
    }
    Ok(())
}

/// Execute a command inside the container.
pub fn exec(config: &Config, user: &str, workdir: &str, args: &[&str]) -> Result<()> {
    let status = Command::new("docker")
        .args(["exec", "-it", "-u", user, "-w", workdir])
        .arg(&config.project.container_name)
        .args(args)
        .status()
        .context("failed to docker exec")?;
    if !status.success() {
        bail!("docker exec failed with status {}", status);
    }
    Ok(())
}

/// Execute a command inside the container (non-interactive, capture output).
pub fn exec_output(config: &Config, user: &str, args: &[&str]) -> Result<String> {
    let output = Command::new("docker")
        .args(["exec", "-u", user])
        .arg(&config.project.container_name)
        .args(args)
        .output()
        .context("failed to docker exec")?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Check if a ralph process is running for a given agent.
pub fn is_agent_running(config: &Config, name: &str) -> bool {
    let pattern = format!("room-ralph.*{name}");
    Command::new("docker")
        .args(["exec", "-u", "agent"])
        .arg(&config.project.container_name)
        .args(["pgrep", "-f", &pattern])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Kill the ralph process and all its children (claude) for a given agent.
///
/// Uses process group kill via a shell one-liner inside the container:
/// 1. Find the ralph PID
/// 2. Send SIGTERM to the entire process group (`kill -- -$PID`)
/// 3. Wait for graceful shutdown
/// 4. SIGKILL the group if still alive
pub fn kill_agent(config: &Config, name: &str) -> Result<()> {
    let container = &config.project.container_name;
    let script = format!(
        r#"PID=$(pgrep -f "room-ralph.*{name}" | head -1); \
           if [ -n "$PID" ]; then \
             kill -- -$PID 2>/dev/null || kill $PID 2>/dev/null; \
             sleep 2; \
             if kill -0 $PID 2>/dev/null; then \
               kill -9 -- -$PID 2>/dev/null || kill -9 $PID 2>/dev/null; \
             fi; \
           fi"#
    );
    let _ = Command::new("docker")
        .args(["exec", "-u", "agent"])
        .arg(container)
        .args(["bash", "-c", &script])
        .status();
    Ok(())
}

/// Ensure the room daemon is running and the default room exists.
pub fn ensure_room(config: &Config) -> Result<()> {
    let container = &config.project.container_name;
    let room = &config.room.default;

    // Start daemon (idempotent — exits cleanly if already running)
    let _ = Command::new("docker")
        .args(["exec", "-d", "-u", "agent"])
        .arg(container)
        .args(["room", "daemon"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    // Brief pause for daemon to start
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Join as system user, create room (all idempotent)
    let join_output = Command::new("docker")
        .args(["exec", "-u", "agent"])
        .arg(container)
        .args(["room", "join", "system"])
        .output()
        .context("failed to join room")?;

    if join_output.status.success() {
        let stdout = String::from_utf8_lossy(&join_output.stdout);
        // Parse token from JSON output
        if let Some(token) = parse_token(&stdout) {
            let _ = Command::new("docker")
                .args(["exec", "-u", "agent"])
                .arg(container)
                .args(["room", "create", room, "-t", &token])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        }
    }

    Ok(())
}

fn parse_token(json: &str) -> Option<String> {
    let parsed: serde_json::Value = serde_json::from_str(json).ok()?;
    parsed.get("token")?.as_str().map(|s| s.to_string())
}

/// Map an AgentRole to room-ralph's --personality flag value.
fn role_to_personality(role: &crate::config::AgentRole) -> &'static str {
    use crate::config::AgentRole;
    match role {
        AgentRole::Coder => "coder",
        AgentRole::Reviewer => "reviewer",
        AgentRole::Manager => "coordinator",
    }
}

/// Build the room-ralph args for a given agent.
fn ralph_cmd_args<'a>(config: &'a Config, name: &'a str, ralph_args: &'a [String]) -> Vec<String> {
    let room = &config.room.default;
    let personality = config
        .get_agent(name)
        .map(|a| role_to_personality(&a.role))
        .unwrap_or("coder");

    let mut args = vec![
        "room-ralph".to_string(),
        room.to_string(),
        name.to_string(),
        "--personality".to_string(),
        personality.to_string(),
        "--allow-all".to_string(),
    ];
    args.extend(ralph_args.iter().cloned());
    args
}

/// Start agents in the background via detached docker exec.
pub fn start_agents_background(
    config: &Config,
    names: &[String],
    ralph_args: &[String],
) -> Result<()> {
    let container = &config.project.container_name;

    for name in names {
        let workdir = format!("/workspaces/{name}");
        let args = ralph_cmd_args(config, name, ralph_args);
        let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

        let status = Command::new("docker")
            .args(["exec", "-d", "-u", "agent", "-w", &workdir])
            .arg(container)
            .args(&args_ref)
            .status()
            .with_context(|| format!("failed to start agent {name}"))?;

        let personality = config
            .get_agent(name)
            .map(|a| role_to_personality(&a.role))
            .unwrap_or("coder");

        if status.success() {
            eprintln!("  started {name} ({personality})");
        } else {
            eprintln!("  failed to start {name}");
        }
    }

    Ok(())
}

/// Start agents and tail their multiplexed output with colored prefixes.
pub fn start_agents_tailed(config: &Config, names: &[String], ralph_args: &[String]) -> Result<()> {
    use std::io::{BufRead, BufReader};
    use std::sync::mpsc;
    use std::thread;

    let colors = [
        "\x1b[36m", // cyan
        "\x1b[33m", // yellow
        "\x1b[35m", // magenta
        "\x1b[32m", // green
        "\x1b[34m", // blue
        "\x1b[31m", // red
        "\x1b[96m", // bright cyan
        "\x1b[93m", // bright yellow
    ];
    let reset = "\x1b[0m";

    let container = &config.project.container_name;
    let room = &config.room.default;

    let (tx, rx) = mpsc::channel::<(String, String)>();

    let mut children = Vec::new();

    for (i, name) in names.iter().enumerate() {
        let color = colors[i % colors.len()];
        let prefix = format!("{color}{:<12}{reset}", name);

        let personality = config
            .get_agent(name)
            .map(|a| role_to_personality(&a.role))
            .unwrap_or("coder");
        eprintln!("{prefix} starting in room '{room}' ({personality})...");

        let workdir = format!("/workspaces/{name}");
        let args = ralph_cmd_args(config, name, ralph_args);
        let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

        let mut cmd = Command::new("docker");
        cmd.args(["exec", "-u", "agent", "-w", &workdir])
            .arg(container)
            .args(&args_ref)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut child = cmd
            .spawn()
            .with_context(|| format!("failed to start agent {name}"))?;

        // Spawn thread for stdout
        if let Some(stdout) = child.stdout.take() {
            let tx = tx.clone();
            let prefix = prefix.clone();
            thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines().map_while(Result::ok) {
                    let _ = tx.send((prefix.clone(), line));
                }
            });
        }

        // Spawn thread for stderr
        if let Some(stderr) = child.stderr.take() {
            let tx = tx.clone();
            let prefix = prefix.clone();
            thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    let _ = tx.send((prefix.clone(), line));
                }
            });
        }

        children.push(child);
    }

    // Drop the original sender so rx closes when all threads finish
    drop(tx);

    // Print multiplexed output
    for (prefix, line) in rx {
        println!("{prefix} {line}");
    }

    // Wait for all children
    for mut child in children {
        let _ = child.wait();
    }

    Ok(())
}

/// Seed Claude Code auth credentials from the host into the container.
pub fn seed_claude_auth(config: &Config) {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/Users/unknown".to_string());
    let creds_path = format!("{home}/.claude/.credentials.json");

    if !std::path::Path::new(&creds_path).exists() {
        eprintln!("  No Claude credentials found at ~/.claude/.credentials.json");
        eprintln!("  Agents will need to authenticate — run: room-sandbox claude <agent-name>");
        return;
    }

    let container = &config.project.container_name;

    // Ensure the .claude directory exists in the container
    let _ = Command::new("docker")
        .args(["exec", "-u", "agent"])
        .arg(container)
        .args(["mkdir", "-p", "/home/agent/.claude"])
        .status();

    // Copy credentials into container
    let dest = format!("{container}:/home/agent/.claude/.credentials.json");
    let result = Command::new("docker")
        .args(["cp", &creds_path, &dest])
        .status();

    match result {
        Ok(status) if status.success() => {
            // Fix ownership
            let _ = Command::new("docker")
                .args(["exec", "-u", "root"])
                .arg(container)
                .args([
                    "chown",
                    "agent:agent",
                    "/home/agent/.claude/.credentials.json",
                ])
                .status();
            eprintln!("  Copied Claude credentials from host");
        }
        _ => {
            eprintln!("  Failed to copy Claude credentials");
            eprintln!("  Run: room-sandbox claude <agent-name> to authenticate manually");
        }
    }
}

/// Run Claude Code interactively in an agent's workspace.
pub fn run_claude(config: &Config, name: &str, extra_args: &[String]) -> Result<()> {
    let container = &config.project.container_name;
    let workdir = format!("/workspaces/{name}");

    let status = Command::new("docker")
        .args(["exec", "-it", "-u", "agent", "-w", &workdir])
        .arg(container)
        .args(["claude-real", "--dangerously-skip-permissions"])
        .args(extra_args)
        .status()
        .context("failed to run claude")?;

    if !status.success() {
        bail!("claude exited with status {}", status);
    }
    Ok(())
}

/// Clone the target repo into an agent's workspace.
pub fn clone_workspace(repo: &str, name: &str) -> Result<()> {
    let workspace = config::agent_workspace(name);
    if workspace.exists() {
        eprintln!("  [skip] {name} — workspace already exists");
        return Ok(());
    }

    eprintln!("  [clone] {name}");
    let status = Command::new("git")
        .args(["clone", repo])
        .arg(&workspace)
        .status()
        .context("failed to clone repo")?;
    if !status.success() {
        bail!("git clone failed for agent {name}");
    }
    Ok(())
}
