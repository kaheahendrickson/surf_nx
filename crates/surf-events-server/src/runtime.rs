use async_nats::jetstream::stream;
use surf_client::{Backend, SignaturesForAddressOptions};
use surf_client_backend_http::HttpBackend;

use crate::checkpoint::{load_checkpoint, save_checkpoint, ServerCheckpointState};
use crate::config::ServerConfig;
use crate::error::ServerError;
use crate::publisher::JetStreamPublisher;
use crate::sync::activity::ActivitySyncService;
use crate::sync::balances::BalanceSyncService;
use crate::sync::follows::FollowSyncService;
use crate::sync::names::NameSyncService;

pub struct ServerRuntime {
    config: ServerConfig,
    backend: HttpBackend,
    follow_sync: FollowSyncService<JetStreamPublisher>,
    name_sync: NameSyncService<JetStreamPublisher>,
    balance_sync: BalanceSyncService<JetStreamPublisher>,
    activity_sync: ActivitySyncService<JetStreamPublisher>,
    checkpoint: ServerCheckpointState,
}

impl ServerRuntime {
    pub async fn connect(config: ServerConfig) -> Result<Self, ServerError> {
        let publisher = JetStreamPublisher::connect(&config.nats_url).await?;
        ensure_stream(&publisher, &config).await?;

        let backend = HttpBackend::new(&config.rpc_url);
        let follow_sync = FollowSyncService::new(publisher.clone(), config.signals_program);
        let name_sync = NameSyncService::new(publisher.clone(), config.registry_program);
        let balance_sync = BalanceSyncService::new(publisher.clone(), config.token_program);
        let activity_sync = ActivitySyncService::new(
            publisher.clone(),
            config.tracked_address,
            config.token_program,
            config.registry_program,
            config.signals_program,
        );

        let checkpoint = load_checkpoint(&config.checkpoint_path)?;

        Ok(Self {
            config,
            backend,
            follow_sync,
            name_sync,
            balance_sync,
            activity_sync,
            checkpoint,
        })
    }

    pub async fn run(mut self) -> Result<(), ServerError> {
        println!(
            "surf-events-server polling {} for {} and publishing to {}",
            self.config.rpc_url, self.config.tracked_address, self.config.nats_url
        );

        loop {
            self.poll_once().await?;
            tokio::time::sleep(std::time::Duration::from_millis(self.config.poll_interval_ms)).await;
        }
    }

    async fn poll_once(&mut self) -> Result<(), ServerError> {
        if !self.checkpoint.bootstrapped {
            self.name_sync.bootstrap(&self.backend).await.map_err(|err| ServerError::Sync(err.to_string()))?;
            let (_, balance) = self.balance_sync.sync_owner_if_changed(&self.backend, &self.config.tracked_address, &self.checkpoint.balance).await.map_err(|err| ServerError::Sync(err.to_string()))?;
            self.checkpoint.balance = balance;
            self.checkpoint.bootstrapped = true;
            save_checkpoint(&self.config.checkpoint_path, &self.checkpoint)?;
        }
        let (_, names) = self.name_sync.sync_recent_since(&self.backend, self.config.signature_batch_limit, &self.checkpoint.names).await.map_err(|err| ServerError::Sync(err.to_string()))?;
        self.checkpoint.names = names;
        let (_, balance) = self.balance_sync.sync_owner_if_changed(&self.backend, &self.config.tracked_address, &self.checkpoint.balance).await.map_err(|err| ServerError::Sync(err.to_string()))?;
        self.checkpoint.balance = balance;
        let (_, activity) = self.activity_sync.sync_recent_since(&self.backend, self.config.transaction_history_limit, &self.checkpoint.activity).await.map_err(|err| ServerError::Sync(err.to_string()))?;
        self.checkpoint.activity = activity;
        let signatures = self
            .backend
            .get_signatures_for_address(
                &self.config.tracked_address,
                Some(SignaturesForAddressOptions {
                    limit: Some(self.config.signature_batch_limit),
                    ..Default::default()
                }),
            )
            .await
            .map_err(|err| ServerError::Rpc(err.to_string()))?;

        let mut fresh = signatures
            .into_iter()
            .filter(|entry| self.checkpoint.follow.should_process(&entry.signature, entry.slot))
            .collect::<Vec<_>>();
        fresh.sort_by_key(|entry| entry.slot);

        let mut processed = Vec::new();

        for entry in fresh {
            self.follow_sync
                .sync_signature(&self.backend, &entry.signature)
                .await
                .map_err(|err| ServerError::Sync(err.to_string()))?;
            processed.push((entry.signature, entry.slot));
        }

        self.checkpoint.follow.advance(processed);
        save_checkpoint(&self.config.checkpoint_path, &self.checkpoint)?;

        Ok(())
    }
}

async fn ensure_stream(
    publisher: &JetStreamPublisher,
    config: &ServerConfig,
) -> Result<(), ServerError> {
    publisher
        .context()
        .get_or_create_stream(stream::Config {
            name: config.stream_name.clone(),
            subjects: vec!["surf.>".to_owned()],
            storage: stream::StorageType::Memory,
            ..Default::default()
        })
        .await
        .map_err(|err| ServerError::JetStream(err.to_string()))?;
    Ok(())
}
