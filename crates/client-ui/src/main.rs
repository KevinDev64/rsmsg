use anyhow::Result;
use client_core::ClientCore;

fn main() -> Result<()> {
    let core = ClientCore::new();
    core.healthcheck()?;
    Ok(())
}
