# room-sandbox — Design Document

## Overview

`room-sandbox` is a Rust CLI tool (`cargo install room-sandbox`) that sets up isolated, Dockerized multi-agent development environments using `room` and `room-ralph`. Each agent gets its own clone of a target repo, runs in a shared container, and communicates via a local room daemon.

It replaces the ad-hoc shell scripts in `/Volumes/Knox/tools/sandbox/` with a structured, team-friendly tool.

---

## Architecture

### Directory Layout (after `room-sandbox init`)

```
my-sandbox/                    # user creates this, cd's into it (or runs init inside an existing repo)
  sandbox.toml                 # source of truth — committed, shareable with team
  .room-sandbox/               # all generated artifacts — gitignored
    .sandbox-state.json        # last-applied config hashes
    .env                       # secrets (API keys, tokens)
    Dockerfile
    docker-compose.yml
    entrypoint.sh
    claude-wrapper.sh
    workspaces/
      r2d2/                    # independent git clone of target repo
      bumblebee/
      saphire/
```

`init` adds `.room-sandbox/` to `.gitignore` automatically. Only `sandbox.toml` is meant to be committed.

### Running inside an existing repo

If `room-sandbox init` is run inside a git repository, the wizard detects this and:

1. Infers `project.repo` from the existing remote (`git remote get-url origin`).
2. Auto-detects languages from marker files already present (`Cargo.toml`, `package.json`, etc.).
3. Tells the user: "Detected git repo `org/project` — using as sandbox target."
4. Still asks for confirmation / lets user override.
5. Writes `sandbox.toml` at the repo root, adds `.room-sandbox/` to `.gitignore`.

This way a team can check `sandbox.toml` into their project repo and anyone can `room-sandbox apply` to get a working sandbox.

### Config File — `sandbox.toml`

```toml
[project]
repo = "git@github.com:org/project.git"
container_name = "sandbox-my-sandbox"   # derived from directory name by default

[agents]
names = ["r2d2", "bumblebee", "saphire"]

[room]
default = "dev"

[auth]
method = "gh-cli"       # "gh-cli" | "pat" | "ssh"
mount_ssh = true        # whether to mount ~/.ssh read-only into the container

[environment]
languages = ["rust", "node"]        # "rust" | "node" | "python"
tools = ["claude", "gemini"]        # "claude" | "codex" | "gemini" | "glow"
```

---

## State Drift Detection — `check_state()`

A shared function used by all commands (except `init` and `clean`) to compare `sandbox.toml` against `.room-sandbox/.sandbox-state.json` and detect unapplied changes.

Returns which sections have drifted and what specifically changed. Commands use this contextually:

