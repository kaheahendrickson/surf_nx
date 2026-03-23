//! Sync service orchestrator.

use std::sync::Arc;
use std::time::Duration;

use surf_store::KeyValueStore;

use crate::activity_syncer::ActivitySyncer;
use crate::activity_event_syncer::ActivityEventSyncer;
use crate::balance_syncer::BalanceSyncer;
use crate::balance_event_syncer::BalanceEventSyncer;
use crate::checkpoint::{
    delete_checkpoint, load_checkpoint, load_event_checkpoint, SyncCheckpoint, SyncServiceState,
    SyncState, ACTIVITY_EVENT_SYNC_KEY, ACTIVITY_SYNC_KEY, BALANCE_EVENT_SYNC_KEY,
    BALANCE_SYNC_KEY, FOLLOW_EVENT_SYNC_KEY, FOLLOW_SYNC_KEY, NAME_EVENT_SYNC_KEY, NAME_SYNC_KEY,
};
use crate::config::SyncConfig;
use crate::error::SyncError;
use crate::follow_event_syncer::FollowEventSyncer;
use crate::follow_syncer::FollowSyncer;
use crate::name_event_syncer::NameEventSyncer;
use crate::name_syncer::NameSyncer;
use crate::sleep::SleepProvider;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(module = "/src/follow_event_syncer_wasm.js")]
extern "C" {
    #[wasm_bindgen(js_name = emitSyncUpdate)]
    fn js_emit_sync_update(domain: &str);
}

/// Sync service for orchestrating name and balance synchronization.
#[cfg(not(target_arch = "wasm32"))]
pub struct SyncService<B: surf_client::Backend, S: KeyValueStore, SP: SleepProvider> {
    backend: Arc<B>,
    store: S,
    config: SyncConfig,
    sleep_provider: SP,
    state: SyncServiceState,
}

#[cfg(not(target_arch = "wasm32"))]
impl<B: surf_client::Backend, S: KeyValueStore, SP: SleepProvider> SyncService<B, S, SP> {
    pub fn new(
        backend: Arc<B>,
        store: S,
        config: SyncConfig,
        sleep_provider: SP,
    ) -> Result<Self, SyncError> {
        config.validate()?;

        Ok(Self {
            backend,
            store,
            config,
            sleep_provider,
            state: SyncServiceState::Idle,
        })
    }

    pub async fn sync(&mut self) -> Result<(), SyncError> {
        self.state = SyncServiceState::Bootstrapping;

        let result = self.run_sync().await;

        if result.is_err() {
            self.state = SyncServiceState::Idle;
        }

        result
    }

