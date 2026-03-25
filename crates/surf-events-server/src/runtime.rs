use async_nats::jetstream::stream;
use surf_client_backend_http::HttpBackend;

use crate::checkpoint::{load_checkpoint, save_checkpoint, ServerCheckpointState};
use crate::config::ServerConfig;
use crate::error::ServerError;
use crate::publisher::JetStreamPublisher;
use crate::sync::follows::FollowSyncService;
use crate::sync::names::NameSyncService;
use crate::sync::tokens::TokenSyncService;

pub struct ServerRuntime {
    config: ServerConfig,
    backend: HttpBackend,
    name_sync: NameSyncService<JetStreamPublisher>,
    token_sync: TokenSyncService<JetStreamPublisher>,
    signals_sync: FollowSyncService<JetStreamPublisher>,
    checkpoint: ServerCheckpointState,
}

impl ServerRuntime {
    pub async fn connect(config: ServerConfig) -> Result<Self, ServerError> {
        let publisher = JetStreamPublisher::connect(&config.nats_url).await?;
        ensure_stream(&publisher, &config).await?;

        let backend = HttpBackend::new(&config.rpc_url);
        let name_sync = NameSyncService::new(publisher.clone(), config.registry_program);
        let token_sync = TokenSyncService::new(publisher.clone(), config.token_program);
        let signals_sync = FollowSyncService::new(publisher.clone(), config.signals_program);

        let checkpoint = load_checkpoint(&config.checkpoint_path)?;

        Ok(Self {
            config,
            backend,
            name_sync,
            token_sync,
            signals_sync,
            checkpoint,
        })
    }

    pub async fn run(mut self) -> Result<(), ServerError> {
        println!(
            "surf-events-server polling {} for token program {}, signals program {} and publishing to {}",
            self.config.rpc_url, self.config.token_program, self.config.signals_program, self.config.nats_url
        );

        loop {
            self.poll_once().await?;
            tokio::time::sleep(std::time::Duration::from_millis(self.config.poll_interval_ms)).await;
        }
    }

    async fn poll_once(&mut self) -> Result<(), ServerError> {
        if !self.checkpoint.bootstrapped {
            self.name_sync.bootstrap(&self.backend).await.map_err(|err| ServerError::Sync(err.to_string()))?;
            self.checkpoint.bootstrapped = true;
            save_checkpoint(&self.config.checkpoint_path, &self.checkpoint)?;
        }
        let (_, names) = self.name_sync.sync_recent_since(&self.backend, self.config.signature_batch_limit, &self.checkpoint.names).await.map_err(|err| ServerError::Sync(err.to_string()))?;
        self.checkpoint.names = names;
        let (_, tokens) = self.token_sync.sync_recent_since(&self.backend, self.config.signature_batch_limit, &self.checkpoint.tokens).await.map_err(|err| ServerError::Sync(err.to_string()))?;
        self.checkpoint.tokens = tokens;
        let (_, signals) = self.signals_sync.sync_recent_since(&self.backend, self.config.signature_batch_limit, &self.checkpoint.signals).await.map_err(|err| ServerError::Sync(err.to_string()))?;
        self.checkpoint.signals = signals;
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
