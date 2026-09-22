use aes_gcm::aead::{Aead, Generate, Key, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{Context, Result, anyhow};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;

const NONCE_LEN: usize = 12;

/// Encrypts secrets at rest — today, OAuth refresh tokens.
#[derive(Clone)]
pub struct Cipher {
    key: Aes256Gcm,
}

impl Cipher {
    pub fn from_base64_key(key: &str) -> Result<Self> {
        let bytes = BASE64
            .decode(key.trim())
            .context("ENCRYPTION_KEY must be valid base64")?;
        if bytes.len() != 32 {
            return Err(anyhow!(
                "ENCRYPTION_KEY must decode to 32 bytes, got {}",
                bytes.len()
            ));
        }
        let key = Key::<Aes256Gcm>::try_from(bytes.as_slice())
            .map_err(|_| anyhow!("ENCRYPTION_KEY is not a valid 32-byte key"))?;
        Ok(Self {
            key: Aes256Gcm::new(&key),
        })
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<String> {
        // Nonce must be unique per message; AES-GCM fails catastrophically on reuse.
        let nonce = Nonce::generate();

        let ciphertext = self
            .key
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|_| anyhow!("encryption failed"))?;

        let mut sealed = nonce.to_vec();
        sealed.extend_from_slice(&ciphertext);
        Ok(BASE64.encode(sealed))
    }

    pub fn decrypt(&self, sealed: &str) -> Result<String> {
        let bytes = BASE64.decode(sealed).context("ciphertext is not base64")?;
        if bytes.len() <= NONCE_LEN {
            return Err(anyhow!("ciphertext is too short"));
        }
        let (nonce, ciphertext) = bytes.split_at(NONCE_LEN);
        let nonce = Nonce::try_from(nonce).map_err(|_| anyhow!("ciphertext has a bad nonce"))?;

        let plaintext = self
            .key
            .decrypt(&nonce, ciphertext)
            .map_err(|_| anyhow!("decryption failed"))?;

        String::from_utf8(plaintext).context("decrypted value is not valid UTF-8")
    }
}
