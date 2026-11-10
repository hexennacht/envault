use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};

use keyring::Entry;

pub const SERVICE_NAME: &str = "envault";

static FALLBACK_KEYCHAIN: LazyLock<RwLock<HashMap<String, [u8; 32]>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// Store derived encryption key for a project in the OS keychain with fallback cache.
pub fn store_key(project: &str, key: &[u8; 32]) -> anyhow::Result<()> {
    // 1. Update in-memory fallback cache
    if let Ok(mut store) = FALLBACK_KEYCHAIN.write() {
        store.insert(project.to_string(), *key);
    }

    // 2. Try OS keyring
    if let Ok(entry) = Entry::new(SERVICE_NAME, project) {
        // Fallback gracefully if OS keyring fails (headless CI, no storage backend)
        let _ = entry.set_secret(key);
    }

    Ok(())
}

/// Load derived encryption key for a project from OS keychain, environment variable, or fallback cache.
pub fn load_key(project: &str) -> anyhow::Result<Option<[u8; 32]>> {
    // 1. Check in-memory fallback cache
    let in_memory = FALLBACK_KEYCHAIN
        .read()
        .ok()
        .and_then(|store| store.get(project).copied());
    if let Some(key) = in_memory {
        return Ok(Some(key));
    }

    // 2. Check environment variable fallback (e.g. ENVAULT_KEY_<PROJECT> or ENVAULT_KEY)
    let env_var_name = format!("ENVAULT_KEY_{}", sanitize_env_name(project));
    let env_key = std::env::var(&env_var_name)
        .ok()
        .or_else(|| std::env::var("ENVAULT_KEY").ok())
        .and_then(|v| parse_hex_key(&v));

    if let Some(key) = env_key {
        if let Ok(mut store) = FALLBACK_KEYCHAIN.write() {
            store.insert(project.to_string(), key);
        }
        return Ok(Some(key));
    }

    // 3. Check OS keyring
    if let Ok(entry) = Entry::new(SERVICE_NAME, project) {
        match entry.get_secret() {
            Ok(bytes) => {
                let resolved_key = if bytes.len() == 32 {
                    let mut key = [0u8; 32];
                    key.copy_from_slice(&bytes);
                    Some(key)
                } else if bytes.len() == 64 {
                    std::str::from_utf8(&bytes).ok().and_then(parse_hex_key)
                } else {
                    None
                };

                if let Some(key) = resolved_key {
                    if let Ok(mut store) = FALLBACK_KEYCHAIN.write() {
                        store.insert(project.to_string(), key);
                    }
                    return Ok(Some(key));
                }
            }
            Err(keyring::Error::NoEntry) => {
                // Not found in keyring
            }
            Err(_e) => {
                // Keyring error (unavailable, locked, headless CI) -> graceful fallback
            }
        }
    }

    Ok(None)
}

/// Delete cached key from OS keychain and fallback cache for a given project.
pub fn delete_key(project: &str) -> anyhow::Result<()> {
    if let Ok(mut store) = FALLBACK_KEYCHAIN.write() {
        store.remove(project);
    }

    if let Ok(entry) = Entry::new(SERVICE_NAME, project) {
        let _ = entry.delete_credential();
    }

    Ok(())
}

/// Clear in-memory fallback cache (useful in tests).
pub fn clear_fallback_keychain() {
    if let Ok(mut store) = FALLBACK_KEYCHAIN.write() {
        store.clear();
    }
}

fn sanitize_env_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn parse_hex_key(hex_str: &str) -> Option<[u8; 32]> {
    let s = hex_str.trim();
    if s.len() != 64 {
        return None;
    }
    let (chunks, _) = s.as_bytes().as_chunks::<2>();
    let mut key = [0u8; 32];
    for (i, chunk) in chunks.iter().enumerate() {
        let chunk_str = std::str::from_utf8(chunk).ok()?;
        key[i] = u8::from_str_radix(chunk_str, 16).ok()?;
    }
    Some(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_and_load_fallback_keychain() {
        let proj = "test-proj-fallback";
        let key = [0xabu8; 32];

        // Ensure clean state
        let _ = delete_key(proj);

        store_key(proj, &key).expect("store_key should succeed");
        let loaded = load_key(proj).expect("load_key should succeed");
        assert_eq!(loaded, Some(key));

        delete_key(proj).expect("delete_key should succeed");
        let after_delete = load_key(proj).expect("load_key should succeed");
        assert_eq!(after_delete, None);
    }

    #[test]
    fn test_env_var_fallback() {
        let proj = "test-env-proj";
        let key = [0x55u8; 32];
        let hex_str: String = key.iter().map(|b| format!("{b:02x}")).collect();

        // Clear memory
        clear_fallback_keychain();

        let env_var = "ENVAULT_KEY_TEST_ENV_PROJ";
        unsafe {
            std::env::set_var(env_var, &hex_str);
        }

        let loaded = load_key(proj).expect("load_key with env var should succeed");
        assert_eq!(loaded, Some(key));

        unsafe {
            std::env::remove_var(env_var);
        }
        clear_fallback_keychain();
    }

    #[test]
    fn test_parse_hex_key() {
        let raw = [42u8; 32];
        let hex_str: String = raw.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(parse_hex_key(&hex_str), Some(raw));

        // Invalid lengths or characters
        assert_eq!(parse_hex_key("abcd"), None);
        assert_eq!(parse_hex_key(&"z".repeat(64)), None);
    }
}
