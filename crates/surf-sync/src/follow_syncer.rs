use std::sync::Arc;

use solana_signature::Signature;
use surf_events::FollowRecord;
use surf_store::{KeyValueStore, FOLLOWS};

use crate::checkpoint::{save_checkpoint, SyncCheckpoint, FOLLOW_SYNC_KEY};
use crate::config::SyncConfig;
use crate::error::SyncError;
use crate::parser::{is_signal_instruction, parse_signal_instruction};

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
