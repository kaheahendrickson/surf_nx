//! Error types for surf-store.

use thiserror::Error;

/// Errors that can occur when working with a key-value store.
#[derive(Debug, Error)]
pub enum StoreError {
    /// An I/O error occurred (filesystem or OPFS operation failed).
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// The provided key is invalid (e.g., contains invalid hex characters).
    #[error("Invalid key: {0}")]
    InvalidKey(String),

    /// The specified column family is not one of the predefined column families.
    #[error("Invalid column family: {0}")]
    InvalidColumnFamily(String),

    /// The specified column family does not exist in the store.
    #[error("Column family not found: {0}")]
    ColumnFamilyNotFound(String),

    /// The store has not been initialized.
    #[error("Store not initialized")]
    NotInitialized,

    /// The store has been closed and cannot accept further operations.
    #[error("Store already closed")]
    Closed,

    /// A serialization or deserialization error occurred.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// An OPFS-specific error occurred (browser only).
    #[cfg(target_arch = "wasm32")]
    #[error("OPFS error: {0}")]
    Opfs(String),

    /// A JavaScript interop error occurred (browser only).
    #[cfg(target_arch = "wasm32")]
    #[error("JavaScript error: {0}")]
    Js(String),
}

impl StoreError {
    /// Creates a new OPFS error with the given message (browser only).
    #[cfg(target_arch = "wasm32")]
    pub fn opfs(msg: impl Into<String>) -> Self {
        StoreError::Opfs(msg.into())
    }
}
