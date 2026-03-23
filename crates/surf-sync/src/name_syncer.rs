//! Name syncer for synchronizing name registry accounts.

use std::sync::Arc;

use surf_protocol::{decode_name_record, derive_name_record_pda, NameRecord};
use surf_store::{KeyValueStore, NAMES};

use crate::checkpoint::{save_checkpoint, SyncCheckpoint, NAME_SYNC_KEY};
use crate::config::SyncConfig;
use crate::error::SyncError;
use crate::parser::{is_register_instruction, parse_register_instruction};

pub async fn apply_name_record<S: KeyValueStore>(
    store: &S,
    name: &str,
    record: &[u8],
) -> Result<(), SyncError> {
    store.set(NAMES, name.as_bytes(), record).await?;
    Ok(())
}

/// Name syncer for synchronizing name registry accounts.
#[cfg(not(target_arch = "wasm32"))]
pub struct NameSyncer<B: surf_client::Backend> {
    backend: Arc<B>,
    config: SyncConfig,
}

#[cfg(not(target_arch = "wasm32"))]
impl<B: surf_client::Backend> NameSyncer<B> {
    pub fn new(backend: Arc<B>, config: SyncConfig) -> Self {
        Self { backend, config }
    }

    pub async fn bootstrap<S: KeyValueStore>(
        &self,
        store: &S,
    ) -> Result<SyncCheckpoint, SyncError> {
        use surf_client::ProgramAccountsFilter;

        let filter = ProgramAccountsFilter {
            data_size: Some(NameRecord::LEN),
        };

        let accounts = self
            .backend
            .get_program_accounts(&self.config.registry_program, Some(filter))
            .await?;

        for account_info in accounts {
            if account_info.account.data.len() < NameRecord::LEN {
                continue;
            }

            if let Some(record) = decode_name_record(&account_info.account.data) {
                let name_slice = &record.name[..record.len as usize];
                let (expected_pda, _) =
                    derive_name_record_pda(name_slice, &self.config.registry_program);

                if account_info.pubkey != expected_pda {
                    continue;
                }

                    let key = normalize_name_key(name_slice);
                    store.set(NAMES, &key, &account_info.account.data).await?;
            }
        }

        let checkpoint = SyncCheckpoint::new(self.config.registry_program, 0);
        save_checkpoint(store, NAME_SYNC_KEY, &checkpoint).await?;
        Ok(checkpoint)
    }

    pub async fn sync_incremental<S: KeyValueStore>(
        &self,
        store: &S,
        checkpoint: &SyncCheckpoint,
    ) -> Result<SyncCheckpoint, SyncError> {
        use surf_client::SignaturesForAddressOptions;

        let options = SignaturesForAddressOptions::default();

        let signatures = self
            .backend
            .get_signatures_for_address(&self.config.registry_program, Some(options))
            .await?;

        if signatures.is_empty() {
            return Ok(checkpoint.clone());
        }

        let mut new_checkpoint = checkpoint.clone();
        let mut max_slot = checkpoint.last_slot;

        for sig_info in signatures {
            if sig_info.slot <= checkpoint.last_slot {
                continue;
            }

            let tx = self.backend.get_transaction(&sig_info.signature).await?;

            if let Some(tx) = tx {
                let updated = self.process_transaction(store, &tx).await?;
                if updated && sig_info.slot > max_slot {
                    max_slot = sig_info.slot;
                }
            }
        }

        new_checkpoint.last_slot = max_slot;
        save_checkpoint(store, NAME_SYNC_KEY, &new_checkpoint).await?;
        Ok(new_checkpoint)
    }

    async fn process_transaction<S: KeyValueStore>(
        &self,
        store: &S,
        tx: &surf_client::ParsedTransaction,
    ) -> Result<bool, SyncError> {
        let mut updated = false;

        for instruction in &tx.message.instructions {
            if !is_register_instruction(&instruction.data) {
                continue;
            }

            if let Ok(parsed) = parse_register_instruction(&instruction.data) {
                let name_slice = &parsed.name[..parsed.name_len as usize];
                let (expected_pda, _) =
                    derive_name_record_pda(name_slice, &self.config.registry_program);

                let account = self.backend.get_account(&expected_pda).await?;

                if let Some(account) = account {
                    let key = normalize_name_key(name_slice);
                    store.set(NAMES, &key, &account.data).await?;
                    updated = true;
                }
            }
        }

        Ok(updated)
    }
}