    async fn run_sync(&mut self) -> Result<(), SyncError> {
        let name_checkpoint = load_checkpoint(&self.store, NAME_SYNC_KEY).await?;
        let activity_checkpoint = load_checkpoint(&self.store, ACTIVITY_SYNC_KEY).await?;
        let follow_checkpoint = load_checkpoint(&self.store, FOLLOW_SYNC_KEY).await?;

        let name_syncer = NameSyncer::new(Arc::clone(&self.backend), self.config.clone());

        self.state = SyncServiceState::Streaming;

        let balance_syncer = BalanceSyncer::new(Arc::clone(&self.backend), self.config.clone());
        let activity_syncer = ActivitySyncer::new(Arc::clone(&self.backend), self.config.clone());
        let follow_syncer = FollowSyncer::new(Arc::clone(&self.backend), self.config.clone());
        let name_event_syncer = if let Some(events_config) = self.config.event_stream.clone() {
            Some(NameEventSyncer::connect(events_config.clone()).await?)
        } else { None };
        let balance_event_syncer = if let Some(events_config) = self.config.event_stream.clone() {
            Some(BalanceEventSyncer::connect(events_config.clone()).await?)
        } else { None };
        let activity_event_syncer = if let Some(events_config) = self.config.event_stream.clone() {
            Some(ActivityEventSyncer::connect(events_config.clone()).await?)
        } else { None };
        let follow_event_syncer = if let Some(events_config) = self.config.event_stream.clone() {
            Some(FollowEventSyncer::connect(events_config).await?)
        } else {
            None
        };
        let mut name_checkpoint = if let Some(event_syncer) = &name_event_syncer {
            let event_checkpoint = event_syncer.sync_available(&self.store).await?;
            let mut checkpoint = name_checkpoint.unwrap_or_else(|| SyncCheckpoint::new(self.config.registry_program, 0));
            checkpoint.update(checkpoint.last_slot.max(event_checkpoint.last_slot), None);
            checkpoint
        } else {
            match name_checkpoint {
                Some(cp) => cp,
                None => name_syncer.bootstrap(&self.store).await?,
            }
        };
        if let Some(event_syncer) = &balance_event_syncer {
            let event_checkpoint = event_syncer.sync_available(&self.store, &self.config.tracked_balance).await?;
            let mut checkpoint = load_checkpoint(&self.store, BALANCE_SYNC_KEY).await?.unwrap_or_else(|| SyncCheckpoint::new(self.config.token_program, 0));
            checkpoint.update(checkpoint.last_slot.max(event_checkpoint.last_slot), None);
        } else {
            let _ = balance_syncer.sync_balance(&self.store).await?;
        }
        let mut activity_checkpoint = if let Some(event_syncer) = &activity_event_syncer {
            let event_checkpoint = event_syncer.sync_available(&self.store, &self.config.tracked_balance).await?;
            let mut checkpoint = activity_checkpoint.unwrap_or_else(|| SyncCheckpoint::new(self.config.tracked_balance, 0));
            checkpoint.update(checkpoint.last_slot.max(event_checkpoint.last_slot), None);
            checkpoint
        } else { activity_syncer.sync(&self.store, activity_checkpoint.as_ref()).await? };
        let mut follow_checkpoint = if let Some(event_syncer) = &follow_event_syncer {
            let event_checkpoint = event_syncer
                .sync_available(&self.store, &self.config.tracked_balance)
                .await?;
            let mut checkpoint = follow_checkpoint
                .unwrap_or_else(|| SyncCheckpoint::new(self.config.signals_program, 0));
            checkpoint.update(checkpoint.last_slot.max(event_checkpoint.last_slot), None);
            checkpoint
        } else {
            follow_syncer.sync(&self.store, follow_checkpoint.as_ref()).await?
        };

        loop {
            if self.state == SyncServiceState::Stopped {
                break;
            }

            self.sleep_provider
                .sleep(Duration::from_millis(self.config.poll_interval_ms))
                .await;

            if self.state == SyncServiceState::Stopped {
                break;
            }

            name_checkpoint = if let Some(event_syncer) = &name_event_syncer {
                let event_checkpoint = event_syncer.sync_available(&self.store).await?;
                let mut checkpoint = name_checkpoint.clone();
                checkpoint.update(checkpoint.last_slot.max(event_checkpoint.last_slot), None);
                checkpoint
            } else { name_syncer.sync_incremental(&self.store, &name_checkpoint).await? };

            if let Some(event_syncer) = &balance_event_syncer {
                let _ = event_syncer.sync_available(&self.store, &self.config.tracked_balance).await?;
            } else {
                let _ = balance_syncer.sync_balance(&self.store).await?;
            }
            activity_checkpoint = if let Some(event_syncer) = &activity_event_syncer {
                let event_checkpoint = event_syncer.sync_available(&self.store, &self.config.tracked_balance).await?;
                let mut checkpoint = activity_checkpoint.clone();
                checkpoint.update(checkpoint.last_slot.max(event_checkpoint.last_slot), None);
                checkpoint
            } else { activity_syncer.sync(&self.store, Some(&activity_checkpoint)).await? };
            follow_checkpoint = if let Some(event_syncer) = &follow_event_syncer {
                let event_checkpoint = event_syncer
                    .sync_available(&self.store, &self.config.tracked_balance)
                    .await?;
                let mut checkpoint = follow_checkpoint.clone();
                checkpoint.update(checkpoint.last_slot.max(event_checkpoint.last_slot), None);
                checkpoint
            } else {
                follow_syncer.sync(&self.store, Some(&follow_checkpoint)).await?
            };
        }

        self.state = SyncServiceState::Idle;
        Ok(())
    }

    pub fn get_state(&self) -> SyncServiceState {
        self.state
    }

