//! OS Keychain integration for secure token storage
//!
//! Wraps the OS keychain with an in-memory cache so each key is read from
//! the system at most once per app session, avoiding repeated password prompts.

use std::collections::HashMap;

use keyring::Entry;

use crate::error::AppError;

/// Service name for keyring entries
const SERVICE_NAME: &str = "opnble";

/// Trait for keychain operations
pub trait Keychain: Send + Sync {
    /// Store a secret value
    fn store(&self, key: &str, value: &str) -> Result<(), AppError>;

    /// Retrieve a secret value
    fn get(&self, key: &str) -> Result<Option<String>, AppError>;

    /// Delete a secret value
    fn delete(&self, key: &str) -> Result<(), AppError>;
}

/// Raw keyring operations without caching, extracted for testability and DRY.
trait KeyringBackend: Send + Sync {
    fn set_password(&self, key: &str, value: &str) -> Result<(), AppError>;
    fn password(&self, key: &str) -> Result<Option<String>, AppError>;
    fn delete_password(&self, key: &str) -> Result<(), AppError>;
}

/// Production backend that delegates to the OS keyring via `keyring::Entry`.
struct OsKeyringBackend;

impl KeyringBackend for OsKeyringBackend {
    fn set_password(&self, key: &str, value: &str) -> Result<(), AppError> {
        let entry = entry_for(key)?;
        entry.set_password(value).map_err(|e| AppError::AuthFailed {
            reason: format!("cannot store secret: {e}"),
        })
    }

    fn password(&self, key: &str) -> Result<Option<String>, AppError> {
        let entry = entry_for(key)?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(AppError::AuthFailed {
                reason: format!("cannot retrieve secret: {e}"),
            }),
        }
    }

    fn delete_password(&self, key: &str) -> Result<(), AppError> {
        let entry = entry_for(key)?;
        match entry.delete_password() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(AppError::AuthFailed {
                reason: format!("cannot delete secret: {e}"),
            }),
        }
    }
}

fn entry_for(key: &str) -> Result<Entry, AppError> {
    Entry::new(SERVICE_NAME, key).map_err(|e| AppError::AuthFailed {
        reason: format!("cannot create keyring entry: {e}"),
    })
}

/// OS keychain implementation with an in-memory read-through cache.
///
/// The OS keychain prompt (macOS Keychain Access, etc.) fires on every raw
/// read. Caching avoids repeated prompts within a single app session.
#[expect(
    clippy::disallowed_types,
    reason = "std::sync::Mutex is correct: all Keychain methods are sync, lock is never held across await"
)]
pub struct OsKeychain {
    backend: Box<dyn KeyringBackend>,
    cache: std::sync::Mutex<HashMap<String, Option<String>>>,
}

#[expect(
    clippy::disallowed_types,
    reason = "std::sync::Mutex is correct: all Keychain methods are sync, lock is never held across await"
)]
impl OsKeychain {
    pub fn new() -> Self {
        Self {
            backend: Box::new(OsKeyringBackend),
            cache: std::sync::Mutex::new(HashMap::new()),
        }
    }

    #[cfg(test)]
    fn with_backend(backend: Box<dyn KeyringBackend>) -> Self {
        Self {
            backend,
            cache: std::sync::Mutex::new(HashMap::new()),
        }
    }
}

impl Default for OsKeychain {
    fn default() -> Self {
        Self::new()
    }
}

impl Keychain for OsKeychain {
    fn store(&self, key: &str, value: &str) -> Result<(), AppError> {
        self.backend.set_password(key, value)?;

        self.cache
            .lock()
            .expect("keychain cache poisoned")
            .insert(key.to_string(), Some(value.to_string()));

        Ok(())
    }

    fn get(&self, key: &str) -> Result<Option<String>, AppError> {
        // Lock held across backend read intentionally: prevents concurrent
        // callers from each triggering an OS keychain password prompt.
        let mut cache = self.cache.lock().expect("keychain cache poisoned");
        if let Some(cached) = cache.get(key) {
            return Ok(cached.clone());
        }

        let result = self.backend.password(key)?;
        cache.insert(key.to_string(), result.clone());
        Ok(result)
    }

