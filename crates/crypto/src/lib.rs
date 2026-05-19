use anyhow::{Result, anyhow};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use chacha20poly1305::{
    KeyInit, XChaCha20Poly1305, XNonce,
    aead::{Aead, generic_array::GenericArray},
};
use hkdf::Hkdf;
use rand::{RngCore, rngs::OsRng};
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};

pub struct CryptoEngine;

pub struct X25519KeyPair {
    pub private_b64: String,
    pub public_b64: String,
}

impl CryptoEngine {
    pub fn new() -> Self {
        Self
    }

    pub fn healthcheck(&self) -> Result<()> {
        Ok(())
    }

    pub fn generate_shared_key_b64(&self) -> String {
        let mut key = [0_u8; 32];
        OsRng.fill_bytes(&mut key);
        STANDARD.encode(key)
    }

    pub fn generate_x25519_keypair(&self) -> X25519KeyPair {
        let secret = StaticSecret::random_from_rng(OsRng);
        let public = PublicKey::from(&secret);
        X25519KeyPair {
            private_b64: STANDARD.encode(secret.to_bytes()),
            public_b64: STANDARD.encode(public.as_bytes()),
        }
    }

    pub fn derive_shared_key_b64(
        &self,
        own_private_b64: &str,
        peer_public_b64: &str,
    ) -> Result<String> {
        let own_private = decode_key(own_private_b64)?;
        let peer_public = decode_key(peer_public_b64)?;
        let secret = StaticSecret::from(own_private);
        let peer = PublicKey::from(peer_public);
        let dh = secret.diffie_hellman(&peer);

        let hk = Hkdf::<Sha256>::new(None, dh.as_bytes());
        let mut key = [0_u8; 32];
        hk.expand(b"rsmsg-chat-key", &mut key)
            .map_err(|_| anyhow!("hkdf expand failed"))?;
        Ok(STANDARD.encode(key))
    }

    pub fn encrypt_text_to_b64(&self, key_b64: &str, plaintext: &str) -> Result<String> {
        let key = decode_key(key_b64)?;
        let cipher = XChaCha20Poly1305::new(GenericArray::from_slice(&key));
        let mut nonce_bytes = [0_u8; 24];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = XNonce::from_slice(&nonce_bytes);
        let mut out = nonce_bytes.to_vec();
        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|_| anyhow!("encryption failed"))?;
        out.extend_from_slice(&ciphertext);
        Ok(STANDARD.encode(out))
    }

    pub fn decrypt_text_from_b64(&self, key_b64: &str, envelope_b64: &str) -> Result<String> {
        let key = decode_key(key_b64)?;
        let cipher = XChaCha20Poly1305::new(GenericArray::from_slice(&key));
        let envelope = STANDARD
            .decode(envelope_b64)
            .map_err(|_| anyhow!("invalid envelope base64"))?;
        if envelope.len() < 24 {
            return Err(anyhow!("invalid envelope length"));
        }
        let (nonce_bytes, ciphertext) = envelope.split_at(24);
        let nonce = XNonce::from_slice(nonce_bytes);
        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| anyhow!("decryption failed"))?;
        Ok(String::from_utf8(plaintext).map_err(|_| anyhow!("invalid utf8 payload"))?)
    }
}

fn decode_key(key_b64: &str) -> Result<[u8; 32]> {
    let bytes = STANDARD
        .decode(key_b64)
        .map_err(|_| anyhow!("invalid key base64"))?;
    if bytes.len() != 32 {
        return Err(anyhow!("key must be 32 bytes"));
    }
    let mut key = [0_u8; 32];
    key.copy_from_slice(&bytes);
    Ok(key)
}
