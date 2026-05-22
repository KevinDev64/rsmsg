use anyhow::{Result, anyhow};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use chacha20poly1305::{
    KeyInit, XChaCha20Poly1305, XNonce,
    aead::{Aead, generic_array::GenericArray},
};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use hkdf::Hkdf;
use rand::{RngCore, rngs::OsRng};
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};

#[derive(Clone, Copy)]
pub struct CryptoEngine;

pub struct X25519KeyPair {
    pub private_b64: String,
    pub public_b64: String,
}

pub struct Ed25519KeyPair {
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

    pub fn generate_ed25519_keypair(&self) -> Ed25519KeyPair {
        let signing = SigningKey::generate(&mut OsRng);
        let verifying = signing.verifying_key();
        Ed25519KeyPair {
            private_b64: STANDARD.encode(signing.to_bytes()),
            public_b64: STANDARD.encode(verifying.to_bytes()),
        }
    }

    pub fn sign_prekey_b64(
        &self,
        signing_private_b64: &str,
        prekey_public_b64: &str,
    ) -> Result<String> {
        let private = decode_key(signing_private_b64)?;
        let prekey = STANDARD
            .decode(prekey_public_b64)
            .map_err(|_| anyhow!("invalid prekey base64"))?;
        let signing = SigningKey::from_bytes(&private);
        let signature = signing.sign(&prekey_signature_payload(&prekey));
        Ok(STANDARD.encode(signature.to_bytes()))
    }

    pub fn verify_prekey_signature_b64(
        &self,
        signing_public_b64: &str,
        prekey_public_b64: &str,
        signature_b64: &str,
    ) -> Result<()> {
        let public = decode_key(signing_public_b64)?;
        let prekey = STANDARD
            .decode(prekey_public_b64)
            .map_err(|_| anyhow!("invalid prekey base64"))?;
        let signature_bytes = STANDARD
            .decode(signature_b64)
            .map_err(|_| anyhow!("invalid signature base64"))?;
        let verifying =
            VerifyingKey::from_bytes(&public).map_err(|_| anyhow!("invalid signing public key"))?;
        let signature = Signature::from_slice(&signature_bytes)
            .map_err(|_| anyhow!("invalid prekey signature"))?;
        verifying
            .verify(&prekey_signature_payload(&prekey), &signature)
            .map_err(|_| anyhow!("invalid prekey signature"))
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
        self.encrypt_bytes_to_b64(key_b64, plaintext.as_bytes())
    }

    pub fn encrypt_bytes_to_b64(&self, key_b64: &str, plaintext: &[u8]) -> Result<String> {
        let key = decode_key(key_b64)?;
        let cipher = XChaCha20Poly1305::new(GenericArray::from_slice(&key));
        let mut nonce_bytes = [0_u8; 24];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = XNonce::from_slice(&nonce_bytes);
        let mut out = nonce_bytes.to_vec();
        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|_| anyhow!("encryption failed"))?;
        out.extend_from_slice(&ciphertext);
        Ok(STANDARD.encode(out))
    }

    pub fn decrypt_text_from_b64(&self, key_b64: &str, envelope_b64: &str) -> Result<String> {
        let plaintext = self.decrypt_bytes_from_b64(key_b64, envelope_b64)?;
        Ok(String::from_utf8(plaintext).map_err(|_| anyhow!("invalid utf8 payload"))?)
    }

    pub fn decrypt_bytes_from_b64(&self, key_b64: &str, envelope_b64: &str) -> Result<Vec<u8>> {
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
        Ok(plaintext)
    }

    pub fn ratchet_step_b64(&self, chain_key_b64: &str) -> Result<(String, String)> {
        let chain_key = decode_key(chain_key_b64)?;
        let hk = Hkdf::<Sha256>::new(None, &chain_key);
        let mut message_key = [0_u8; 32];
        let mut next_chain_key = [0_u8; 32];
        hk.expand(b"rsmsg-message-key-v1", &mut message_key)
            .map_err(|_| anyhow!("hkdf expand failed"))?;
        hk.expand(b"rsmsg-chain-key-v1", &mut next_chain_key)
            .map_err(|_| anyhow!("hkdf expand failed"))?;
        Ok((
            STANDARD.encode(message_key),
            STANDARD.encode(next_chain_key),
        ))
    }
}

fn prekey_signature_payload(prekey_public: &[u8]) -> Vec<u8> {
    let mut payload = b"rsmsg-signed-prekey-v1".to_vec();
    payload.extend_from_slice(prekey_public);
    payload
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
