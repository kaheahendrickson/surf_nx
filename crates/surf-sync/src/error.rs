//! Error types for surf-sync.

use solana_pubkey::Pubkey;
use surf_store::StoreError;
use thiserror::Error;

/// Errors that can occur during synchronization.
#[derive(Debug, Error)]
pub enum SyncError {
    /// RPC provider connection issue.
    #[error("Provider error: {0}")]
    Provider(String),

    /// Account on-chain data is missing or malformed.
    #[error("Invalid account data for {0}")]
    InvalidAccountData(Pubkey),

    /// Checkpoint data is corrupted and cannot be deserialized.
    #[error("Checkpoint corrupted")]
    CheckpointCorrupted,

    /// Transaction instruction could not be parsed.
    #[error("Invalid instruction data")]
    InvalidInstruction,

    /// Request timed out.
    #[error("Request timeout")]
    Timeout,

    /// RPC rate limit hit.
    #[error("Rate limited")]
    RateLimited,

    /// Store operation failed.
    #[error(transparent)]
    Store(#[from] StoreError),

    /// Client error from surf-client.
    #[error("Client error: {0}")]
    Client(String),

    /// Configuration is invalid.
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Sync was stopped.
    #[error("Sync stopped")]
    Stopped,

    /// Event streaming infrastructure failed.
    #[error("Event stream error: {0}")]
    EventStream(String),

    /// Event payload was invalid.
    #[error("Invalid event payload: {0}")]
    InvalidEvent(String),
}

impl From<surf_client::Error> for SyncError {
    fn from(err: surf_client::Error) -> Self {
        match err {
            surf_client::Error::AccountNotFound(pubkey) => {
                SyncError::Provider(format!("Account not found: {}", pubkey))
            }
            surf_client::Error::InvalidAccountData => {
                SyncError::Client("Invalid account data".to_string())
            }
            surf_client::Error::Backend(msg) => SyncError::Provider(msg),
            other => SyncError::Client(other.to_string()),
        }
    }
}
