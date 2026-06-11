//! Token storage abstraction
//!
//! Provides a trait for secure token storage and an implementation
//! that delegates to the infrastructure keychain.

use std::sync::Arc;

use crate::error::AppError;
use crate::infrastructure::Keychain;

/// Trait for storing and retrieving authentication tokens by provider
pub trait TokenStore: Send + Sync {
    /// Store a token for a provider
    fn store(&self, provider: &str, token: &str) -> Result<(), AppError>;

    /// Get a token for a provider
    fn get(&self, provider: &str) -> Result<Option<String>, AppError>;

    /// Delete a token for a provider
    fn delete(&self, provider: &str) -> Result<(), AppError>;
}

/// Token store implementation backed by the OS keychain
pub struct KeychainTokenStore {
    keychain: Arc<dyn Keychain>,
}

impl KeychainTokenStore {
    /// Create a new `KeychainTokenStore` wrapping the given keychain
    pub fn new(keychain: Arc<dyn Keychain>) -> Self {
        Self { keychain }
    }
}

impl TokenStore for KeychainTokenStore {
    fn store(&self, provider: &str, token: &str) -> Result<(), AppError> {
        self.keychain.store(provider, token)
    }

    fn get(&self, provider: &str) -> Result<Option<String>, AppError> {
        self.keychain.get(provider)
    }

    fn delete(&self, provider: &str) -> Result<(), AppError> {
        self.keychain.delete(provider)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_store_trait_is_object_safe() {
        // Verify trait can be used as dyn
        fn _accepts_dyn(_: &dyn TokenStore) {}
    }

    /// Mock keychain for testing
    #[expect(
        clippy::disallowed_types,
        reason = "test mock uses sync Mutex for simplicity"
    )]
    struct MockKeychain {
        tokens: std::sync::Mutex<std::collections::HashMap<String, String>>,
    }

    #[expect(
        clippy::disallowed_types,
        reason = "test mock uses sync Mutex for simplicity"
    )]
    impl MockKeychain {
        fn new() -> Self {
            Self {
                tokens: std::sync::Mutex::new(std::collections::HashMap::new()),
            }
        }
    }

    impl Keychain for MockKeychain {
        fn store(&self, key: &str, value: &str) -> Result<(), AppError> {
            self.tokens
                .lock()
                .unwrap()
                .insert(key.to_string(), value.to_string());
            Ok(())
        }

        fn get(&self, key: &str) -> Result<Option<String>, AppError> {
            Ok(self.tokens.lock().unwrap().get(key).cloned())
        }

        fn delete(&self, key: &str) -> Result<(), AppError> {
            self.tokens.lock().unwrap().remove(key);
            Ok(())
        }
    }

    #[test]
    fn test_keychain_token_store_store_and_get() {
        let keychain = Arc::new(MockKeychain::new());
        let store = KeychainTokenStore::new(keychain);

        store.store("github", "test_token").unwrap();
        let token = store.get("github").unwrap();
        assert_eq!(token, Some("test_token".to_string()));
    }

    #[test]
    fn test_keychain_token_store_get_missing() {
        let keychain = Arc::new(MockKeychain::new());
        let store = KeychainTokenStore::new(keychain);

        let token = store.get("nonexistent").unwrap();
        assert_eq!(token, None);
    }

    #[test]
    fn test_keychain_token_store_delete() {
        let keychain = Arc::new(MockKeychain::new());
        let store = KeychainTokenStore::new(keychain);

        store.store("github", "test_token").unwrap();
        store.delete("github").unwrap();
        let token = store.get("github").unwrap();
        assert_eq!(token, None);
    }

    #[test]
    fn test_keychain_token_store_delete_nonexistent() {
        let keychain = Arc::new(MockKeychain::new());
        let store = KeychainTokenStore::new(keychain);

        // Deleting non-existent should succeed
        store.delete("nonexistent").unwrap();
    }
}
