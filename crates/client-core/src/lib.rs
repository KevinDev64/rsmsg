use anyhow::Result;
use crypto::CryptoEngine;

pub struct ClientCore {
    crypto: CryptoEngine,
}

impl ClientCore {
    pub fn new() -> Self {
        Self {
            crypto: CryptoEngine::new(),
        }
    }

    pub fn healthcheck(&self) -> Result<()> {
        self.crypto.healthcheck()
    }
}
