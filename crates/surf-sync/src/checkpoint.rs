//! Checkpoint management for sync state persistence.

use solana_pubkey::Pubkey;
use surf_store::{KeyValueStore, CHECKPOINTS};

use crate::error::SyncError;

/// Key for name sync checkpoint in CHECKPOINTS column.
pub const NAME_SYNC_KEY: &[u8] = b"name_sync";

/// Key for balance sync checkpoint in CHECKPOINTS column.
pub const BALANCE_SYNC_KEY: &[u8] = b"balance_sync";

/// Key for curated activity sync checkpoint in CHECKPOINTS column.
pub const ACTIVITY_SYNC_KEY: &[u8] = b"activity_sync";

/// Key for follow sync checkpoint in CHECKPOINTS column.
pub const FOLLOW_SYNC_KEY: &[u8] = b"follow_sync";

/// Key for follow JetStream event checkpoint in CHECKPOINTS column.
pub const FOLLOW_EVENT_SYNC_KEY: &[u8] = b"follow_event_sync";
pub const NAME_EVENT_SYNC_KEY: &[u8] = b"name_event_sync";
pub const BALANCE_EVENT_SYNC_KEY: &[u8] = b"balance_event_sync";
pub const ACTIVITY_EVENT_SYNC_KEY: &[u8] = b"activity_event_sync";

/// Sync service state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SyncServiceState {
    Idle,
    Bootstrapping,
    Streaming,
    Paused,
    Stopped,
}

/// Checkpoint tracking sync progress for a program.
#[derive(Debug, Clone)]
pub struct SyncCheckpoint {
    /// Program ID being synced.
    pub program_id: Pubkey,
    /// Last processed slot.
    pub last_slot: u64,
    /// Last processed transaction signature (optional).
    pub last_signature: Option<[u8; 64]>,
    /// Number of accounts synced.
    pub account_count: u64,
    /// Unix timestamp in milliseconds when last synced.
    pub synced_at: i64,
}

impl SyncCheckpoint {
    /// Creates a new checkpoint for a program.
    pub fn new(program_id: Pubkey, last_slot: u64) -> Self {
        Self {
            program_id,
            last_slot,
            last_signature: None,
            account_count: 0,
            synced_at: current_timestamp_millis(),
        }
    }

    /// Updates the checkpoint with new slot and signature.
    pub fn update(&mut self, slot: u64, signature: Option<[u8; 64]>) {
        self.last_slot = slot;
        self.last_signature = signature;
        self.synced_at = current_timestamp_millis();
    }

    /// Increments the account count.
    pub fn increment_accounts(&mut self, count: u64) {
        self.account_count += count;
    }

    /// Serializes the checkpoint to bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, SyncError> {
        let mut bytes = Vec::with_capacity(128);
        bytes.extend_from_slice(self.program_id.as_ref());
        bytes.extend_from_slice(&self.last_slot.to_le_bytes());

        match &self.last_signature {
            Some(sig) => {
                bytes.push(1);
                bytes.extend_from_slice(sig);
            }
            None => {
                bytes.push(0);
            }
        }
        bytes.extend_from_slice(&self.account_count.to_le_bytes());
        bytes.extend_from_slice(&self.synced_at.to_le_bytes());

        Ok(bytes)
    }

    /// Deserializes the checkpoint from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, SyncError> {
        if data.len() < 32 + 8 + 1 + 8 + 8 {
            return Err(SyncError::CheckpointCorrupted);
        }
        let program_id =
            Pubkey::try_from(&data[0..32]).map_err(|_| SyncError::CheckpointCorrupted)?;

        let last_slot = u64::from_le_bytes(data[32..40].try_into().unwrap());

        let has_signature = data[40] != 0;
        let last_signature = if has_signature {
            if data.len() < 32 + 8 + 1 + 64 + 8 + 8 {
                return Err(SyncError::CheckpointCorrupted);
            }
            let mut sig = [0u8; 64];
            sig.copy_from_slice(&data[41..105]);
            Some(sig)
        } else {
            None
        };
        let offset = if has_signature { 105 } else { 41 };
        if data.len() < offset + 8 + 8 {
            return Err(SyncError::CheckpointCorrupted);
        }

        let account_count = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
        let synced_at = i64::from_le_bytes(data[offset + 8..offset + 16].try_into().unwrap());

        Ok(Self {
            program_id,
            last_slot,
            last_signature,
            account_count,
            synced_at,
        })
    }
}

