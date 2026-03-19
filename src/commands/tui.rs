use anyhow::Result;

use crate::config::Config;
use crate::docker;
use crate::state;

pub fn run(username: Option<&str>) -> Result<()> {
    let config = Config::load()?;

    // Warn on drift but don't block
    let _ = state::warn_drift();

    // Ensure container is running
    docker::ensure_running(&config)?;

    let user = username.unwrap_or_else(|| {
        // Leak is fine here — we're about to exec and exit
        Box::leak(whoami().into_boxed_str())
    });
    let room = &config.room.default;

    docker::exec(&config, "agent", "/workspaces", &["room", room, user])
}

fn whoami() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "user".to_string())
}
