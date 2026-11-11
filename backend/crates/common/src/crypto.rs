//! Secrets at rest: AES-256-GCM with the operator's `APP_ENCRYPTION_KEY`.
//! Stored form is `nonce (12 bytes) || ciphertext+tag`.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;

const NONCE_LEN: usize = 12;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CryptoError {
    #[error("APP_ENCRYPTION_KEY must be base64 of exactly 32 bytes (openssl rand -base64 32)")]
    BadKey,
    #[error("stored secret cannot be decrypted; was APP_ENCRYPTION_KEY changed?")]
    Decrypt,
    #[error("encryption failed")]
    Encrypt,
}

#[derive(Clone)]
pub struct SecretBox {
    cipher: Aes256Gcm,
}

impl SecretBox {
    pub fn from_base64(key: &str) -> Result<Self, CryptoError> {
        let bytes = STANDARD
            .decode(key.trim())
            .map_err(|_| CryptoError::BadKey)?;
        if bytes.len() != 32 {
            return Err(CryptoError::BadKey);
        }
        let cipher = Aes256Gcm::new_from_slice(&bytes).map_err(|_| CryptoError::BadKey)?;
        Ok(Self { cipher })
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<Vec<u8>, CryptoError> {
        let mut nonce = [0u8; NONCE_LEN];
        getrandom::fill(&mut nonce).map_err(|_| CryptoError::Encrypt)?;
        let ct = self
            .cipher
            .encrypt(&Nonce::from(nonce), plaintext.as_bytes())
            .map_err(|_| CryptoError::Encrypt)?;
        let mut out = nonce.to_vec();
        out.extend(ct);
        Ok(out)
    }

    pub fn decrypt(&self, stored: &[u8]) -> Result<String, CryptoError> {
        if stored.len() <= NONCE_LEN {
            return Err(CryptoError::Decrypt);
        }
        let (nonce, ct) = stored.split_at(NONCE_LEN);
        let nonce: [u8; NONCE_LEN] = nonce.try_into().map_err(|_| CryptoError::Decrypt)?;
        let pt = self
            .cipher
            .decrypt(&Nonce::from(nonce), ct)
            .map_err(|_| CryptoError::Decrypt)?;
        String::from_utf8(pt).map_err(|_| CryptoError::Decrypt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=";

    #[test]
    fn round_trips_and_uses_fresh_nonces() {
        let b = SecretBox::from_base64(KEY).unwrap();
        let a1 = b.encrypt("sk-test").unwrap();
        let a2 = b.encrypt("sk-test").unwrap();
        assert_ne!(a1, a2);
        assert_eq!(b.decrypt(&a1).unwrap(), "sk-test");
    }

    #[test]
    fn rejects_bad_keys_and_tampering() {
        assert!(SecretBox::from_base64("short").is_err());
        let b = SecretBox::from_base64(KEY).unwrap();
        let mut ct = b.encrypt("secret").unwrap();
        let last = ct.len() - 1;
        ct[last] ^= 1;
        assert_eq!(b.decrypt(&ct), Err(CryptoError::Decrypt));
    }
}
