# room-sandbox

Dockerized multi-agent sandbox for [room](https://github.com/knoxio/room). Sets up isolated development environments where multiple AI agents collaborate on a codebase via [room-ralph](https://github.com/knoxio/room-ralph).

Each agent gets its own clone of the target repo, runs inside a shared Docker container, and communicates through a room instance. Agents are assigned roles (coder, reviewer, manager) that determine their behavior and tool access.

## Install

```bash
cargo install room-sandbox
```

## Quick Start

```bash
mkdir my-sandbox && cd my-sandbox
room-sandbox init
```

The interactive wizard walks you through:
- **Repo** — git URL, SSH, HTTPS, or shorthand (`org/repo`)
- **Languages** — rust, node, python (auto-detected from repo)
- **Utilities** — glow, playwright, just, mise, terraform, docker, kubectl, etc.
- **Agents** — names and roles (coder, reviewer, manager)
- **Auth** — GitHub account selection (supports multiple `gh` accounts), PAT, or SSH

Once initialized, start your agents:

```bash
room-sandbox agent start --all       # start all agents in background
room-sandbox tui                     # open the room TUI to observe and interact
```

## Config

Everything is driven by `sandbox.toml`, which can be committed to your project repo so teammates can `room-sandbox apply` to get a working sandbox:

```toml
[project]
repo = "git@github.com:org/project.git"
container_name = "sandbox-project"

[[agent]]
name = "r2d2"
role = "coder"

[[agent]]
name = "saphire"
role = "reviewer"

[[agent]]
name = "ba"
role = "manager"

[room]
default = "dev"

[auth]
method = "gh-cli"
mount_ssh = true
gh_account = "myaccount"

[environment]
languages = ["node", "rust"]
utilities = ["glow", "just", "docker"]
```

## Commands

### Setup

| Command | Description |
|---------|-------------|
| `room-sandbox init` | Interactive wizard — scaffolds sandbox.toml and builds everything |
| `room-sandbox apply` | Diff config against state, show plan, apply on confirmation |
| `room-sandbox clean` | Remove all generated artifacts, keep sandbox.toml |

### Agents

| Command | Description |
|---------|-------------|
| `room-sandbox agent add <name>` | Add an agent (prompts for role, auto-applies) |
| `room-sandbox agent remove <name>` | Remove an agent and its workspace |
| `room-sandbox agent list` | Show agents with role and status (running/ready/missing) |
| `room-sandbox agent start <names...>` | Start agents in background |
| `room-sandbox agent start --all` | Start all agents |
| `room-sandbox agent start -t <names...>` | Start and tail multiplexed output |
| `room-sandbox agent stop <names...>` | Stop agents (kills process group) |
| `room-sandbox agent stop --all` | Stop all agents |
| `room-sandbox agent restart <names...>` | Restart agents |

### Interactive

| Command | Description |
|---------|-------------|
| `room-sandbox tui` | Open the room TUI |
| `room-sandbox tui --as helix` | Join as a specific username |
| `room-sandbox shell [name]` | Bash into the container (optionally into an agent's workspace) |
| `room-sandbox shell --root` | Shell in as root |
| `room-sandbox claude <name>` | Run Claude Code interactively in an agent's workspace |
| `room-sandbox claude <name> -- --model sonnet` | Pass extra args to Claude |

### Container

| Command | Description |
|---------|-------------|
| `room-sandbox up` | Start the container |
| `room-sandbox down` | Stop the container |
| `room-sandbox logs` | Tail container logs |

## Agent Roles

Roles map to [room-ralph personalities](https://github.com/knoxio/room-ralph) which set system prompts and tool profiles:

| Role | Personality | Behavior |
|------|-------------|----------|
| **coder** | `coder` | Picks up tasks, writes code, runs tests, opens PRs |
| **reviewer** | `reviewer` | Reviews PRs, checks quality, leaves feedback — does not write code |
| **manager** | `coordinator` | Breaks down goals into tasks, manages the taskboard, coordinates agents |

## Directory Layout

```
my-sandbox/
  sandbox.toml              # config (committed)
  .room-sandbox/            # generated artifacts (gitignored)
    .sandbox-state.json
    .env
    Dockerfile
    docker-compose.yml
    entrypoint.sh
    claude-wrapper.sh
    workspaces/
      r2d2/                 # git clone of target repo
      saphire/
      ba/
```

## Utilities

The init wizard lets you select utilities to install in the container:

| Utility | Description |
|---------|-------------|
| glow | Markdown reader |
| playwright | Browser automation |
| just | Command runner |
| mise | Tool version manager |
| proto | Toolchain manager |
| pulumi | Infrastructure as code |
| terraform | Infrastructure as code |
| ansible | Automation (auto-installs python) |
| aws-cli | AWS command line |
| docker | Docker-in-Docker CLI |
| kubectl | Kubernetes CLI |
| yq | YAML processor |

## Prerequisites

- [Docker](https://docs.docker.com/get-docker/)
- [GitHub CLI](https://cli.github.com/) (optional, for auth)

## License

MIT
