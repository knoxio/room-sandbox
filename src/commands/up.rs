use anyhow::{Result, bail};

use crate::config;
use crate::docker;

pub fn run() -> Result<()> {
    if !config::sandbox_dir().exists() {
        bail!("not initialized — run `room-sandbox init` first");
    }
    docker::up()
}
