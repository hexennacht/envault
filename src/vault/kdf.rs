use argon2::{Algorithm, Argon2, Params, Version};
use rand::RngCore;

pub const ARGON2_M_COST: u32 = 65536; // 64 MiB
pub const ARGON2_T_COST: u32 = 3;     // 3 iterations
pub const ARGON2_P_COST: u32 = 4;     // 4 lanes parallelism
pub const KEY_LEN: usize = 32;

/// Derive a 32-byte encryption key from a password and salt using Argon2id.
pub fn derive_key(password: &str, salt: &[u8]) -> anyhow::Result<[u8; 32]> {
    let params = Params::new(ARGON2_M_COST, ARGON2_T_COST, ARGON2_P_COST, Some(KEY_LEN))
        .map_err(|e| anyhow::anyhow!("invalid argon2 params: {e}"))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key = [0u8; 32];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| anyhow::anyhow!("argon2 key derivation failed: {e}"))?;

    Ok(key)
}

/// Generate a cryptographically secure 32-byte salt using rand::RngCore.
pub fn generate_salt() -> [u8; 32] {
    let mut salt = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut salt);
    salt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_key_deterministic() {
        let password = "master_password_test_123!";
        let salt = [42u8; 32];

        let key1 = derive_key(password, &salt).expect("first key derivation should succeed");
        let key2 = derive_key(password, &salt).expect("second key derivation should succeed");

        assert_eq!(key1, key2, "same password and salt must produce identical key");
        assert_ne!(key1, [0u8; 32], "derived key must not be all zeroes");
    }

    #[test]
    fn test_derive_key_diff_passwords() {
        let salt = [11u8; 32];
        let key1 = derive_key("password_alpha", &salt).expect("derivation should succeed");
        let key2 = derive_key("password_beta", &salt).expect("derivation should succeed");

        assert_ne!(
            key1, key2,
            "different passwords must produce different derived keys"
        );
    }

    #[test]
    fn test_derive_key_diff_salts() {
        let password = "identical_password";
        let salt1 = [1u8; 32];
        let salt2 = [2u8; 32];

        let key1 = derive_key(password, &salt1).expect("derivation should succeed");
        let key2 = derive_key(password, &salt2).expect("derivation should succeed");

        assert_ne!(
            key1, key2,
            "different salts must produce different derived keys"
        );
    }

    #[test]
    fn test_generate_salt_randomness() {
        let salt1 = generate_salt();
        let salt2 = generate_salt();

        assert_eq!(salt1.len(), 32);
        assert_eq!(salt2.len(), 32);
        assert_ne!(
            salt1, salt2,
            "subsequent salt generations must produce unique values"
        );
    }
}