| Command type | Behavior on drift |
|---|---|
| **Read-only** (`agent list`, `logs`) | Warn: "sandbox.toml has unapplied changes — run `room-sandbox apply`" but proceed |
| **Runtime** (`shell`, `agent start`, `tui`, `up`) | Error if drift affects the requested operation (e.g. `shell r2d2` but r2d2 was just added and never cloned). Warn and proceed if drift is unrelated (e.g. environment change doesn't block shell access) |
| **Mutating** (`agent add`, `agent remove`) | Show the full picture: pending drift + the new mutation combined into one apply plan. Confirm before applying everything. User never gets a partial apply |

**Example — mutating command with existing drift:**

```
room-sandbox agent add wall-e

Unapplied changes detected:

  [environment]
  + python

Adding wall-e will also apply pending changes:

  Actions:
    1. Clone workspace for wall-e
    2. Write project-scoped CLAUDE.md for wall-e
    3. Rebuild container image (environment changed)
    4. Recreate container

Apply all changes? [y/N]
```

---

## Commands

### `room-sandbox init`

Interactive wizard that scaffolds a new sandbox environment.

**Repo URL normalization:**

The `--repo` flag (or wizard prompt) accepts three formats:

| Input | Resolved URL |
|---|---|
| `org/repo` (short form) | `git@github.com:org/repo.git` if auth is `ssh`, `https://github.com/org/repo.git` if auth is `gh-cli` or `pat` |
| `git@github.com:org/repo.git` | used as-is |
| `https://github.com/org/repo.git` | used as-is |

Short form assumes GitHub. Other hosts are not supported yet but the design leaves room for a future `[git] host` config field.

The resolved URL is what gets stored in `sandbox.toml`.

**Flow:**

1. **Detect context:**
   - If inside a git repo: infer `project.repo` from remote, auto-detect languages, inform the user.
   - If directory is empty: proceed with manual setup.
   - If directory is non-empty and not a git repo: refuse (ambiguous state).
   - If `sandbox.toml` already exists: refuse (use `apply` instead).
2. **Repo** — if not auto-detected, prompt for git URL (accepts short form `org/repo`, SSH, or HTTPS). Do a shallow clone (`--depth 1`) to a temp dir to auto-detect the stack.
3. **Environment — languages** — auto-detect from repo markers (`Cargo.toml` → rust, `package.json` → node, `requirements.txt`/`pyproject.toml` → python). Present detected + available, let user confirm/modify.
4. **Environment — tools** — multi-select from: claude, codex, gemini, glow. No auto-detection.
5. **Agents** — prompt for agent names. Default: `["r2d2", "bumblebee", "saphire"]`.
6. **Auth** — run `gh auth status` to show detected account. Options:
   - Use detected gh CLI token (show which account)
   - Provide a PAT manually
   - SSH only (no token)
   - If user picks gh-cli but has multiple accounts, warn and let them confirm or switch to PAT.
7. **SSH** — ask whether to mount `~/.ssh` into the container (default: yes if auth method is ssh, no if pat).
8. **Room** — prompt for default room name (default: `"dev"`).
9. Delete the temp shallow clone (if one was made).
10. Write `sandbox.toml` at the current directory root.
11. Create `.room-sandbox/` and add it to `.gitignore`.
12. Write `.room-sandbox/.env` (with secrets).
13. Generate Docker assets (`Dockerfile`, `docker-compose.yml`, `entrypoint.sh`, `claude-wrapper.sh`) into `.room-sandbox/` from templates + config.
14. Clone agent workspaces into `.room-sandbox/workspaces/<name>/`.
15. Write project-scoped CLAUDE.md for each agent (room usage instructions).
16. `docker compose build` + `docker compose up -d` (using `.room-sandbox/docker-compose.yml`).
17. Write `.room-sandbox/.sandbox-state.json` with applied config hashes.

**Non-interactive mode:**

```
room-sandbox init \
  --repo git@github.com:org/project.git \
  --agents r2d2,bumblebee \
  --languages rust,node \
  --tools claude \
  --auth gh-cli \
  --room dev
```

Skips the wizard, uses provided values + sensible defaults for anything omitted.

---

### `room-sandbox apply`

Reads `sandbox.toml`, diffs against `.room-sandbox/.sandbox-state.json`, shows a plan, and applies on confirmation.

**Diff logic per section:**

| Config change | Actions |
|---|---|
| `agents.names` added | Clone repo into `.room-sandbox/workspaces/<name>/`, write project-scoped CLAUDE.md |
| `agents.names` removed | Delete `.room-sandbox/workspaces/<name>/`, remove project-scoped CLAUDE.md |
| `environment` changed | Regenerate Dockerfile, rebuild Docker image, recreate container |
| `auth.method` changed | Update `.env` with new token, restart container |
| `auth.mount_ssh` changed | Regenerate `docker-compose.yml`, recreate container |
| `project.repo` changed | **Destructive** — wipe all workspaces, re-clone from new repo |
| `project.container_name` changed | Stop old container, start with new name |
| `room.default` changed | Update project-scoped CLAUDE.md templates only |

**Output format (terraform-style):**

```
room-sandbox apply

Changes detected:

  [environment]
  + python
  - rust

  [agents]
  + wall-e (clone git@github.com:org/project.git)

  Actions:
    1. Clone workspace for wall-e
    2. Write project-scoped CLAUDE.md for wall-e
    3. Rebuild container image (environment changed)
    4. Recreate container

Apply these changes? [y/N]
```

**Destructive changes get extra emphasis:**

```
  [project]
  ~ repo: git@github.com:org/old.git → git@github.com:org/new.git

  ⚠ This will DELETE all existing workspaces and re-clone.

  Actions:
    1. Remove workspaces: r2d2, bumblebee, saphire
    2. Clone 3 workspaces from new repo
    ...
```

Always requires explicit `y` confirmation. No `--yes` / `--force` flag.

---

### `room-sandbox agent add <name>`

Add an agent to the sandbox.

1. Validate agent name doesn't already exist in `sandbox.toml`.
2. Run `check_state()` — if there's existing drift, include it in the plan.
3. Update `sandbox.toml` — append name to `agents.names`.
4. Show combined plan (pending drift + new agent clone). Confirm with user.
5. Apply: clone repo into `.room-sandbox/workspaces/<name>/`, write project-scoped CLAUDE.md, plus any other pending changes.
6. Update `.sandbox-state.json`.

### `room-sandbox agent remove <name>`

Remove an agent from the sandbox.

1. Validate agent name exists in `sandbox.toml`.
2. Run `check_state()` — if there's existing drift, include it in the plan.
3. Update `sandbox.toml` — remove name from `agents.names`.
4. Show combined plan (pending drift + workspace deletion). Confirm with user.
5. Apply: delete `.room-sandbox/workspaces/<name>/`, remove project-scoped CLAUDE.md, plus any other pending changes.
6. Update `.sandbox-state.json`.

### `room-sandbox agent list`

List all agents defined in `sandbox.toml` with status:

```
NAME        STATUS
r2d2        running   (pid 1234)
bumblebee   ready
saphire     stopped
wall-e      missing   (run apply)
```

Statuses:
- **running** — ralph process active in container
- **ready** — workspace exists, not running
- **stopped** — workspace exists, was previously running
- **missing** — in toml but workspace not cloned (needs `apply`)

### `room-sandbox agent start <name> [names...] [-- extra-ralph-args...]`

Start ralph agent loops. Room is always read from `sandbox.toml` `room.default`.

1. Run `check_state()` — if drift exists, show plan, confirm, apply first.
2. Validate each agent exists in toml + workspace exists on disk.
3. Ensure container is running — auto-`up` if not.
4. For each agent, check if already running — error individually ("agent 'r2d2' is already running — use `agent restart r2d2`").
5. Start `room daemon` (idempotent).
6. Join agent, create room, subscribe (all idempotent).
7. Exec ralph loop: `room-ralph <room> <name> --allow-all [extra-args]`.

Example:
```
room-sandbox agent start r2d2
room-sandbox agent start r2d2 bumblebee saphire
room-sandbox agent start r2d2 -- --model sonnet --issue 42
```

Running detection: `docker exec <container> pgrep -f "room-ralph.*<name>"`.

### `room-sandbox agent stop <name> [names...]`

Kill the ralph process for the named agent(s) inside the container. No confirmation.

```
room-sandbox agent stop r2d2
room-sandbox agent stop r2d2 bumblebee
```

### `room-sandbox agent restart <name> [names...]`

Stop + start. Accepts the same `[-- extra-ralph-args...]` passthrough.

```
room-sandbox agent restart r2d2
room-sandbox agent restart r2d2 -- --model sonnet
```

---

### `room-sandbox tui [--as <user>]`

Open the room TUI inside the container. Room setup (daemon, room creation) is handled by `init`/`apply`, not here.

1. Run `check_state()` — warn on drift, but don't block.
2. Ensure container is running — auto-`up` if not.
3. `docker exec -it -u agent <container> room tui <room> --as <user>`.

Defaults: room from `sandbox.toml` `room.default`, user from `$(whoami)`.

```
room-sandbox tui              # join as $(whoami)
room-sandbox tui --as helix   # override username
```

---

### `room-sandbox shell [name] [--root]`

Open a bash shell inside the container.

```
room-sandbox shell              # shell as agent user, cwd /workspaces
room-sandbox shell r2d2         # shell as agent user, cwd /workspaces/r2d2
room-sandbox shell --root       # shell as root, cwd /workspaces
room-sandbox shell r2d2 --root  # shell as root, cwd /workspaces/r2d2
```

1. Ensure the container is running.
2. Run `check_state()` — warn on unrelated drift, error if the named agent was added but never cloned.
3. If `name` is provided, validate it exists in `sandbox.toml` AND workspace exists on disk. Error with available agents if not found.
4. `docker exec -it -u {root|agent} -w /workspaces/{name|.} <container> bash`

---

### `room-sandbox up`

Start the sandbox container (if not already running).

```
docker compose -f .room-sandbox/docker-compose.yml up -d
```

Fails if `.room-sandbox/` doesn't exist (not initialized).

### `room-sandbox down`

Stop the sandbox container without destroying anything.

```
docker compose -f .room-sandbox/docker-compose.yml down
```

Workspaces, config, and volumes are preserved. `up` resumes where you left off.

### `room-sandbox logs`

Tail the sandbox container logs.

```
docker compose -f .room-sandbox/docker-compose.yml logs -f
```

### `room-sandbox clean`

Remove all generated artifacts, keeping only `sandbox.toml`.

1. Show what will be deleted: `.room-sandbox/` (workspaces, Docker assets, state, env).
2. Stop and remove the container if running.
3. Confirm with user.
4. Delete `.room-sandbox/`.
5. `sandbox.toml` remains — user can `room-sandbox apply` to rebuild from scratch.

---

## Agent Instruction Injection

Each agent gets room usage instructions via Claude Code's project-scoped config at:
```
~/.claude/projects/<workspace-path-hash>/CLAUDE.md
```

This is written by the entrypoint (on container start) and by `agent add`. Content is templated from `sandbox.toml`:

```markdown
# Room Agent Instructions

You are agent `{agent_name}` in room `{room_name}`.

## Communication
- `room send <message>` — send a message to the room
- `room poll` — check for new messages
- ...
```

This approach has zero impact on the target repo — no files added, no git diff noise.

---

## Docker Assets

All Docker assets live inside `.room-sandbox/` and are generated from `sandbox.toml`.

### Dockerfile

Uses build args for feature flags. No commenting in/out — conditional `RUN` blocks:

```dockerfile
ARG INSTALL_RUST=false
ARG INSTALL_NODE=false
ARG INSTALL_PYTHON=false
ARG INSTALL_GLOW=false
ARG INSTALL_CLAUDE=false
ARG INSTALL_CODEX=false
ARG INSTALL_GEMINI=false

FROM rust:bookworm

# Base dependencies (always installed)
RUN apt-get update && apt-get install -y \
    git openssh-client curl jq tmux pkg-config libssl-dev gosu sqlite3 \
    && rm -rf /var/lib/apt/lists/*

# Conditional: Node.js
RUN if [ "$INSTALL_NODE" = "true" ]; then \
      curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
      && apt-get install -y nodejs \
      && corepack enable && corepack prepare yarn@1.22.22 --activate \
      && npm install -g turbo; \
    fi

# Conditional: Python
RUN if [ "$INSTALL_PYTHON" = "true" ]; then \
      apt-get update && apt-get install -y python3 python3-pip python3-venv \
      && rm -rf /var/lib/apt/lists/*; \
    fi

# Conditional: glow
RUN if [ "$INSTALL_GLOW" = "true" ]; then \
      curl -fsSL https://github.com/charmbracelet/glow/releases/download/v2.1.0/glow_2.1.0_linux_amd64.tar.gz \
      | tar xz -C /usr/local/bin glow; \
    fi

# Conditional: Claude Code
RUN if [ "$INSTALL_CLAUDE" = "true" ]; then \
      npm install -g @anthropic-ai/claude-code; \
    fi

# ... etc for codex, gemini

# room + room-ralph (always installed)
RUN cargo install room-cli room-ralph
```

The CLI generates `docker-compose.yml` with the correct `build.args` based on `sandbox.toml`.

### docker-compose.yml

Generated from config. Auth/SSH mounts are conditional:

```yaml
services:
  sandbox:
    build:
      context: .
      args:
        INSTALL_RUST: "true"
        INSTALL_NODE: "true"
        # ... from sandbox.toml [environment]
    container_name: sandbox-my-sandbox
    volumes:
      - ./workspaces:/mnt/sandbox-root
      - claude-data:/home/agent/.claude
      - cargo-cache:/usr/local/cargo/registry
      # conditional:
      - ~/.ssh:/home/agent/.ssh-host:ro    # only if mount_ssh = true
    env_file: .env
    # ...
```

---

## State Tracking — `.room-sandbox/.sandbox-state.json`

Written after every successful apply. Used to diff against current `sandbox.toml`.

```json
{
  "applied_at": "2026-03-19T18:50:00Z",
  "config_hashes": {
    "project": "a1b2c3",
    "agents": "d4e5f6",
    "room": "g7h8i9",
    "auth": "j0k1l2",
    "environment": "m3n4o5"
  }
}
```

Each hash is computed from the serialized section content. `apply` compares current hashes to stored hashes to determine what changed.
