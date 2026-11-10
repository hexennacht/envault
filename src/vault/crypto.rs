use aes_gcm::{
    aead::{AeadInPlace, KeyInit},
    Aes256Gcm, Key, Nonce, Tag,
};
use anyhow::Context;
use rand::RngCore;

pub const IV_LEN: usize = 12;
pub const TAG_LEN: usize = 16;
pub const MIN_PAYLOAD_LEN: usize = IV_LEN + TAG_LEN;

/// Encrypted payload containing IV, ciphertext, and authentication tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptedPayload {
    pub iv: [u8; 12],
    pub ciphertext: Vec<u8>,
    pub tag: [u8; 16],
}

impl EncryptedPayload {
    /// Serialize payload into binary format: IV (12 bytes) | ciphertext | tag (16 bytes).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(IV_LEN + self.ciphertext.len() + TAG_LEN);
        out.extend_from_slice(&self.iv);
        out.extend_from_slice(&self.ciphertext);
        out.extend_from_slice(&self.tag);
        out
    }

    /// Parse binary payload with format: IV (12 bytes) | ciphertext | tag (16 bytes).
    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        if bytes.len() < MIN_PAYLOAD_LEN {
            anyhow::bail!(
                "invalid payload length: expected at least {MIN_PAYLOAD_LEN} bytes (IV + tag), got {}",
                bytes.len()
            );
        }
        let mut iv = [0u8; IV_LEN];
        iv.copy_from_slice(&bytes[..IV_LEN]);

        let tag_start = bytes.len() - TAG_LEN;
        let mut tag = [0u8; TAG_LEN];
        tag.copy_from_slice(&bytes[tag_start..]);

        let ciphertext = bytes[IV_LEN..tag_start].to_vec();
        Ok(Self {
            iv,
            ciphertext,
            tag,
        })
    }
}

/// Encrypt plaintext with AES-256-GCM.
/// Returns serialized payload: IV (12 bytes) | ciphertext | tag (16 bytes).
pub fn encrypt(key: &[u8; 32], plaintext: &str) -> anyhow::Result<Vec<u8>> {
    let payload = encrypt_payload(key, plaintext)?;
    Ok(payload.to_bytes())
}

/// Encrypt plaintext with AES-256-GCM returning an `EncryptedPayload`.
pub fn encrypt_payload(key: &[u8; 32], plaintext: &str) -> anyhow::Result<EncryptedPayload> {
    let mut iv = [0u8; IV_LEN];
    rand::thread_rng().fill_bytes(&mut iv);

    let cipher_key = Key::<Aes256Gcm>::from_slice(key);
    let cipher = Aes256Gcm::new(cipher_key);
    let nonce = Nonce::from_slice(&iv);
    let mut buffer = plaintext.as_bytes().to_vec();

    let tag = cipher
        .encrypt_in_place_detached(nonce, b"", &mut buffer)
        .map_err(|e| anyhow::anyhow!("encryption failed: {e}"))?;

    let mut tag_bytes = [0u8; TAG_LEN];
    tag_bytes.copy_from_slice(tag.as_slice());

    Ok(EncryptedPayload {
        iv,
        ciphertext: buffer,
        tag: tag_bytes,
    })
}

/// Decrypt binary payload formatted as IV (12 bytes) | ciphertext | tag (16 bytes).
pub fn decrypt(key: &[u8; 32], payload: &[u8]) -> anyhow::Result<String> {
    let parsed = EncryptedPayload::from_bytes(payload)
        .context("failed to parse encrypted payload")?;
    decrypt_payload(key, &parsed)
}