    pub async fn get_sync_state(&self) -> Result<SyncState, SyncError> {
        let name_checkpoint = load_checkpoint(&self.store, NAME_SYNC_KEY).await?;
        let balance_checkpoint = load_checkpoint(&self.store, BALANCE_SYNC_KEY).await?;
        let activity_checkpoint = load_checkpoint(&self.store, ACTIVITY_SYNC_KEY).await?;
        let follow_checkpoint = load_checkpoint(&self.store, FOLLOW_SYNC_KEY).await?;
        let follow_event_checkpoint = load_event_checkpoint(&self.store, FOLLOW_EVENT_SYNC_KEY).await?;
        let name_event_checkpoint = load_event_checkpoint(&self.store, NAME_EVENT_SYNC_KEY).await?;
        let balance_event_checkpoint = load_event_checkpoint(&self.store, BALANCE_EVENT_SYNC_KEY).await?;
        let activity_event_checkpoint = load_event_checkpoint(&self.store, ACTIVITY_EVENT_SYNC_KEY).await?;

        Ok(SyncState {
            name_checkpoint,
            balance_checkpoint,
            activity_checkpoint,
            follow_checkpoint,
            follow_event_checkpoint,
            name_event_checkpoint,
            balance_event_checkpoint,
            activity_event_checkpoint,
        })
    }

    pub async fn reset(&self) -> Result<(), SyncError> {
        delete_checkpoint(&self.store, NAME_SYNC_KEY).await?;
        delete_checkpoint(&self.store, BALANCE_SYNC_KEY).await?;
        delete_checkpoint(&self.store, ACTIVITY_SYNC_KEY).await?;
        delete_checkpoint(&self.store, FOLLOW_SYNC_KEY).await?;
        delete_checkpoint(&self.store, FOLLOW_EVENT_SYNC_KEY).await?;
        delete_checkpoint(&self.store, NAME_EVENT_SYNC_KEY).await?;
        delete_checkpoint(&self.store, BALANCE_EVENT_SYNC_KEY).await?;
        delete_checkpoint(&self.store, ACTIVITY_EVENT_SYNC_KEY).await?;
        Ok(())
    }

    pub fn stop(&mut self) {
        self.state = SyncServiceState::Stopped;
    }
}

/// Sync service for orchestrating name and balance synchronization (WASM version).
#[cfg(target_arch = "wasm32")]
pub struct SyncService<B: surf_client::WasmBackend, S: KeyValueStore, SP: SleepProvider> {
    backend: Arc<B>,
    store: S,
    config: SyncConfig,
    sleep_provider: SP,
    state: SyncServiceState,
}

#[cfg(target_arch = "wasm32")]
impl<B: surf_client::WasmBackend, S: KeyValueStore, SP: SleepProvider> SyncService<B, S, SP> {
    pub fn new(
        backend: Arc<B>,
        store: S,
        config: SyncConfig,
        sleep_provider: SP,
    ) -> Result<Self, SyncError> {
        config.validate()?;

        Ok(Self {
            backend,
            store,
            config,
            sleep_provider,
            state: SyncServiceState::Idle,
        })
    }

    pub async fn sync(&mut self) -> Result<(), SyncError> {
        self.state = SyncServiceState::Bootstrapping;

        let result = self.run_sync().await;

        if result.is_err() {
            self.state = SyncServiceState::Idle;
        }

        result
    }