/// Name syncer for synchronizing name registry accounts (WASM version).
#[cfg(target_arch = "wasm32")]
pub struct NameSyncer<B: surf_client::WasmBackend> {
    backend: Arc<B>,
    config: SyncConfig,
}

#[cfg(target_arch = "wasm32")]
impl<B: surf_client::WasmBackend> NameSyncer<B> {
    pub fn new(backend: Arc<B>, config: SyncConfig) -> Self {
        Self { backend, config }
    }

    pub async fn bootstrap<S: KeyValueStore>(
        &self,
        store: &S,
    ) -> Result<SyncCheckpoint, SyncError> {
        use surf_client::ProgramAccountsFilter;

        let filter = ProgramAccountsFilter {
            data_size: Some(NameRecord::LEN),
        };

        let accounts = self
            .backend
            .get_program_accounts(&self.config.registry_program, Some(filter))
            .await?;

        for account_info in accounts {
            if account_info.account.data.len() < NameRecord::LEN {
                continue;
            }

            if let Some(record) = decode_name_record(&account_info.account.data) {
                let name_slice = &record.name[..record.len as usize];
                let (expected_pda, _) =
                    derive_name_record_pda(name_slice, &self.config.registry_program);

                if account_info.pubkey != expected_pda {
                    continue;
                }

                let key = normalize_name_key(name_slice);
                store.set(NAMES, &key, &account_info.account.data).await?;
            }
        }

        let checkpoint = SyncCheckpoint::new(self.config.registry_program, 0);
        save_checkpoint(store, NAME_SYNC_KEY, &checkpoint).await?;
        Ok(checkpoint)
    }

    pub async fn sync_incremental<S: KeyValueStore>(
        &self,
        store: &S,
        checkpoint: &SyncCheckpoint,
    ) -> Result<SyncCheckpoint, SyncError> {
        use surf_client::SignaturesForAddressOptions;

        let options = SignaturesForAddressOptions::default();

        let signatures = self
            .backend
            .get_signatures_for_address(&self.config.registry_program, Some(options))
            .await?;

        if signatures.is_empty() {
            return Ok(checkpoint.clone());
        }

        let mut new_checkpoint = checkpoint.clone();
        let mut max_slot = checkpoint.last_slot;

        for sig_info in signatures {
            if sig_info.slot <= checkpoint.last_slot {
                continue;
            }

            let tx = self.backend.get_transaction(&sig_info.signature).await?;

            if let Some(tx) = tx {
                let updated = self.process_transaction(store, &tx).await?;
                if updated && sig_info.slot > max_slot {
                    max_slot = sig_info.slot;
                }
            }
        }

        new_checkpoint.last_slot = max_slot;
        save_checkpoint(store, NAME_SYNC_KEY, &new_checkpoint).await?;
        Ok(new_checkpoint)
    }

    async fn process_transaction<S: KeyValueStore>(
        &self,
        store: &S,
        tx: &surf_client::ParsedTransaction,
    ) -> Result<bool, SyncError> {
        let mut updated = false;

        for instruction in &tx.message.instructions {
            if !is_register_instruction(&instruction.data) {
                continue;
            }

            if let Ok(parsed) = parse_register_instruction(&instruction.data) {
                let name_slice = &parsed.name[..parsed.name_len as usize];
                let (expected_pda, _) =
                    derive_name_record_pda(name_slice, &self.config.registry_program);

                let account = self.backend.get_account(&expected_pda).await?;

                if let Some(account) = account {
                    let key = normalize_name_key(name_slice);
                    store.set(NAMES, &key, &account.data).await?;
                    updated = true;
                }
            }
        }

        Ok(updated)
    }
}

fn normalize_name_key(name: &[u8]) -> Vec<u8> {
    name.to_vec()
}

pub fn encode_name_record(owner: &solana_pubkey::Pubkey, name: &str) -> Vec<u8> {
    let mut data = vec![0u8; NameRecord::LEN];
    data[0..32].copy_from_slice(owner.as_ref());
    let name_bytes = name.as_bytes();
    let copy_len = name_bytes.len().min(32);
    data[32..32 + copy_len].copy_from_slice(&name_bytes[..copy_len]);
    data[64] = copy_len as u8;
    data
}
