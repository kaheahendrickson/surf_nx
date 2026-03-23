use std::sync::Arc;

use solana_signature::Signature;
use surf_store::{KeyValueStore, FOLLOWS};

use crate::checkpoint::{save_checkpoint, SyncCheckpoint, FOLLOW_SYNC_KEY};
use crate::config::SyncConfig;
use crate::error::SyncError;
use crate::parser::{is_signal_instruction, parse_signal_instruction};

const FOLLOW_RECORD_LEN: usize = 8 + 8 + 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FollowRecord {
    pub slot: u64,
    pub block_time: i64,
    pub signature: [u8; 64],
}

impl FollowRecord {
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(FOLLOW_RECORD_LEN);
        bytes.extend_from_slice(&self.slot.to_le_bytes());
        bytes.extend_from_slice(&self.block_time.to_le_bytes());
        bytes.extend_from_slice(&self.signature);
        bytes
    }

    pub fn decode(data: &[u8]) -> Result<Self, SyncError> {
        if data.len() != FOLLOW_RECORD_LEN {
            return Err(SyncError::InvalidInstruction);
        }

        let slot = u64::from_le_bytes(
            data[0..8]
                .try_into()
                .map_err(|_| SyncError::InvalidInstruction)?,
        );
        let block_time = i64::from_le_bytes(
            data[8..16]
                .try_into()
                .map_err(|_| SyncError::InvalidInstruction)?,
        );
        let mut signature = [0u8; 64];
        signature.copy_from_slice(&data[16..80]);
        Ok(Self {
            slot,
            block_time,
            signature,
        })
    }
}

pub async fn apply_follow_created<S: KeyValueStore>(
    store: &S,
    target: &solana_pubkey::Pubkey,
    record: &FollowRecord,
) -> Result<(), SyncError> {
    store.set(FOLLOWS, target.as_ref(), &record.encode()).await?;
    Ok(())
}

pub async fn apply_follow_removed<S: KeyValueStore>(
    store: &S,
    target: &solana_pubkey::Pubkey,
) -> Result<(), SyncError> {
    store.delete(FOLLOWS, target.as_ref()).await?;
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub struct FollowSyncer<B: surf_client::Backend> {
    backend: Arc<B>,
    config: SyncConfig,
}

#[cfg(target_arch = "wasm32")]
pub struct FollowSyncer<B: surf_client::WasmBackend> {
    backend: Arc<B>,
    config: SyncConfig,
}

macro_rules! impl_follow_syncer {
    ($backend_trait:path) => {
        impl<B: $backend_trait> FollowSyncer<B> {
            pub fn new(backend: Arc<B>, config: SyncConfig) -> Self {
                Self { backend, config }
            }

            pub async fn sync<S: KeyValueStore>(
                &self,
                store: &S,
                checkpoint: Option<&SyncCheckpoint>,
            ) -> Result<SyncCheckpoint, SyncError> {
                let options = surf_client::SignaturesForAddressOptions {
                    limit: Some(self.config.transaction_history_limit),
                    ..Default::default()
                };

                let signatures = self
                    .backend
                    .get_signatures_for_address(&self.config.tracked_balance, Some(options))
                    .await?;

                let mut next = checkpoint
                    .cloned()
                    .unwrap_or_else(|| SyncCheckpoint::new(self.config.signals_program, 0));
                let mut max_slot = next.last_slot;
                let mut latest_signature: Option<[u8; 64]> = None;

                for sig_info in signatures {
                    if sig_info.slot <= next.last_slot {
                        continue;
                    }

                    let Some(tx) = self.backend.get_transaction(&sig_info.signature).await? else {
                        continue;
                    };

                    for instruction in &tx.message.instructions {
                        let Some(program_id) = tx
                            .message
                            .account_keys
                            .get(instruction.program_id_index as usize)
                            .copied()
                        else {
                            continue;
                        };

                        if program_id != self.config.signals_program {
                            continue;
                        }

                        if !is_signal_instruction(&instruction.data) {
                            continue;
                        }

                        let accounts = instruction
                            .accounts
                            .iter()
                            .filter_map(|index| tx.message.account_keys.get(*index as usize).copied())
                            .collect::<Vec<_>>();
                        if accounts.first() != Some(&self.config.tracked_balance) {
                            continue;
                        }

                        let parsed = parse_signal_instruction(&instruction.data)?;
                        match parsed.kind {
                            surf_protocol::SignalKind::Follow => {
                                let record = FollowRecord {
                                    slot: tx.slot,
                                    block_time: tx.block_time.unwrap_or(-1),
                                    signature: signature_bytes(&sig_info.signature),
                                };
                                apply_follow_created(store, &parsed.target, &record).await?;
                            }
                            surf_protocol::SignalKind::Unfollow => {
                                apply_follow_removed(store, &parsed.target).await?;
                            }
                        }

                        if sig_info.slot >= max_slot {
                            max_slot = sig_info.slot;
                            latest_signature = Some(signature_bytes(&sig_info.signature));
                        }
                    }
                }

                next.update(max_slot, latest_signature);
                save_checkpoint(store, FOLLOW_SYNC_KEY, &next).await?;
                Ok(next)
            }
        }
    };
}

fn signature_bytes(signature: &Signature) -> [u8; 64] {
    let mut bytes = [0u8; 64];
    bytes.copy_from_slice(signature.as_ref());
    bytes
}

#[cfg(not(target_arch = "wasm32"))]
impl_follow_syncer!(surf_client::Backend);

#[cfg(target_arch = "wasm32")]
impl_follow_syncer!(surf_client::WasmBackend);
