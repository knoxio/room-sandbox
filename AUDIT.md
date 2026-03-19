# Code Audit: room-sandbox

## Anti-Patterns Verdict
**PASS with observations.** The codebase does not exhibit the typical "AI slop" visual tells (as it is a CLI tool), and the logic is generally well-structured. However, there are some systemic security anti-patterns regarding shell command construction and container permissions that need addressing.

## Executive Summary
- **Total Issues Found**: 8
- **Critical**: 2
- **High**: 2
- **Medium**: 3
- **Low**: 1
- **Overall Quality Score**: 7/10
- **Recommended Next Steps**: Address command injection vulnerabilities in `src/docker.rs` immediately, followed by hardening the Docker container security profile.

---

## Detailed Findings by Severity

### Critical Issues

#### 1. Command Injection in `kill_agent`
- **Location**: `src/docker.rs`, `kill_agent` function
- **Severity**: Critical
- **Category**: Security
- **Description**: The `kill_agent` function uses `format!` to build a bash script containing the agent's `name`, which is then executed via `bash -c` inside the container. If an agent name contains shell metacharacters (e.g., `; rm -rf /`), they will be executed.
- **Impact**: Arbitrary command execution within the container as the `agent` user.
- **Recommendation**: Sanitize the agent name or use a safer way to find and kill processes (e.g., passing arguments directly to `pgrep` and `kill` without a shell wrapper).
- **Suggested command**: `/harden`

#### 2. Insecure Docker Socket Mounting
- **Location**: `src/docker.rs`, `generate_compose` function
- **Severity**: Critical
- **Category**: Security
- **Description**: When the `Docker` utility is enabled, `/var/run/docker.sock` is mounted into the container.
- **Impact**: Mounting the Docker socket gives the container (and any agent running within it) full root access to the host machine. A malicious or compromised agent could escape the sandbox.
- **Recommendation**: Use a Docker-in-Docker (DinD) sidecar or a restricted proxy if Docker access is required, and warn the user explicitly about the risks.
- **Suggested command**: `/harden`

### High-Severity Issues

#### 1. Command Injection in `inject_agent_instructions`
- **Location**: `src/docker.rs`, `inject_agent_instructions` function
- **Severity**: High
- **Category**: Security
- **Description**: Uses `format!` to construct a shell command: `sh -c "cat > '{claude_md_path}'"`.
- **Impact**: While `claude_md_path` is derived from the agent name (which is somewhat controlled), it still presents a risk if agent names are not strictly validated.
- **Recommendation**: Use `docker cp` or a more direct way to write files into the container.
- **Suggested command**: `/harden`

#### 2. Insecure SSH Key Handling in `entrypoint.sh`
- **Location**: `src/docker.rs`, `generate_entrypoint` function
- **Severity**: High
- **Category**: Security
- **Description**: The entrypoint script copies the *entire* host `~/.ssh` directory into the container if `mount_ssh` is enabled.
- **Impact**: This exposes all private keys, including those not needed for the sandbox, to the container environment.
- **Recommendation**: Only mount or copy specific keys required for the sandbox, or use `ssh-agent` forwarding.
- **Suggested command**: `/harden`

### Medium-Severity Issues

#### 1. Unoptimized Dockerfile Layers
- **Location**: `src/docker.rs`, `generate_dockerfile` function
- **Severity**: Medium
- **Category**: Performance
- **Description**: Multiple `RUN apt-get update` calls across different conditional sections.
- **Impact**: Larger image sizes and slower build times because layers cannot be efficiently cached or combined.
- **Recommendation**: Consolidate `apt-get install` calls into a single layer where possible.
- **Suggested command**: `/optimize`

#### 2. Brittle Token Parsing
- **Location**: `src/docker.rs`, `parse_token` function
- **Severity**: Medium
- **Category**: Quality / Resilience
- **Description**: The `parse_token` function uses manual string manipulation (indices and `find`) to parse JSON output.
- **Impact**: Prone to breaking if the JSON format changes slightly (e.g., extra whitespace or different field ordering).
- **Recommendation**: Use `serde_json` to parse the output properly.
- **Suggested command**: `/harden`

#### 3. Lack of Error Handling in Room Initialization
- **Location**: `src/docker.rs`, `ensure_room` function
- **Severity**: Medium
- **Category**: Quality
- **Description**: Several `Command` calls use `let _ = ...` or ignore the status code.
- **Impact**: Failures in starting the room daemon or creating the default room will go unnoticed by the CLI, leading to a broken state.
- **Recommendation**: Check return statuses and return `Result` from these operations.
- **Suggested command**: `/harden`

### Low-Severity Issues

#### 1. Hard-coded User/Group IDs
- **Location**: `src/docker.rs`, `generate_dockerfile` and `generate_entrypoint`
- **Severity**: Low
- **Category**: Portability
- **Description**: UID `1000` is hard-coded for the `agent` user.
- **Impact**: May cause permission issues on Linux hosts where the user's UID is not 1000.
- **Recommendation**: Allow configuring the UID/GID or detect it at runtime.
- **Suggested command**: `/normalize`

---

## Patterns & Systemic Issues
- **Shell-out Pattern**: The project relies heavily on `std::process::Command` with shell wrappers (`sh -c`, `bash -c`). This is a recurring source of both security risks (injection) and portability/reliability issues.
- **Loose Permissions**: The sandbox design prioritizes ease of use (e.g., mounting host SSH and Docker socket) over strict isolation.

## Positive Findings
- **Clean Architecture**: The separation between CLI commands, config, state, and Docker logic is well-defined.
- **State Management**: The use of config hashing to detect drift is a robust way to manage environment synchronization.
- **User Experience**: The use of `inquire` for interactive wizards provides a high-quality CLI experience.

## Recommendations by Priority
1. **Immediate**: Sanitize agent names or refactor `docker.rs` to avoid `sh -c` and `bash -c` with interpolated strings.
2. **Short-term**: Refactor `parse_token` to use `serde_json`.
3. **Medium-term**: Implement a more secure SSH key handling mechanism (e.g., selective mounting).
4. **Long-term**: Consolidate Dockerfile layers to improve build performance.

## Suggested Commands for Fixes
- Use `/harden` to address the command injection and insecure mounting issues.
- Use `/optimize` to consolidate Dockerfile layers.
- Use `/polish` to improve the token parsing and error handling in `docker.rs`.