/// Combined sync state for both name and balance syncers.
#[derive(Debug, Clone, Default)]
pub struct SyncState {
    /// Checkpoint for name registry sync.
    pub name_checkpoint: Option<SyncCheckpoint>,
    /// Checkpoint for balance sync.
    pub balance_checkpoint: Option<SyncCheckpoint>,
    /// Checkpoint for curated activity sync.
    pub activity_checkpoint: Option<SyncCheckpoint>,
    /// Checkpoint for follow sync.
    pub follow_checkpoint: Option<SyncCheckpoint>,
    /// Checkpoint for follow event stream sync.
    pub follow_event_checkpoint: Option<EventStreamCheckpoint>,
    pub name_event_checkpoint: Option<EventStreamCheckpoint>,
    pub balance_event_checkpoint: Option<EventStreamCheckpoint>,
    pub activity_event_checkpoint: Option<EventStreamCheckpoint>,
}

/// Checkpoint for JetStream-delivered event streams.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventStreamCheckpoint {
    pub last_stream_sequence: u64,
    pub last_slot: u64,
    pub last_event_id: Option<String>,
    pub synced_at: i64,
}

impl Default for EventStreamCheckpoint {
    fn default() -> Self {
        Self::new()
    }
}

impl EventStreamCheckpoint {
    pub fn new() -> Self {
        Self {
            last_stream_sequence: 0,
            last_slot: 0,
            last_event_id: None,
            synced_at: current_timestamp_millis(),
        }
    }

