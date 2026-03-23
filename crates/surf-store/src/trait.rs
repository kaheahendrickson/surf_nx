//! KeyValueStore trait definition.

use crate::error::StoreError;
use std::future::Future;

/// An async key-value store with column family namespacing.
///
/// This trait provides a unified interface for key-value storage across
/// different platforms (native filesystem, browser OPFS, in-memory).
///
/// # Column Families
///
/// Keys are organized into column families for namespacing. Use the predefined
/// constants: [`NAMES`], [`CHECKPOINTS`], [`BALANCES`], [`PROPOSALS`], [`METADATA`].
///
/// # Thread Safety
///
/// Native implementations are `Send + Sync` and can be safely shared across threads.
/// Browser/WASM implementations are single-threaded and omit those bounds.
///
/// # Example
///
/// ```rust,no_run
/// use surf_store::{KeyValueStore, MemoryStore, NAMES};
///
/// # #[tokio::main]
/// # async fn main() {
/// let store = MemoryStore::new();
/// store.set(NAMES, b"key", b"value").await.unwrap();
/// let value = store.get(NAMES, b"key").await.unwrap();
/// # }
/// ```
///
/// [`NAMES`]: crate::NAMES
/// [`CHECKPOINTS`]: crate::CHECKPOINTS
/// [`BALANCES`]: crate::BALANCES
/// [`PROPOSALS`]: crate::PROPOSALS
/// [`METADATA`]: crate::METADATA
#[cfg(not(target_arch = "wasm32"))]
pub trait KeyValueStore: Send + Sync {
    /// Retrieves a value by key from the specified column family.
    ///
    /// Returns `Ok(None)` if the key does not exist.
    ///
    /// # Arguments
    ///
    /// * `cf` - Column family name (must be one of the predefined constants)
    /// * `key` - The key to look up
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::InvalidColumnFamily`] if the column family is not recognized.
    /// Returns [`StoreError::Closed`] if the store has been closed.
    fn get(
        &self,
        cf: &str,
        key: &[u8],
    ) -> impl Future<Output = Result<Option<Vec<u8>>, StoreError>> + Send;

    /// Sets a value for the given key in the specified column family.
    ///
    /// # Arguments
    ///
    /// * `cf` - Column family name
    /// * `key` - The key to set
    /// * `value` - The value to store
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::InvalidColumnFamily`] if the column family is not recognized.
    /// Returns [`StoreError::Closed`] if the store has been closed.
    fn set(
        &self,
        cf: &str,
        key: &[u8],
        value: &[u8],
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// Deletes a key from the specified column family.
    ///
    /// Returns `Ok(())` even if the key did not exist.
    ///
    /// # Arguments
    ///
    /// * `cf` - Column family name
    /// * `key` - The key to delete
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::InvalidColumnFamily`] if the column family is not recognized.
    /// Returns [`StoreError::Closed`] if the store has been closed.
    fn delete(&self, cf: &str, key: &[u8]) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// Lists all keys in the specified column family.
    ///
    /// # Arguments
    ///
    /// * `cf` - Column family name
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::InvalidColumnFamily`] if the column family is not recognized.
    /// Returns [`StoreError::Closed`] if the store has been closed.
    fn list_keys(&self, cf: &str) -> impl Future<Output = Result<Vec<Vec<u8>>, StoreError>> + Send;

    /// Checks if a key exists in the specified column family.
    ///
    /// # Arguments
    ///
    /// * `cf` - Column family name
    /// * `key` - The key to check
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::InvalidColumnFamily`] if the column family is not recognized.
    /// Returns [`StoreError::Closed`] if the store has been closed.
    fn exists(
        &self,
        cf: &str,
        key: &[u8],
    ) -> impl Future<Output = Result<bool, StoreError>> + Send {
        async move { self.get(cf, key).await.map(|v| v.is_some()) }
    }

    /// Flushes any pending writes to persistent storage.
    ///
    /// For in-memory stores, this is a no-op.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Closed`] if the store has been closed.
    fn flush(&self) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// Closes the store, releasing any resources.
    ///
    /// After calling this method, all subsequent operations will return [`StoreError::Closed`].
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Closed`] if the store is already closed.
    fn close(&self) -> impl Future<Output = Result<(), StoreError>> + Send;
}

#[cfg(target_arch = "wasm32")]
pub trait KeyValueStore {
    fn get(
        &self,
        cf: &str,
        key: &[u8],
    ) -> impl Future<Output = Result<Option<Vec<u8>>, StoreError>>;

    fn set(
        &self,
        cf: &str,
        key: &[u8],
        value: &[u8],
    ) -> impl Future<Output = Result<(), StoreError>>;

    fn delete(&self, cf: &str, key: &[u8]) -> impl Future<Output = Result<(), StoreError>>;

    fn list_keys(&self, cf: &str) -> impl Future<Output = Result<Vec<Vec<u8>>, StoreError>>;

    fn exists(&self, cf: &str, key: &[u8]) -> impl Future<Output = Result<bool, StoreError>> {
        async move { self.get(cf, key).await.map(|v| v.is_some()) }
    }

    fn flush(&self) -> impl Future<Output = Result<(), StoreError>>;

    fn close(&self) -> impl Future<Output = Result<(), StoreError>>;
}
