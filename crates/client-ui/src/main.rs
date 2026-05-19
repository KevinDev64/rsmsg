use anyhow::Result;
use client_core::{ClientConfig, ClientCore};

fn main() -> Result<()> {
    let core = ClientCore::new(ClientConfig::local_default());
    core.healthcheck()?;
    Ok(())
}
