//! Vault for encrypted secrets management

use aes_gcm::{Aes256Gcm, KeyInit, aead::{Aead, generic_array::GenericArray}};
use aes_gcm::aead::generic_array::typenum;
use argon2::Argon2;
use argon2::password_hash::SaltString;
use rand::rngs::OsRng;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use crate::error::{Error, Result};

/// Vault header marker
const VAULT_HEADER: &str = "$RUSTIBLE_VAULT;1.0;AES256";

/// Vault for encrypting/decrypting secrets
pub struct Vault {
    password: String,
}

impl Vault {
    /// Create a new vault with password
    pub fn new(password: impl Into<String>) -> Self {
        Self { password: password.into() }
    }

    /// Encrypt content
    pub fn encrypt(&self, content: &str) -> Result<String> {
        let salt = SaltString::generate(&mut OsRng);
        let key = self.derive_key(&salt)?;

        let cipher = Aes256Gcm::new(&key);
        let nonce_bytes: [u8; 12] = rand::random();
        let nonce = GenericArray::from_slice(&nonce_bytes);

        let ciphertext = cipher.encrypt(nonce, content.as_bytes())
            .map_err(|e| Error::Vault(format!("Encryption failed: {}", e)))?;

        let mut encrypted = Vec::new();
        encrypted.extend_from_slice(salt.as_str().as_bytes());
        encrypted.push(b'\n');
        encrypted.extend_from_slice(&nonce_bytes);
        encrypted.extend_from_slice(&ciphertext);

        Ok(format!("{}\n{}", VAULT_HEADER, BASE64.encode(&encrypted)))
    }

    /// Decrypt content
    pub fn decrypt(&self, content: &str) -> Result<String> {
        let lines: Vec<&str> = content.lines().collect();
        if lines.is_empty() || !lines[0].starts_with("$RUSTIBLE_VAULT") {
            return Err(Error::Vault("Invalid vault format".into()));
        }

        let encrypted = BASE64.decode(lines[1..].join(""))
            .map_err(|e| Error::Vault(format!("Base64 decode failed: {}", e)))?;

        // Parse salt, nonce, and ciphertext
        let salt_end = encrypted.iter().position(|&b| b == b'\n')
            .ok_or_else(|| Error::Vault("Invalid vault format".into()))?;
        let salt_str = std::str::from_utf8(&encrypted[..salt_end])
            .map_err(|_| Error::Vault("Invalid salt".into()))?;
        let salt = SaltString::from_b64(salt_str)
            .map_err(|_| Error::Vault("Invalid salt".into()))?;

        let nonce_start = salt_end + 1;
        let nonce = GenericArray::from_slice(&encrypted[nonce_start..nonce_start + 12]);
        let ciphertext = &encrypted[nonce_start + 12..];

        let key = self.derive_key(&salt)?;
        let cipher = Aes256Gcm::new(&key);

        let plaintext = cipher.decrypt(nonce, ciphertext)
            .map_err(|_| Error::Vault("Decryption failed - wrong password?".into()))?;

        String::from_utf8(plaintext)
            .map_err(|_| Error::Vault("Invalid UTF-8 in decrypted content".into()))
    }

    /// Check if content is vault encrypted
    pub fn is_encrypted(content: &str) -> bool {
        content.starts_with("$RUSTIBLE_VAULT")
    }

    fn derive_key(&self, salt: &SaltString) -> Result<GenericArray<u8, typenum::U32>> {
        let argon2 = Argon2::default();
        let mut key = [0u8; 32];
        argon2.hash_password_into(self.password.as_bytes(), salt.as_str().as_bytes(), &mut key)
            .map_err(|e| Error::Vault(format!("Key derivation failed: {}", e)))?;
        Ok(GenericArray::clone_from_slice(&key))
    }
}
