//! Balance synchronization for a single pubkey.

use std::sync::Arc;

use surf_protocol::TokenBalance;
use surf_store::{KeyValueStore, BALANCES, LAMPORTS};

use crate::checkpoint::{save_checkpoint, SyncCheckpoint, BALANCE_SYNC_KEY};
use crate::config::SyncConfig;
use crate::error::SyncError;

pub async fn apply_balance_record<S: KeyValueStore>(
    store: &S,
    owner: &solana_pubkey::Pubkey,
    record: &[u8],
) -> Result<(), SyncError> {
    store.set(BALANCES, owner.as_ref(), record).await?;
    Ok(())
}

pub async fn apply_lamports_record<S: KeyValueStore>(
    store: &S,
    owner: &solana_pubkey::Pubkey,
    lamports: u64,
) -> Result<(), SyncError> {
    store.set(LAMPORTS, owner.as_ref(), &lamports.to_le_bytes()).await?;
    Ok(())
}

/// Balance syncer for synchronizing a single pubkey's token balance.
#[cfg(not(target_arch = "wasm32"))]
pub struct BalanceSyncer<B: surf_client::Backend> {
    backend: Arc<B>,
    config: SyncConfig,
}

#[cfg(not(target_arch = "wasm32"))]
impl<B: surf_client::Backend> BalanceSyncer<B> {
    pub fn new(backend: Arc<B>, config: SyncConfig) -> Self {
        Self { backend, config }
    }

    pub async fn sync_balance<S: KeyValueStore>(
        &self,
        store: &S,
    ) -> Result<SyncCheckpoint, SyncError> {
        let (pda, _bump) = surf_protocol::derive_token_balance_pda(
            &self.config.tracked_balance,
            &self.config.token_program,
        );

        let account = self.backend.get_account(&pda).await?;

        match account {
            Some(account) => {
                if account.data.len() < TokenBalance::LEN {
                    return Err(SyncError::InvalidAccountData(pda));
                }

                let key = self.config.tracked_balance.as_ref().to_vec();
                store.set(BALANCES, &key, &account.data).await?;
            }
            None => {
                let key = self.config.tracked_balance.as_ref().to_vec();
                store.set(BALANCES, &key, &[]).await?;
            }
        }

        let lamports = self
            .backend
            .get_balance(&self.config.tracked_balance)
            .await?;
        let lamports_key = self.config.tracked_balance.as_ref().to_vec();
        let lamports_value = lamports.unwrap_or(0).to_le_bytes();
        store.set(LAMPORTS, &lamports_key, &lamports_value).await?;

        let mut checkpoint = SyncCheckpoint::new(self.config.token_program, 0);
        checkpoint.increment_accounts(1);
        save_checkpoint(store, BALANCE_SYNC_KEY, &checkpoint).await?;

        Ok(checkpoint)
    }

    pub async fn get_current_balance<S: KeyValueStore>(
        &self,
        store: &S,
    ) -> Result<Option<u64>, SyncError> {
        let key = self.config.tracked_balance.as_ref().to_vec();
        let data = store.get(BALANCES, &key).await?;

        match data {
            Some(data) if data.len() >= TokenBalance::LEN => {
                let balance = surf_protocol::decode_token_balance(&data)
                    .ok_or_else(|| SyncError::InvalidAccountData(self.config.tracked_balance))?;
                Ok(Some(balance.amount))
            }
            _ => Ok(None),
        }
    }

    pub async fn get_current_lamports<S: KeyValueStore>(
        &self,
        store: &S,
    ) -> Result<Option<u64>, SyncError> {
        let key = self.config.tracked_balance.as_ref().to_vec();
        let data = store.get(LAMPORTS, &key).await?;

        match data {
            Some(data) if data.len() == 8 => {
                Ok(Some(u64::from_le_bytes(data.try_into().map_err(|_| {
                    SyncError::InvalidAccountData(self.config.tracked_balance)
                })?)))
            }
            Some(_) => Err(SyncError::InvalidAccountData(self.config.tracked_balance)),
            None => Ok(None),
        }
    }
}

/// Balance syncer for synchronizing a single pubkey's token balance (WASM version).
#[cfg(target_arch = "wasm32")]
pub struct BalanceSyncer<B: surf_client::WasmBackend> {
    backend: Arc<B>,
    config: SyncConfig,
}

#[cfg(target_arch = "wasm32")]
impl<B: surf_client::WasmBackend> BalanceSyncer<B> {
    pub fn new(backend: Arc<B>, config: SyncConfig) -> Self {
        Self { backend, config }
    }

    pub async fn sync_balance<S: KeyValueStore>(
        &self,
        store: &S,
    ) -> Result<SyncCheckpoint, SyncError> {
        let (pda, _bump) = surf_protocol::derive_token_balance_pda(
            &self.config.tracked_balance,
            &self.config.token_program,
        );

        let account = self.backend.get_account(&pda).await?;

        match account {
            Some(account) => {
                if account.data.len() < TokenBalance::LEN {
                    return Err(SyncError::InvalidAccountData(pda));
                }

                let key = self.config.tracked_balance.as_ref().to_vec();
                store.set(BALANCES, &key, &account.data).await?;
            }
            None => {
                let key = self.config.tracked_balance.as_ref().to_vec();
                store.set(BALANCES, &key, &[]).await?;
            }
        }

        let lamports = self
            .backend
            .get_balance(&self.config.tracked_balance)
            .await?;
        let lamports_key = self.config.tracked_balance.as_ref().to_vec();
        let lamports_value = lamports.unwrap_or(0).to_le_bytes();
        store.set(LAMPORTS, &lamports_key, &lamports_value).await?;

        let mut checkpoint = SyncCheckpoint::new(self.config.token_program, 0);
        checkpoint.increment_accounts(1);
        save_checkpoint(store, BALANCE_SYNC_KEY, &checkpoint).await?;

        Ok(checkpoint)
    }

    pub async fn get_current_balance<S: KeyValueStore>(
        &self,
        store: &S,
    ) -> Result<Option<u64>, SyncError> {
        let key = self.config.tracked_balance.as_ref().to_vec();
        let data = store.get(BALANCES, &key).await?;

        match data {
            Some(data) if data.len() >= TokenBalance::LEN => {
                let balance = surf_protocol::decode_token_balance(&data)
                    .ok_or_else(|| SyncError::InvalidAccountData(self.config.tracked_balance))?;
                Ok(Some(balance.amount))
            }
            _ => Ok(None),
        }
    }

    pub async fn get_current_lamports<S: KeyValueStore>(
        &self,
        store: &S,
    ) -> Result<Option<u64>, SyncError> {
        let key = self.config.tracked_balance.as_ref().to_vec();
        let data = store.get(LAMPORTS, &key).await?;

        match data {
            Some(data) if data.len() == 8 => {
                Ok(Some(u64::from_le_bytes(data.try_into().map_err(|_| {
                    SyncError::InvalidAccountData(self.config.tracked_balance)
                })?)))
            }
            Some(_) => Err(SyncError::InvalidAccountData(self.config.tracked_balance)),
            None => Ok(None),
        }
    }
}
