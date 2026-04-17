# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed
- Migrated CI workflows to Blacksmith runners

## [1.5.0] - 2026-03-21

### Changed
- Updated all agent prompts to room v3.6.0 taskboard API
- `/taskboard review` → `/taskboard request_review`
- Reviewer workflow: `qa-queue` → `review_claim` → `approve`/`reject`
- Manager workflow: added `history` and `mine` for tracking
- Full lifecycle documented: Open → Claimed → Planned → InProgress → AwaitingReview → ReviewClaimed → Finished
- Strengthened QA guidelines: reviewers are the last line of defense, must fix all issues before approval, never label as "non-blocking" or defer to future PR
- Added documentation accuracy requirements: coders must verify docs/README accuracy before opening PRs, reviewers must check for documentation drift
- `apply` auto-starts container when down (no drift needed)
- `agent remove` directly deletes workspace

### Fixed
- Claude Code installs as native binary (no longer npm)
- Node.js only installed when needed (node language or playwright utility)

## [1.4.0] - 2026-03-20

### Added
- Correct `/taskboard` command reference in agent prompts (post, claim, plan, approve, update, review, finish, cancel)
- `/set_status` instructions for agent presence updates
- Agent personality files always injected on `apply` and `agent start`
- Claude auth warning shown after container rebuild
- `agent start --all` skips already-running agents instead of erroring

### Fixed
- Auto-tag uses PAT (`RELEASE_TOKEN`) so tag push triggers the release workflow
- `clean` uses `docker rm -f` directly instead of compose (no `.env` dependency, handles stopped containers)
- `apply` regenerates `.env` when missing (clean → apply flow)
- Personality files always written (not gated on drift type)

## [1.3.0] - 2026-03-20

### Added
- Auto-tag workflow: merging `release/v*` branches auto-creates git tags
- Release validation CI: checks Cargo.toml version matches branch, changelog has entry, tag doesn't exist
- Custom personality files with `/taskboard` instructions per agent role

### Fixed
- `apply` regenerates `.env` if missing (fixes apply-after-clean)
- Docker volumes isolated per sandbox via compose project name

## [1.2.0] - 2026-03-20

### Fixed
- Docker volumes now isolated per sandbox (container_name used as compose project name)
- `clean` removes Docker volumes to prevent room history leaking between sandboxes

### Added
- Custom personality files with `/taskboard` instructions per agent role
- Personality files passed to room-ralph via `--personality` flag

## [1.1.0] - 2026-03-20

### Changed
- Default agents: r2d2, c3po, wall-e, qa, manager
- Init shows clear Claude Code authentication warning with login instructions

### Fixed
- Cargo publish token handling in release workflow
- Added license and repository fields to Cargo.toml for crates.io
- Removed broken credential seeding (credentials file only contained MCP OAuth)
- `apply` regenerates `.env` and clones workspaces after `clean`

### Security
- SSH agent forwarding instead of key copying (private keys never enter container)
- Agent name validation to prevent command injection
- Host UID detection for container user mapping
- Proper error handling in room initialization

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