    async fn run_sync(&mut self) -> Result<(), SyncError> {
        if self.config.event_stream.is_some() {
            return self.run_event_stream_sync().await;
        }

        let name_checkpoint = load_checkpoint(&self.store, NAME_SYNC_KEY).await?;
        let activity_checkpoint = load_checkpoint(&self.store, ACTIVITY_SYNC_KEY).await?;
        let follow_checkpoint = load_checkpoint(&self.store, FOLLOW_SYNC_KEY).await?;

        let name_syncer = NameSyncer::new(Arc::clone(&self.backend), self.config.clone());

        self.state = SyncServiceState::Streaming;

        let balance_syncer = BalanceSyncer::new(Arc::clone(&self.backend), self.config.clone());
        let activity_syncer = ActivitySyncer::new(Arc::clone(&self.backend), self.config.clone());
        let follow_syncer = FollowSyncer::new(Arc::clone(&self.backend), self.config.clone());
        let name_event_syncer = if let Some(events_config) = self.config.event_stream.clone() {
            Some(NameEventSyncer::connect(events_config.clone()).await?)
        } else { None };
        let balance_event_syncer = if let Some(events_config) = self.config.event_stream.clone() {
            Some(BalanceEventSyncer::connect(events_config.clone()).await?)
        } else { None };
        let activity_event_syncer = if let Some(events_config) = self.config.event_stream.clone() {
            Some(ActivityEventSyncer::connect(events_config.clone()).await?)
        } else { None };
        let follow_event_syncer = if let Some(events_config) = self.config.event_stream.clone() {
            Some(FollowEventSyncer::connect(events_config).await?)
        } else {
            None
        };
        let mut name_checkpoint = if let Some(event_syncer) = &name_event_syncer {
            let event_checkpoint = event_syncer.sync_available(&self.store).await?;
            let mut checkpoint = name_checkpoint.unwrap_or_else(|| SyncCheckpoint::new(self.config.registry_program, 0));
            checkpoint.update(checkpoint.last_slot.max(event_checkpoint.last_slot), None);
            checkpoint
        } else {
            match name_checkpoint {
                Some(cp) => cp,
                None => name_syncer.bootstrap(&self.store).await?,
            }
        };
        if let Some(event_syncer) = &balance_event_syncer {
            let event_checkpoint = event_syncer.sync_available(&self.store, &self.config.tracked_balance).await?;
            let mut checkpoint = load_checkpoint(&self.store, BALANCE_SYNC_KEY).await?.unwrap_or_else(|| SyncCheckpoint::new(self.config.token_program, 0));
            checkpoint.update(checkpoint.last_slot.max(event_checkpoint.last_slot), None);
        } else {
            let _ = balance_syncer.sync_balance(&self.store).await?;
        }
        let mut activity_checkpoint = if let Some(event_syncer) = &activity_event_syncer {
            let event_checkpoint = event_syncer.sync_available(&self.store, &self.config.tracked_balance).await?;
            let mut checkpoint = activity_checkpoint.unwrap_or_else(|| SyncCheckpoint::new(self.config.tracked_balance, 0));
            checkpoint.update(checkpoint.last_slot.max(event_checkpoint.last_slot), None);
            checkpoint
        } else {
            activity_syncer.sync(&self.store, activity_checkpoint.as_ref()).await?
        };
        let mut follow_checkpoint = if let Some(event_syncer) = &follow_event_syncer {
            let event_checkpoint = event_syncer
                .sync_available(&self.store, &self.config.tracked_balance)
                .await?;
            let mut checkpoint = follow_checkpoint
                .unwrap_or_else(|| SyncCheckpoint::new(self.config.signals_program, 0));
            checkpoint.update(checkpoint.last_slot.max(event_checkpoint.last_slot), None);
            checkpoint
        } else {
            follow_syncer.sync(&self.store, follow_checkpoint.as_ref()).await?
        };

        loop {
            if self.state == SyncServiceState::Stopped {
                break;
            }

            self.sleep_provider
                .sleep(Duration::from_millis(self.config.poll_interval_ms))
                .await;

            if self.state == SyncServiceState::Stopped {
                break;
            }

            name_checkpoint = if let Some(event_syncer) = &name_event_syncer {
                let event_checkpoint = event_syncer.sync_available(&self.store).await?;
                let mut checkpoint = name_checkpoint.clone();
                checkpoint.update(checkpoint.last_slot.max(event_checkpoint.last_slot), None);
                checkpoint
            } else {
                name_syncer.sync_incremental(&self.store, &name_checkpoint).await?
            };

            if let Some(event_syncer) = &balance_event_syncer {
                let _ = event_syncer.sync_available(&self.store, &self.config.tracked_balance).await?;
            } else {
                let _ = balance_syncer.sync_balance(&self.store).await?;
            }
            activity_checkpoint = if let Some(event_syncer) = &activity_event_syncer {
                let event_checkpoint = event_syncer.sync_available(&self.store, &self.config.tracked_balance).await?;
                let mut checkpoint = activity_checkpoint.clone();
                checkpoint.update(checkpoint.last_slot.max(event_checkpoint.last_slot), None);
                checkpoint
            } else {
                activity_syncer.sync(&self.store, Some(&activity_checkpoint)).await?
            };
            follow_checkpoint = if let Some(event_syncer) = &follow_event_syncer {
                let event_checkpoint = event_syncer
                    .sync_available(&self.store, &self.config.tracked_balance)
                    .await?;
                let mut checkpoint = follow_checkpoint.clone();
                checkpoint.update(checkpoint.last_slot.max(event_checkpoint.last_slot), None);
                checkpoint
            } else {
                follow_syncer.sync(&self.store, Some(&follow_checkpoint)).await?
            };
        }

        self.state = SyncServiceState::Idle;
        Ok(())
    }

