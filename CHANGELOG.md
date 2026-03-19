# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-03-19

### Added
- Interactive `init` wizard with repo auto-detection, gh multi-account support, and language/utility selection
- `apply` command with terraform-style drift detection, diff display, and confirmation
- Agent management: `agent add`, `agent remove`, `agent list`, `agent start`, `agent stop`, `agent restart`
- Role-based agents (coder, reviewer, manager) mapping to room-ralph personalities
- `--all` flag for start/stop/restart to operate on all agents
- `-t` / `--tail` flag to stream multiplexed colored agent output
- `tui` command to open the room TUI
- `shell` command with `--root` flag and optional agent workspace targeting
- `claude` command to run Claude Code interactively in agent workspaces with arg passthrough
- `up`, `down`, `logs`, `clean` container lifecycle commands
- Modular Dockerfile with selectable languages (rust, node, python) and utilities (glow, playwright, just, mise, proto, pulumi, ansible, aws-cli, terraform, docker, kubectl, yq)
- SSH agent forwarding (private keys never enter the container)
- Claude Code auth seeding from host credentials
- Role-based CLAUDE.md injection into container project-scoped config
- Agent name validation to prevent command injection
- Host UID detection for container user mapping
- `.room-sandbox/` directory for all generated artifacts (gitignored)
- `sandbox.toml` as the single source of truth (committable, shareable)
- CI workflow (fmt, clippy, build, test)
- Release workflow (multi-platform builds, GitHub release, crates.io publish)
