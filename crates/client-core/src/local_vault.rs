use std::{fs, path::Path};

use anyhow::{Result, anyhow};
use argon2::Argon2;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use chacha20poly1305::{
    KeyInit, XChaCha20Poly1305, XNonce,
    aead::{Aead, generic_array::GenericArray},
};
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[derive(Clone)]
pub struct VaultKey([u8; 32]);

#[derive(Serialize, Deserialize)]
struct VaultFile {
    version: u8,
    salt_b64: String,
    nonce_b64: String,
    ciphertext_b64: String,
}

pub fn load_json<T: DeserializeOwned>(path: &str, password: Option<&str>) -> Option<T> {
    let file = Path::new(path);
    if !file.exists() {
        return None;
    }
    let raw = fs::read_to_string(file).ok()?;
    if let Ok(vault) = serde_json::from_str::<VaultFile>(&raw) {
        let password = password?;
        let plaintext = decrypt_vault_file(password, vault).ok()?;
        return serde_json::from_slice(&plaintext).ok();
    }
    serde_json::from_str(&raw).ok()
}

pub fn save_json<T: Serialize>(path: &str, value: &T, password: Option<&str>) -> Result<()> {
    let plaintext = serde_json::to_vec_pretty(value)?;
    crate::storage::ensure_parent(std::path::Path::new(path));
    if let Some(password) = password {
        let vault = encrypt_vault_file(password, &plaintext)?;
        fs::write(path, serde_json::to_string_pretty(&vault)?)?;
    } else {
        fs::write(path, plaintext)?;
    }
    Ok(())
}

fn encrypt_vault_file(password: &str, plaintext: &[u8]) -> Result<VaultFile> {
    let mut salt = [0_u8; 16];
    let mut nonce = [0_u8; 24];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut nonce);
    let key = derive_key(password, &salt)?;
    let cipher = XChaCha20Poly1305::new(GenericArray::from_slice(&key.0));
    let ciphertext = cipher
        .encrypt(XNonce::from_slice(&nonce), plaintext)
        .map_err(|_| anyhow!("local vault encryption failed"))?;
    Ok(VaultFile {
        version: 1,
        salt_b64: STANDARD.encode(salt),
        nonce_b64: STANDARD.encode(nonce),
        ciphertext_b64: STANDARD.encode(ciphertext),
    })
}

fn decrypt_vault_file(password: &str, vault: VaultFile) -> Result<Vec<u8>> {
    if vault.version != 1 {
        return Err(anyhow!("unsupported local vault version"));
    }
    let salt = STANDARD.decode(vault.salt_b64)?;
    let nonce = STANDARD.decode(vault.nonce_b64)?;
    let ciphertext = STANDARD.decode(vault.ciphertext_b64)?;
    if nonce.len() != 24 {
        return Err(anyhow!("invalid local vault nonce"));
    }
    let key = derive_key(password, &salt)?;
    let cipher = XChaCha20Poly1305::new(GenericArray::from_slice(&key.0));
    cipher
        .decrypt(XNonce::from_slice(&nonce), ciphertext.as_slice())
        .map_err(|_| anyhow!("local vault decryption failed"))
}

fn derive_key(password: &str, salt: &[u8]) -> Result<VaultKey> {
    let mut key = [0_u8; 32];
    Argon2::default()
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|_| anyhow!("local vault key derivation failed"))?;
    Ok(VaultKey(key))
}