    async fn run_event_stream_sync(&mut self) -> Result<(), SyncError> {
        let events_config = self
            .config
            .event_stream
            .clone()
            .ok_or_else(|| SyncError::InvalidConfig("missing event stream config".to_string()))?;

        let name_checkpoint = load_checkpoint(&self.store, NAME_SYNC_KEY).await?;
        let activity_checkpoint = load_checkpoint(&self.store, ACTIVITY_SYNC_KEY).await?;
        let follow_checkpoint = load_checkpoint(&self.store, FOLLOW_SYNC_KEY).await?;

        let name_syncer = NameSyncer::new(Arc::clone(&self.backend), self.config.clone());
        let balance_syncer = BalanceSyncer::new(Arc::clone(&self.backend), self.config.clone());
        let activity_syncer = ActivitySyncer::new(Arc::clone(&self.backend), self.config.clone());
        let follow_syncer = FollowSyncer::new(Arc::clone(&self.backend), self.config.clone());

        match name_checkpoint {
            Some(checkpoint) => {
                let _ = name_syncer.sync_incremental(&self.store, &checkpoint).await?;
            }
            None => {
                let _ = name_syncer.bootstrap(&self.store).await?;
            }
        }
        let _ = balance_syncer.sync_balance(&self.store).await?;
        let _ = activity_syncer
            .sync(&self.store, activity_checkpoint.as_ref())
            .await?;
        let _ = follow_syncer.sync(&self.store, follow_checkpoint.as_ref()).await?;

        js_emit_sync_update("names");
        js_emit_sync_update("balances");
        js_emit_sync_update("activity");
        js_emit_sync_update("follows");

        self.state = SyncServiceState::Streaming;

        let name_event_syncer = NameEventSyncer::connect(events_config.clone()).await?;
        let balance_event_syncer = BalanceEventSyncer::connect(events_config.clone()).await?;
        let activity_event_syncer = ActivityEventSyncer::connect(events_config.clone()).await?;
        let follow_event_syncer = FollowEventSyncer::connect(events_config).await?;

        futures::future::try_join4(
            name_event_syncer.stream_updates(&self.store),
            balance_event_syncer.stream_updates(&self.store, &self.config.tracked_balance),
            activity_event_syncer.stream_updates(&self.store, &self.config.tracked_balance),
            follow_event_syncer.stream_updates(&self.store, &self.config.tracked_balance),
        )
        .await?;

        self.state = SyncServiceState::Idle;
        Ok(())
    }

    pub fn get_state(&self) -> SyncServiceState {
        self.state
    }

    pub async fn get_sync_state(&self) -> Result<SyncState, SyncError> {
        let name_checkpoint = load_checkpoint(&self.store, NAME_SYNC_KEY).await?;
        let balance_checkpoint = load_checkpoint(&self.store, BALANCE_SYNC_KEY).await?;
        let activity_checkpoint = load_checkpoint(&self.store, ACTIVITY_SYNC_KEY).await?;
        let follow_checkpoint = load_checkpoint(&self.store, FOLLOW_SYNC_KEY).await?;
        let follow_event_checkpoint = load_event_checkpoint(&self.store, FOLLOW_EVENT_SYNC_KEY).await?;
        let name_event_checkpoint = load_event_checkpoint(&self.store, NAME_EVENT_SYNC_KEY).await?;
        let balance_event_checkpoint = load_event_checkpoint(&self.store, BALANCE_EVENT_SYNC_KEY).await?;
        let activity_event_checkpoint = load_event_checkpoint(&self.store, ACTIVITY_EVENT_SYNC_KEY).await?;

        Ok(SyncState {
            name_checkpoint,
            balance_checkpoint,
            activity_checkpoint,
            follow_checkpoint,
            follow_event_checkpoint,
            name_event_checkpoint,
            balance_event_checkpoint,
            activity_event_checkpoint,
        })
    }

    pub async fn reset(&self) -> Result<(), SyncError> {
        delete_checkpoint(&self.store, NAME_SYNC_KEY).await?;
        delete_checkpoint(&self.store, BALANCE_SYNC_KEY).await?;
        delete_checkpoint(&self.store, ACTIVITY_SYNC_KEY).await?;
        delete_checkpoint(&self.store, FOLLOW_SYNC_KEY).await?;
        delete_checkpoint(&self.store, FOLLOW_EVENT_SYNC_KEY).await?;
        delete_checkpoint(&self.store, NAME_EVENT_SYNC_KEY).await?;
        delete_checkpoint(&self.store, BALANCE_EVENT_SYNC_KEY).await?;
        delete_checkpoint(&self.store, ACTIVITY_EVENT_SYNC_KEY).await?;
        Ok(())
    }

    pub fn stop(&mut self) {
        self.state = SyncServiceState::Stopped;
    }
}
