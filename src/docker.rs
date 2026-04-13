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

    let needs_node =
        languages.contains(&Language::Node) || utilities.contains(&Utility::Playwright);

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
    } else if needs_node {
        sections.push(
            r#"# Node.js (required for Playwright)
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
        r#"# Claude Code (native binary)
RUN curl -fsSL https://claude.ai/install.sh | bash \
    && cp -L /root/.local/bin/claude /usr/local/bin/claude-real \
    && chmod 755 /usr/local/bin/claude-real \
    && rm -rf /root/.local/share/claude /root/.local/bin/claude
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

# Non-root user (UID matches host user to avoid permission issues)
ARG AGENT_UID=1000
RUN useradd -m -s /bin/bash -u $AGENT_UID agent

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
        // Mount SSH agent socket for forwarding — private keys never enter the container
        volumes.push("      - ${SSH_AUTH_SOCK:-/dev/null}:/tmp/ssh-agent.sock:ro".to_string());
        // Mount known_hosts and public keys only (no private key material)
        volumes.push("      - ~/.ssh/known_hosts:/home/agent/.ssh/known_hosts:ro".to_string());
    }

    if config.environment.utilities.contains(&Utility::Docker) {
        volumes.push("      - /var/run/docker.sock:/var/run/docker.sock".to_string());
    }

    let volumes_str = volumes.join("\n");

    let mut env_vars = Vec::new();
    if config.auth.mount_ssh {
        env_vars.push("      - SSH_AUTH_SOCK=/tmp/ssh-agent.sock".to_string());
    }

    let env_section = if env_vars.is_empty() {
        String::new()
    } else {
        format!("    environment:\n{}\n", env_vars.join("\n"))
    };

    // Detect host UID for container user mapping
    let host_uid = std::process::Command::new("id")
        .args(["-u"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "1000".to_string());

    format!(
        r#"services:
  sandbox:
    build:
      context: .
      args:
        AGENT_UID: "{host_uid}"
    container_name: {container}
    volumes:
{volumes_str}
    env_file: .env
{env_section}    restart: unless-stopped

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

# SSH agent forwarding: ensure the socket is accessible to the agent user.
# Private keys stay on the host — only the agent socket is forwarded.
if [ -S /tmp/ssh-agent.sock ]; then
    chmod 777 /tmp/ssh-agent.sock 2>/dev/null || true
    echo "[entrypoint] SSH agent socket available"
fi

# SSH known_hosts: ensure directory and permissions
mkdir -p /home/agent/.ssh
chmod 700 /home/agent/.ssh
chown -R agent:agent /home/agent/.ssh

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
/// Resolve the compose project name from config or directory name.
fn compose_project_name() -> String {
    Config::load()
        .map(|c| c.project.container_name.clone())
        .unwrap_or_else(|_| "room-sandbox".to_string())
}

fn compose(args: &[&str]) -> Result<()> {
    let dir = config::sandbox_dir();
    let project = compose_project_name();
    let status = Command::new("docker")
        .args(["compose", "-p", &project, "-f"])
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

/// Run docker compose build with --no-cache to pull latest packages.
pub fn build_no_cache() -> Result<()> {
    compose(&["build", "--no-cache"])
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

/// Ensure /workspaces/<name> symlinks exist inside the container for all agents.
/// The entrypoint creates these on boot, but agents added after boot need them too.
pub fn ensure_workspace_symlinks(config: &Config) -> Result<()> {
    let container = &config.project.container_name;
    for agent in &config.agents {
        let link = format!("/workspaces/{}", agent.name);
        let target = format!("/mnt/sandbox-root/{}", agent.name);
        // ln -sfn is idempotent
        let _ = Command::new("docker")
            .args(["exec", "-u", "agent"])
            .arg(container)
            .args(["ln", "-sfn", &target, &link])
            .output();
    }
    Ok(())
}

/// Write role-based CLAUDE.md for each agent into the container's project-scoped config.
/// Inject instructions for specific agents only.
pub fn inject_instructions_for(config: &Config, names: &[String]) -> Result<()> {
    let agents: Vec<_> = config
        .agents
        .iter()
        .filter(|a| names.iter().any(|n| n == &a.name))
        .collect();
    inject_instructions_inner(config, &agents)
}

pub fn inject_agent_instructions(config: &Config) -> Result<()> {
    let agents: Vec<_> = config.agents.iter().collect();
    inject_instructions_inner(config, &agents)
}

fn inject_instructions_inner(config: &Config, agents: &[&crate::config::AgentDef]) -> Result<()> {
    let container = &config.project.container_name;
    let room = &config.room.default;

    for agent in agents {
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
        let personality = generate_personality_file(&agent.name, &agent.role);

        // Write CLAUDE.md and personality file
        for (path, content) in [
            (format!("{project_dir}/CLAUDE.md"), &instructions),
            (
                format!("/home/agent/.room/personality-{}.txt", agent.name),
                &personality,
            ),
        ] {
            // Ensure parent dir exists
            if let Some(parent) = std::path::Path::new(&path).parent() {
                let _ = Command::new("docker")
                    .args(["exec", "-u", "agent"])
                    .arg(container)
                    .args(["mkdir", "-p", &parent.to_string_lossy()])
                    .status();
            }

            let mut child = Command::new("docker")
                .args(["exec", "-i", "-u", "agent"])
                .arg(container)
                .args(["sh", "-c", &format!("cat > '{path}'")])
                .stdin(std::process::Stdio::piped())
                .spawn()
                .with_context(|| format!("failed to write {path}"))?;

            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                stdin.write_all(content.as_bytes())?;
            }

            child.wait()?;
        }

        eprintln!("  [{}] {} instructions written", agent.role, agent.name);
    }

    Ok(())
}

const TASKBOARD_INSTRUCTIONS: &str = r#"## Taskboard

### Task Lifecycle
Open → Claimed → Planned → InProgress → AwaitingReview → ReviewClaimed → Finished

### Commands

**For coders:**
- `/taskboard list` — view active tasks
- `/taskboard mine` — view your assigned tasks
- `/taskboard show <id>` — view task details
- `/taskboard claim <id>` — claim an open task
- `/taskboard plan <id> <plan>` — submit implementation plan
- `/taskboard update <id> <notes>` — post progress update (also renews lease)
- `/taskboard request_review <id>` — mark task ready for review (InProgress → AwaitingReview)
- `/taskboard release <id>` — unassign back to open

**For reviewers:**
- `/taskboard qa-queue` — view tasks awaiting review
- `/taskboard review_claim <id>` — claim a review (AwaitingReview → ReviewClaimed)
- `/taskboard approve <id>` — approve review (ReviewClaimed → Finished)
- `/taskboard reject <id> [reason]` — send back to coder (ReviewClaimed → InProgress)

**For managers:**
- `/taskboard post <description>` — create a new task
- `/taskboard assign <id> <agent>` — assign a task
- `/taskboard approve <id>` — approve a plan (Planned → InProgress)
- `/taskboard history` — view finished/cancelled tasks
- `/taskboard cancel <id> [reason]` — cancel a task

**Help:** `/taskboard help <subcommand>` for usage details.

### Workflow
1. `/taskboard list` or `/taskboard mine` — find work
2. `/taskboard claim <id>` — take a task
3. `/taskboard plan <id> <plan>` — submit your approach
4. Wait for `/taskboard approve` (Planned → InProgress)
5. `/taskboard update <id> <progress>` — report milestones
6. `/taskboard request_review <id>` — when PR is ready (InProgress → AwaitingReview)
7. Reviewer: `/taskboard review_claim <id>` → review → `/taskboard approve <id>` (→ Finished)

Always check the taskboard before asking for work. Never start work without claiming first.
Update progress regularly so the team knows what you're doing.

## Status

Use `/set_status <text>` to keep your presence status updated:
- `/set_status working on tb-003 — shell scaffold`
- `/set_status idle — waiting for tasks`
- `/set_status reviewing PR #42`

Update your status whenever you change what you're doing. **Check the room chat frequently** — always have a `room watch` running in background to stay aware of coordination messages, directives, and blockers. Dropping off the room without permission is a protocol violation.

## QA Review Guidelines

**Reviewers are the last line of defense.** Your job is to ensure no bugs, security issues, or regressions reach production.

- **Never label issues as "non-blocking" or "should be addressed in future PR".** All issues must be fixed before approval. Anything you let through will likely never be done.
- **Reject PRs for:** bugs, security issues, missing error handling, broken tests, logic errors, performance regressions, or documentation drift.
- **Approve with comments only for:** style nits, naming suggestions, minor improvements that don't affect correctness.
- **Review thoroughly:** check edge cases, test coverage, error handling, and documentation accuracy.
- **Wait for CI to pass** before starting review — do not review red PRs.
- **Use `/taskboard qa-queue`** to find tasks awaiting review — do not manually scan `/taskboard list`.

## Documentation

**Keep documentation up to date.** Before opening a PR, verify that any affected docs, README, or CLAUDE.md sections are accurate.

- Every PR description must include: `- [ ] Verified docs/README are accurate after this change (no drift)`
- If your change adds a new command, feature, or module, update the relevant documentation in the same PR.
- If you find documentation drift, fix it in the same PR or file an issue — do not merge PRs that introduce drift.

## Before Pushing

Run this checklist before every push:
1. Format: run the project's formatter (prettier, cargo fmt, etc.)
2. Lint: run the project's linter
3. Typecheck: run typecheck if applicable
4. Test: run the test suite
5. Verify the lockfile is up to date (no uncommitted changes)
6. **Check documentation accuracy** (see above)

Do NOT push and wait for CI to catch issues — run checks locally first.

## Rebase Recovery

When `git rebase` fails (e.g., due to linter hooks or complex conflicts):
1. Create a fresh branch from the target: `git checkout -b <new-branch> origin/main`
2. Cherry-pick your commits: `git cherry-pick <commit-hash>` (one at a time)
3. Run checks after each cherry-pick
4. Force-push to your PR branch if needed

This is standard procedure, not an edge case. Use it early rather than fighting rebases.

## Branch Ownership

- Never push to another agent's branch without asking in the room first
- Each agent works on their own feature branch
- If you need to build on another agent's work, branch from their branch or wait for merge

## When Idle

When all your tasks are done and the taskboard has no open work:
- Check for unmerged approved PRs that might need rebasing
- Run typecheck/lint on main to catch regressions
- Offer to help other agents with rebases or blockers
- Ask the manager if there's upcoming work to prepare for
- Do NOT silently poll — announce you're available in the room"#;

fn generate_role_instructions(name: &str, role: &crate::config::AgentRole, room: &str) -> String {
    use crate::config::AgentRole;

    let role_section = match role {
        AgentRole::Coder => {
            r#"## Role: Coder

You are a **coder** agent. Your primary responsibilities:

- Pick up tasks from the taskboard and implement them
- Write clean, tested, production-quality code
- Create feature branches for your work
- **Keep documentation up to date** — verify docs/README accuracy before opening PRs
- Push changes and create pull requests when work is complete
- Report progress and blockers to the room

### Workflow
1. `/taskboard list` or `/taskboard mine` to find work
2. `/taskboard claim <id>` to take an open task
3. `/taskboard plan <id> <your plan>` to submit your approach
4. Wait for manager to `/taskboard approve` (Planned → InProgress)
5. Implement on a feature branch
6. `/taskboard update <id> <progress>` as you hit milestones
7. Run tests and ensure CI passes locally
8. **Verify documentation accuracy** — check that any affected docs, README, or CLAUDE.md sections reflect your changes
9. Create a PR and `/taskboard request_review <id>` (InProgress → AwaitingReview)
10. After reviewer approves, task moves to Finished — pick up next task

### Documentation Duty
- Every PR description must include: `- [ ] Verified docs/README are accurate after this change (no drift)`
- If your change adds a new command, feature, or module, update the relevant documentation in the same PR.
- If you find documentation drift, fix it in the same PR or file an issue — do not merge PRs that introduce drift."#
        }

        AgentRole::Reviewer => {
            r#"## Role: Reviewer

You are a **reviewer** agent and the **last line of defense** before code reaches production. Your primary responsibilities:

- Review pull requests created by other agents
- Check code quality, correctness, test coverage, and documentation accuracy
- Leave constructive, specific feedback on PRs
- Approve PRs that meet quality standards — **anything you let through will likely never be fixed**
- Flag security issues, bugs, or architectural concerns

### Workflow
1. `/taskboard qa-queue` to find tasks awaiting review
2. `/taskboard review_claim <id>` to claim a review (AwaitingReview → ReviewClaimed)
3. Wait for CI to pass before starting review — do not review red PRs
4. Review the PR — check logic, correctness, edge cases, tests, error handling, documentation drift
5. Run the project's linter and test suite on the branch locally
6. Leave inline comments via `gh pr review`
7. `/taskboard approve <id>` to approve (ReviewClaimed → Finished)
8. Or `/taskboard reject <id> <reason>` to send back (ReviewClaimed → InProgress)

### Review Severity
- **Reject** (`/taskboard reject`): bugs, security issues, missing error handling, broken tests, logic errors, performance regressions, documentation drift
- **Approve with comments**: style nits, naming suggestions, minor improvements that don't affect correctness
- **Never label issues as "non-blocking" or "should be addressed in future PR".** All issues must be fixed before approval.
- Do NOT block PRs for trivial style or lint issues that don't affect correctness
- If a PR is good, approve it quickly — velocity matters

### QA as Last Line of Defense
- **You are responsible for preventing bugs from reaching production.** Assume anything you let through will never be addressed.
- **Check documentation accuracy:** ensure README, CLAUDE.md, and other docs reflect changes.
- **Review thoroughly:** edge cases, error handling, test coverage, security implications.
- **Wait for CI green:** never review a failing PR.

### Guidelines
- Do NOT write code or implement features yourself
- Use `/taskboard qa-queue` — do not manually scan `/taskboard list` for reviews
- Coordinate with other reviewers — check if a task is already ReviewClaimed
- Review base/dependency PRs before downstream PRs"#
        }

        AgentRole::Manager => {
            r#"## Role: Manager

You are a **manager/orchestrator** agent. Your primary responsibilities:

- Break down high-level goals into small, granular tasks on the taskboard
- Post tasks for agents to pick up (`/taskboard post <description>`)
- Let agents self-assign — only use `/taskboard assign` for dependent work
- Approve plans (`/taskboard approve <id>` when Planned → InProgress)
- Track progress and unblock stuck agents
- Coordinate between agents working on related features

### Workflow
1. Receive goals or feature requests from the human operator
2. Break them into small, independently testable tasks (one concern per task)
3. `/taskboard post <description>` for each task
4. Let agents `/taskboard claim` and `/taskboard plan`
5. `/taskboard approve <id>` to approve plans (Planned → InProgress)
6. Monitor with `/taskboard list`, `/taskboard mine`, `/taskboard history`
7. `/taskboard update <id> <note>` to add coordination notes
8. Help resolve blockers and coordinate reviews

### Guidelines
- Do NOT write code yourself — delegate to coders
- Keep tasks small and independently testable
- Ensure agents aren't working on conflicting changes
- Prefer letting agents self-assign — only assign directly when coordinating dependent work
- Use `/taskboard history` to track completed work
- Escalate to the human operator when decisions are needed"#
        }
    };

    format!(
        r#"# Agent Instructions

You are **{name}**, operating in room **{room}**.

{role_section}

{TASKBOARD_INSTRUCTIONS}

## Communication

Use the room to coordinate with other agents and the human operator:
- Send updates when you start/finish tasks
- Ask for help if you're blocked
- Respond to messages directed at you (@{name})
"#
    )
}

/// Generate a personality file for an agent (prepended to ralph's system prompt).
fn generate_personality_file(_name: &str, role: &crate::config::AgentRole) -> String {
    use crate::config::AgentRole;

    let role_prompt = match role {
        AgentRole::Coder => {
            "You are a software engineer agent. Your workflow:\n\
             1. `/taskboard list` or `/taskboard mine` — find work\n\
             2. `/taskboard claim <id>` — take an open task\n\
             3. `/taskboard plan <id> <plan>` — submit your approach\n\
             4. Wait for `/taskboard approve` (Planned → InProgress)\n\
             5. Implement on a feature branch\n\
             6. `/taskboard update <id> <progress>` — report milestones\n\
             7. Run tests and linter locally, open a PR\n\
             8. `/taskboard request_review <id>` (InProgress → AwaitingReview)\n\
             9. Reviewer approves → task Finished → pick up next task\n\n\
             Prefer small, focused changes. One concern per PR. Always use /taskboard commands."
        }
        AgentRole::Reviewer => {
            "You are a code review agent and the last line of defense. Your workflow:\n\
             1. `/taskboard qa-queue` — find tasks awaiting review\n\
             2. `/taskboard review_claim <id>` — claim a review (AwaitingReview → ReviewClaimed)\n\
             3. Wait for CI to pass — do not review red PRs\n\
             4. Review the PR — correctness, test coverage, error handling, edge cases, documentation accuracy\n\
             5. Leave feedback via `gh pr review`\n\
             6. `/taskboard approve <id>` — approve (ReviewClaimed → Finished)\n\
             7. Or `/taskboard reject <id> <reason>` — send back (→ InProgress)\n\n\
             You do not write feature code — you read and critique it.\n\
             Reject for: bugs, security, missing error handling, broken tests, logic errors, performance regressions, documentation drift.\n\
             Approve with comments for: style nits, naming, minor improvements that don't affect correctness.\n\
             **Never label issues as \"non-blocking\" or \"should be addressed in future PR\".** All issues must be fixed before approval.\n\
             Do NOT block PRs for trivial style/lint issues that don't affect correctness — velocity matters."
        }
        AgentRole::Manager => {
            "You are a coordination agent. Your workflow:\n\
             1. Receive goals from the human operator\n\
             2. Break them into small, granular tasks (one concern per task)\n\
             3. `/taskboard post <description>` — create tasks for agents\n\
             4. Let agents claim and plan — then `/taskboard approve <id>` (Planned → InProgress)\n\
             5. `/taskboard list` + `/taskboard history` — monitor progress\n\
             6. Help resolve blockers, coordinate reviews\n\n\
             Do NOT write code. Do NOT assign mega-tasks — keep tasks small and independently testable. \
             Let agents self-assign. Escalate to the human operator for architectural decisions."
        }
    };

    format!(
        "{role_prompt}\n\n\
         IMPORTANT: Always use `/taskboard` commands — never ask for work in the room \
         if the taskboard has tasks. Always claim before starting. Always update progress.\n\
         Use `/set_status <text>` frequently — update your status whenever you change activity.\n\
         Keep the room chat active: always have a `room watch` running in background to stay aware of coordination messages, directives, and blockers.\n\
         Before every push: format, lint, typecheck, test. Do NOT rely on CI.\n\
         Never push to another agent's branch without asking first.\n\
         When idle: check for stale PRs, run checks on main, announce availability.\n"
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
    Command::new("docker")
        .args(["exec", "-d", "-u", "agent"])
        .arg(container)
        .args(["room", "daemon"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .context("failed to start room daemon")?;

    // Brief pause for daemon to start
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Join as system user to get a token
    let join_output = Command::new("docker")
        .args(["exec", "-u", "agent"])
        .arg(container)
        .args(["room", "join", "system"])
        .output()
        .context("failed to join room")?;

    if !join_output.status.success() {
        let stderr = String::from_utf8_lossy(&join_output.stderr);
        eprintln!("warning: room join failed: {}", stderr.trim());
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&join_output.stdout);
    let token = parse_token(&stdout).context("failed to parse token from room join output")?;

    // Create room (idempotent — ignores "already exists")
    let create_output = Command::new("docker")
        .args(["exec", "-u", "agent"])
        .arg(container)
        .args(["room", "create", room, "-t", &token])
        .output()
        .context("failed to create room")?;

    if !create_output.status.success() {
        let stderr = String::from_utf8_lossy(&create_output.stderr);
        // "already exists" is fine
        if !stderr.contains("already exists") {
            eprintln!("warning: room create failed: {}", stderr.trim());
        }
    }

    Ok(())
}

fn parse_token(json: &str) -> Option<String> {
    let parsed: serde_json::Value = serde_json::from_str(json).ok()?;
    parsed.get("token")?.as_str().map(|s| s.to_string())
}

/// Build the room-ralph args for a given agent.
fn ralph_cmd_args(config: &Config, name: &str, ralph_args: &[String]) -> Vec<String> {
    let room = &config.room.default;
    let personality_file = format!("/home/agent/.room/personality-{name}.txt");

    let mut args = vec![
        "room-ralph".to_string(),
        room.to_string(),
        name.to_string(),
        "--personality".to_string(),
        personality_file,
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

        let role = config
            .get_agent(name)
            .map(|a| a.role.to_string())
            .unwrap_or_else(|| "coder".to_string());

        if status.success() {
            eprintln!("  started {name} ({role})");
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

        let role = config
            .get_agent(name)
            .map(|a| a.role.to_string())
            .unwrap_or_else(|| "coder".to_string());
        eprintln!("{prefix} starting in room '{room}' ({role})...");

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