    pub fn update(&mut self, stream_sequence: u64, slot: u64, event_id: Option<String>) {
        self.last_stream_sequence = stream_sequence;
        self.last_slot = slot;
        self.last_event_id = event_id;
        self.synced_at = current_timestamp_millis();
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(8 + 8 + 1 + 256 + 8);
        bytes.extend_from_slice(&self.last_stream_sequence.to_le_bytes());
        bytes.extend_from_slice(&self.last_slot.to_le_bytes());
        match &self.last_event_id {
            Some(event_id) => {
                bytes.push(1);
                let event_bytes = event_id.as_bytes();
                bytes.extend_from_slice(&(event_bytes.len() as u32).to_le_bytes());
                bytes.extend_from_slice(event_bytes);
            }
            None => bytes.push(0),
        }
        bytes.extend_from_slice(&self.synced_at.to_le_bytes());
        bytes
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self, SyncError> {
        if data.len() < 8 + 8 + 1 + 8 {
            return Err(SyncError::CheckpointCorrupted);
        }

        let last_stream_sequence = u64::from_le_bytes(
            data[0..8]
                .try_into()
                .map_err(|_| SyncError::CheckpointCorrupted)?,
        );
        let last_slot = u64::from_le_bytes(
            data[8..16]
                .try_into()
                .map_err(|_| SyncError::CheckpointCorrupted)?,
        );
        let has_event_id = data[16] != 0;
        let mut offset = 17;
        let last_event_id = if has_event_id {
            if data.len() < offset + 4 {
                return Err(SyncError::CheckpointCorrupted);
            }
            let len = u32::from_le_bytes(
                data[offset..offset + 4]
                    .try_into()
                    .map_err(|_| SyncError::CheckpointCorrupted)?,
            ) as usize;
            offset += 4;
            if data.len() < offset + len + 8 {
                return Err(SyncError::CheckpointCorrupted);
            }
            let event_id = String::from_utf8(data[offset..offset + len].to_vec())
                .map_err(|_| SyncError::CheckpointCorrupted)?;
            offset += len;
            Some(event_id)
        } else {
            None
        };

        if data.len() < offset + 8 {
            return Err(SyncError::CheckpointCorrupted);
        }
        let synced_at = i64::from_le_bytes(
            data[offset..offset + 8]
                .try_into()
                .map_err(|_| SyncError::CheckpointCorrupted)?,
        );

        Ok(Self {
            last_stream_sequence,
            last_slot,
            last_event_id,
            synced_at,
        })
    }
}

/// Loads a checkpoint from the store.
///
/// Returns `Ok(None)` if no checkpoint exists for the given key.
pub async fn load_checkpoint<S: KeyValueStore>(
    store: &S,
    key: &[u8],
) -> Result<Option<SyncCheckpoint>, SyncError> {
    let data = store.get(CHECKPOINTS, key).await?;
    match data {
        Some(bytes) => {
            let checkpoint = SyncCheckpoint::from_bytes(&bytes)?;
            Ok(Some(checkpoint))
        }
        None => Ok(None),
    }
}

/// Saves a checkpoint to the store.
pub async fn save_checkpoint<S: KeyValueStore>(
    store: &S,
    key: &[u8],
    checkpoint: &SyncCheckpoint,
) -> Result<(), SyncError> {
    let bytes = checkpoint.to_bytes()?;
    store.set(CHECKPOINTS, key, &bytes).await?;
    Ok(())
}

/// Deletes a checkpoint from the store.
pub async fn delete_checkpoint<S: KeyValueStore>(store: &S, key: &[u8]) -> Result<(), SyncError> {
    store.delete(CHECKPOINTS, key).await?;
    Ok(())
}

pub async fn load_event_checkpoint<S: KeyValueStore>(
    store: &S,
    key: &[u8],
) -> Result<Option<EventStreamCheckpoint>, SyncError> {
    let data = store.get(CHECKPOINTS, key).await?;
    match data {
        Some(bytes) => Ok(Some(EventStreamCheckpoint::from_bytes(&bytes)?)),
        None => Ok(None),
    }
}

pub async fn save_event_checkpoint<S: KeyValueStore>(
    store: &S,
    key: &[u8],
    checkpoint: &EventStreamCheckpoint,
) -> Result<(), SyncError> {
    store.set(CHECKPOINTS, key, &checkpoint.to_bytes()).await?;
    Ok(())
}

fn current_timestamp_millis() -> i64 {
    #[cfg(target_arch = "wasm32")]
    {
        return js_sys::Date::now() as i64;
    }

    #[cfg(not(target_arch = "wasm32"))]
    use std::time::{SystemTime, UNIX_EPOCH};

    #[cfg(not(target_arch = "wasm32"))]
    {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }
}

// TODO: Uncomment when test dependencies are available
// #[cfg(test)]
// mod tests {
//     use super::*;
//     use rstest::{fixture, rstest};
//     use surf_store::MemoryStore;
//
//     #[fixture]
//     fn store() -> MemoryStore {
//         MemoryStore::new()
//     }
//
//     #[fixture]
//     fn program_id() -> Pubkey {
//         Pubkey::new_unique()
//     }
//
//     #[fixture]
//     fn checkpoint(program_id: Pubkey) -> SyncCheckpoint {
//         SyncCheckpoint {
//             program_id,
//             last_slot: 12345,
//             last_signature: Some([1u8; 64]),
//             account_count: 100,
//             synced_at: 1700000000000,
//         }
//     }
//
//     #[rstest]
//     fn test_checkpoint_serialization_roundtrip(checkpoint: SyncCheckpoint) {
//         let bytes = checkpoint.to_bytes().unwrap();
//         let decoded = SyncCheckpoint::from_bytes(&bytes).unwrap();
//         assert_eq!(checkpoint.program_id, decoded.program_id);
//         assert_eq!(checkpoint.last_slot, decoded.last_slot);
//         assert_eq!(checkpoint.last_signature, decoded.last_signature);
//         assert_eq!(checkpoint.account_count, decoded.account_count);
//     }
//
//     #[rstest]
//     fn test_checkpoint_serialization_no_signature(program_id: Pubkey) {
//         let checkpoint = SyncCheckpoint {
//             program_id,
//             last_slot: 500,
//             last_signature: None,
//             account_count: 50,
//             synced_at: 1700000000000,
//         };
//         let bytes = checkpoint.to_bytes().unwrap();
//         let decoded = SyncCheckpoint::from_bytes(&bytes).unwrap();
//         assert_eq!(checkpoint.program_id, decoded.program_id);
//         assert_eq!(checkpoint.last_slot, decoded.last_slot);
//         assert!(decoded.last_signature.is_none());
//     }
//
//     #[rstest]
//     fn test_checkpoint_new(program_id: Pubkey) {
//         let checkpoint = SyncCheckpoint::new(program_id, 100);
//         assert_eq!(checkpoint.program_id, program_id);
//         assert_eq!(checkpoint.last_slot, 100);
//         assert_eq!(checkpoint.account_count, 0);
//         assert!(checkpoint.last_signature.is_none());
//     }
//
//     #[rstest]
//     #[tokio::test]
//     async fn test_load_checkpoint_not_found(store: MemoryStore) {
//         let result = load_checkpoint(&store, NAME_SYNC_KEY).await.unwrap();
//         assert!(result.is_none());
//     }
//
//     #[rstest]
//     #[tokio::test]
//     async fn test_save_and_load_checkpoint(store: MemoryStore, program_id: Pubkey) {
//         let checkpoint = SyncCheckpoint::new(program_id, 500);
//         save_checkpoint(&store, NAME_SYNC_KEY, &checkpoint)
//             .await
//             .unwrap();
//
//         let loaded = load_checkpoint(&store, NAME_SYNC_KEY)
//             .await
//             .unwrap()
//             .unwrap();
//         assert_eq!(loaded.program_id, checkpoint.program_id);
//         assert_eq!(loaded.last_slot, 500);
//     }
//
//     #[rstest]
//     #[tokio::test]
//     async fn test_delete_checkpoint(store: MemoryStore, program_id: Pubkey) {
//         let checkpoint = SyncCheckpoint::new(program_id, 300);
//         save_checkpoint(&store, NAME_SYNC_KEY, &checkpoint)
//             .await
//             .unwrap();
//
//         delete_checkpoint(&store, NAME_SYNC_KEY).await.unwrap();
//         let result = load_checkpoint(&store, NAME_SYNC_KEY).await.unwrap();
//         assert!(result.is_none());
//     }
// }
