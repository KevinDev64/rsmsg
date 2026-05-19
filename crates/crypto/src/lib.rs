use anyhow::Result;

pub struct CryptoEngine;

impl CryptoEngine {
    pub fn new() -> Self {
        Self
    }

    pub fn healthcheck(&self) -> Result<()> {
        Ok(())
    }
}