    fn delete(&self, key: &str) -> Result<(), AppError> {
        self.backend.delete_password(key)?;

        self.cache
            .lock()
            .expect("keychain cache poisoned")
            .insert(key.to_string(), None);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keychain_trait_is_object_safe() {
        fn _accepts_dyn(_: &dyn Keychain) {}
    }

    // -- Fake backend for testing cache + error paths --

    #[expect(
        clippy::disallowed_types,
        reason = "test-only fake uses std::sync::Mutex for simplicity"
    )]
    struct FakeBackend {
        data: std::sync::Mutex<HashMap<String, String>>,
        fail_on_get: bool,
        fail_on_store: bool,
        fail_on_delete: bool,
    }

    #[expect(
        clippy::disallowed_types,
        reason = "test-only fake uses std::sync::Mutex for simplicity"
    )]
    impl FakeBackend {
        fn new() -> Self {
            Self {
                data: std::sync::Mutex::new(HashMap::new()),
                fail_on_get: false,
                fail_on_store: false,
                fail_on_delete: false,
            }
        }

        fn failing_get() -> Self {
            Self {
                fail_on_get: true,
                ..Self::new()
            }
        }

        fn failing_store() -> Self {
            Self {
                fail_on_store: true,
                ..Self::new()
            }
        }

        fn failing_delete() -> Self {
            Self {
                fail_on_delete: true,
                ..Self::new()
            }
        }
    }

    impl KeyringBackend for FakeBackend {
        fn set_password(&self, key: &str, value: &str) -> Result<(), AppError> {
            if self.fail_on_store {
                return Err(AppError::AuthFailed {
                    reason: "simulated store failure".to_string(),
                });
            }
            self.data
                .lock()
                .unwrap()
                .insert(key.to_string(), value.to_string());
            Ok(())
        }

        fn password(&self, key: &str) -> Result<Option<String>, AppError> {
            if self.fail_on_get {
                return Err(AppError::AuthFailed {
                    reason: "simulated get failure".to_string(),
                });
            }
            Ok(self.data.lock().unwrap().get(key).cloned())
        }

        fn delete_password(&self, key: &str) -> Result<(), AppError> {
            if self.fail_on_delete {
                return Err(AppError::AuthFailed {
                    reason: "simulated delete failure".to_string(),
                });
            }
            self.data.lock().unwrap().remove(key);
            Ok(())
        }
    }

    // -- Happy-path tests --

    #[test]
    fn store_and_get_roundtrip() {
        let kc = OsKeychain::with_backend(Box::new(FakeBackend::new()));

        kc.store("github", "ghp_abc123").unwrap();
        assert_eq!(kc.get("github").unwrap(), Some("ghp_abc123".to_string()));
    }

    #[test]
    fn get_missing_key_returns_none() {
        let kc = OsKeychain::with_backend(Box::new(FakeBackend::new()));

        assert_eq!(kc.get("nonexistent").unwrap(), None);
    }

    #[test]
    fn delete_removes_key() {
        let kc = OsKeychain::with_backend(Box::new(FakeBackend::new()));

        kc.store("github", "token").unwrap();
        kc.delete("github").unwrap();
        assert_eq!(kc.get("github").unwrap(), None);
    }

    #[test]
    fn delete_nonexistent_key_succeeds() {
        let kc = OsKeychain::with_backend(Box::new(FakeBackend::new()));

        kc.delete("nonexistent").unwrap();
    }

    // -- Cache behavior tests --

    #[test]
    fn cached_value_is_returned_without_os_hit() {
        let kc = OsKeychain::with_backend(Box::new(FakeBackend::new()));

        kc.store("github", "cached_token").unwrap();

        let result = kc.get("github").unwrap();
        assert_eq!(result, Some("cached_token".to_string()));
    }

    #[test]
    fn cached_none_is_served_from_cache() {
        let kc = OsKeychain::with_backend(Box::new(FakeBackend::new()));

        kc.cache
            .lock()
            .unwrap()
            .insert("empty_key".to_string(), None);

        let result = kc.get("empty_key").unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn delete_caches_none_so_next_get_skips_backend() {
        let kc = OsKeychain::with_backend(Box::new(FakeBackend::new()));

        kc.store("github", "token").unwrap();
        kc.delete("github").unwrap();

        let cached = kc.cache.lock().unwrap().get("github").cloned();
        assert_eq!(cached, Some(None));
    }

    // -- Error-path tests --

    #[test]
    fn store_error_propagates() {
        let kc = OsKeychain::with_backend(Box::new(FakeBackend::failing_store()));

        let result = kc.store("github", "token");
        assert!(result.is_err());
    }

    #[test]
    fn store_error_does_not_populate_cache() {
        let kc = OsKeychain::with_backend(Box::new(FakeBackend::failing_store()));

        let _ = kc.store("github", "token");
        let cached = kc.cache.lock().unwrap().get("github").cloned();
        assert_eq!(cached, None);
    }

    #[test]
    fn get_error_propagates() {
        let kc = OsKeychain::with_backend(Box::new(FakeBackend::failing_get()));

        let result = kc.get("github");
        assert!(result.is_err());
    }

    #[test]
    fn get_error_does_not_populate_cache() {
        let kc = OsKeychain::with_backend(Box::new(FakeBackend::failing_get()));

        let _ = kc.get("github");
        let cached = kc.cache.lock().unwrap().get("github").cloned();
        assert_eq!(cached, None);
    }

    #[test]
    fn delete_error_propagates() {
        let kc = OsKeychain::with_backend(Box::new(FakeBackend::failing_delete()));

        let result = kc.delete("github");
        assert!(result.is_err());
    }

    #[test]
    fn delete_error_does_not_update_cache() {
        let kc = OsKeychain::with_backend(Box::new(FakeBackend::failing_delete()));

        kc.cache
            .lock()
            .unwrap()
            .insert("github".to_string(), Some("token".to_string()));

        let _ = kc.delete("github");

        let cached = kc.cache.lock().unwrap().get("github").cloned();
        assert_eq!(cached, Some(Some("token".to_string())));
    }
}