/// Decrypt an `EncryptedPayload` into plaintext string.
pub fn decrypt_payload(key: &[u8; 32], payload: &EncryptedPayload) -> anyhow::Result<String> {
    let cipher_key = Key::<Aes256Gcm>::from_slice(key);
    let cipher = Aes256Gcm::new(cipher_key);
    let nonce = Nonce::from_slice(&payload.iv);
    let tag = Tag::from_slice(&payload.tag);
    let mut buffer = payload.ciphertext.clone();

    cipher
        .decrypt_in_place_detached(nonce, b"", &mut buffer, tag)
        .map_err(|_| anyhow::anyhow!("decryption failed: authentication tag mismatch or corrupted ciphertext"))?;

    let plaintext = String::from_utf8(buffer)
        .context("decrypted payload is not valid UTF-8")?;

    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = [42u8; 32];
        let plaintexts = [
            "super_secret_value_123",
            "",
            "special chars: !@#$%^&*()_+-=[]{}|;':\",./<>?`~",
            "unicode: 🦀🔒 UTF-8 密钥 비밀번호",
            &"A".repeat(10000), // large secret payload
        ];

        for &plain in &plaintexts {
            let encrypted = encrypt(&key, plain).expect("encryption should succeed");
            assert!(
                encrypted.len() >= MIN_PAYLOAD_LEN,
                "encrypted length must be at least 28 bytes"
            );

            let decrypted = decrypt(&key, &encrypted).expect("decryption should succeed");
            assert_eq!(decrypted, plain);
        }
    }

    #[test]
    fn test_corrupted_ciphertext_detection() {
        let key = [7u8; 32];
        let plaintext = "production_database_password";
        let mut encrypted = encrypt(&key, plaintext).expect("encryption should succeed");

        // Tamper with a byte inside the ciphertext region (between IV and tag)
        let ciphertext_idx = IV_LEN + 2;
        assert!(ciphertext_idx < encrypted.len() - TAG_LEN);
        encrypted[ciphertext_idx] ^= 0x55;

        let result = decrypt(&key, &encrypted);
        assert!(result.is_err(), "decryption of tampered ciphertext must fail");
    }

    #[test]
    fn test_corrupted_tag_detection() {
        let key = [8u8; 32];
        let plaintext = "secret_api_token";
        let mut encrypted = encrypt(&key, plaintext).expect("encryption should succeed");

        // Tamper with the authentication tag at the end
        let tag_idx = encrypted.len() - 1;
        encrypted[tag_idx] ^= 0x01;

        let result = decrypt(&key, &encrypted);
        assert!(result.is_err(), "decryption with tampered tag must fail");
    }

    #[test]
    fn test_corrupted_iv_detection() {
        let key = [9u8; 32];
        let plaintext = "secret_iv_test";
        let mut encrypted = encrypt(&key, plaintext).expect("encryption should succeed");

        // Tamper with the IV
        encrypted[0] ^= 0x01;

        let result = decrypt(&key, &encrypted);
        assert!(result.is_err(), "decryption with tampered IV must fail");
    }

    #[test]
    fn test_wrong_key_fails() {
        let key_a = [1u8; 32];
        let key_b = [2u8; 32];
        let plaintext = "secret_for_key_a";

        let encrypted = encrypt(&key_a, plaintext).expect("encryption should succeed");
        let result = decrypt(&key_b, &encrypted);
        assert!(result.is_err(), "decryption with wrong key must fail");
    }

    #[test]
    fn test_truncated_payload_fails() {
        let key = [10u8; 32];
        let short_payload = [0u8; 20]; // Less than MIN_PAYLOAD_LEN (28)

        let result = decrypt(&key, &short_payload);
        assert!(result.is_err(), "decryption of truncated payload must fail");
    }

    #[test]
    fn test_encrypted_payload_serialization() {
        let payload = EncryptedPayload {
            iv: [1u8; 12],
            ciphertext: vec![2, 3, 4, 5],
            tag: [6u8; 16],
        };

        let bytes = payload.to_bytes();
        assert_eq!(bytes.len(), 12 + 4 + 16);

        let parsed = EncryptedPayload::from_bytes(&bytes).expect("parsing should succeed");
        assert_eq!(parsed, payload);
    }
}
