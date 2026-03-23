use std::sync::Arc;

use solana_signature::Signature;
use surf_events::ActivityRecord;
use surf_store::{KeyValueStore, TRANSACTIONS};

use crate::checkpoint::{save_checkpoint, SyncCheckpoint, ACTIVITY_SYNC_KEY};
use crate::config::SyncConfig;
use crate::error::SyncError;
use crate::parser::parse_curated_activity;

pub async fn apply_activity_record<S: KeyValueStore>(
    store: &S,
    signature: &solana_signature::Signature,
    record: &ActivityRecord,
) -> Result<(), SyncError> {
    store
        .set(TRANSACTIONS, signature.as_ref(), &record.encode())
        .await?;
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub struct ActivitySyncer<B: surf_client::Backend> {
    backend: Arc<B>,
    config: SyncConfig,
}

#[cfg(target_arch = "wasm32")]
pub struct ActivitySyncer<B: surf_client::WasmBackend> {
    backend: Arc<B>,
    config: SyncConfig,
}

macro_rules! impl_activity_syncer {
    ($backend_trait:path) => {
        impl<B: $backend_trait> ActivitySyncer<B> {
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
                    .unwrap_or_else(|| SyncCheckpoint::new(self.config.tracked_balance, 0));
                let mut max_slot = next.last_slot;
                let mut latest_signature: Option<[u8; 64]> = None;

                for sig_info in signatures {
                    if sig_info.slot <= next.last_slot {
                        continue;
                    }

                    let Some(tx) = self.backend.get_transaction(&sig_info.signature).await? else {
                        continue;
                    };

                    let Some(activity) = tx.message.instructions.iter().find_map(|instruction| {
                        parse_curated_activity(
                            &tx,
                            instruction,
                            &self.config.tracked_balance,
                            &self.config.token_program,
                            &self.config.registry_program,
                            &self.config.signals_program,
                        )
                        .ok()
                        .flatten()
                    }) else {
                        continue;
                    };

                    let record = ActivityRecord {
                        kind: activity.kind,
                        counterparty: activity.counterparty,
                        amount: activity.amount,
                        slot: tx.slot,
                        block_time: tx.block_time.unwrap_or(-1),
                        signature: signature_bytes(&sig_info.signature),
                    };

                    apply_activity_record(store, &sig_info.signature, &record).await?;

                    if sig_info.slot >= max_slot {
                        max_slot = sig_info.slot;
                        latest_signature = Some(signature_bytes(&sig_info.signature));
                    }
                }

                next.update(max_slot, latest_signature);
                save_checkpoint(store, ACTIVITY_SYNC_KEY, &next).await?;
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
impl_activity_syncer!(surf_client::Backend);

#[cfg(target_arch = "wasm32")]
impl_activity_syncer!(surf_client::WasmBackend);
