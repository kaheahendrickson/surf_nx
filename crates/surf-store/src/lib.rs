//! # surf-store
//!
//! An isomorphic key-value store that works across native (filesystem) and
//! browser (OPFS) environments.
//!
//! ## Overview
//!
//! This crate provides a [`KeyValueStore`] trait with multiple backend implementations:
//!
//! - [`MemoryStore`] - In-memory storage for testing (available on all platforms)
//! - [`NativeStore`] - Filesystem-based storage for native platforms
//! - [`OpfsStore`] - OPFS-based storage for browser/web worker environments
//!
//! ## Column Families
//!
//! Data is organized into predefined column families for namespacing:
//!
//! - [`NAMES`] - Name records
//! - [`CHECKPOINTS`] - Sync checkpoints
//! - [`BALANCES`] - Token balances
//! - [`LAMPORTS`] - Native SOL balances
//! - [`TRANSACTIONS`] - Curated activity records
//! - [`FOLLOWS`] - Active follow relationships
//! - [`PROPOSALS`] - Governance proposals
//! - [`METADATA`] - Configuration values
//!
//! ## Example
//!
//! ```rust,ignore
//! use surf_store::{KeyValueStore, MemoryStore, NAMES};
//!
//! #[tokio::main]
//! async fn main() {
//!     let store = MemoryStore::new();
//!     
//!     // Store a value
//!     store.set(NAMES, b"alice", b"owner_pubkey").await.unwrap();
//!     
//!     // Retrieve a value
//!     let value = store.get(NAMES, b"alice").await.unwrap();
//!     assert_eq!(value, Some(b"owner_pubkey".to_vec()));
//!     
//!     // List all keys in a column family
//!     let keys = store.list_keys(NAMES).await.unwrap();
//!     assert!(keys.contains(&b"alice".to_vec()));
//! }
//! ```

pub mod column_families;
pub mod error;
pub mod memory;
#[cfg(target_arch = "wasm32")]
#[cfg(test)]
mod wasm_tests;
pub mod r#trait;

#[cfg(not(target_arch = "wasm32"))]
pub mod native;

#[cfg(target_arch = "wasm32")]
pub mod opfs;

pub use column_families::{
    is_valid_column_family, ALL_COLUMN_FAMILIES, BALANCES, CHECKPOINTS, FOLLOWS, LAMPORTS,
    METADATA, NAMES, PROPOSALS, TRANSACTIONS,
};
pub use error::StoreError;
pub use memory::MemoryStore;
pub use r#trait::KeyValueStore;

#[cfg(not(target_arch = "wasm32"))]
pub use native::NativeStore;

#[cfg(target_arch = "wasm32")]
pub use opfs::OpfsStore;
