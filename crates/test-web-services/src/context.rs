use tempfile::TempDir;

use crate::config::WebServicesConfig;
use crate::error::TestWebServicesError;
use crate::events::EventsServerGuard;
use crate::nats::NatsServerGuard;
use crate::validator::RpcValidatorGuard;

#[derive(Debug)]
pub struct TestWebServicesContext {
    _temp_dir: TempDir,
    nats: NatsServerGuard,
    validator: RpcValidatorGuard,
    events: EventsServerGuard,
    _config: WebServicesConfig,
}

impl TestWebServicesContext {
    pub async fn start() -> Result<Self, TestWebServicesError> {
        Self::start_with_config(WebServicesConfig::new()).await
    }

    pub async fn start_with_config(config: WebServicesConfig) -> Result<Self, TestWebServicesError> {
        let temp_dir = tempfile::tempdir()?;
        let checkpoint_path = temp_dir.path().join("checkpoint.json");

        let nats_port = if config.nats_port != 4222 {
            Some(config.nats_port)
        } else {
            None
        };
        let rpc_port = if config.rpc_port != 8899 {
            Some(config.rpc_port)
        } else {
            None
        };

        let nats = NatsServerGuard::start(nats_port).await?;
        let nats_url = nats.url().to_string();

        let validator = RpcValidatorGuard::start(rpc_port).await?;
        let rpc_url = validator.url().to_string();

        let events = EventsServerGuard::start(&nats_url, &rpc_url, &checkpoint_path).await?;

        Ok(Self {
            _temp_dir: temp_dir,
            nats,
            validator,
            events,
            _config: config,
        })
    }

    pub fn nats_url(&self) -> &str {
        self.nats.url()
    }

    pub fn rpc_url(&self) -> &str {
        self.validator.url()
    }

    pub fn nats_port(&self) -> u16 {
        self.nats.port()
    }

    pub fn rpc_port(&self) -> u16 {
        self.validator.port()
    }

    pub fn nats_log_path(&self) -> &std::path::Path {
        self.nats.log_path()
    }

    pub fn validator_log_path(&self) -> &std::path::Path {
        self.validator.log_path()
    }

    pub fn events_log_path(&self) -> &std::path::Path {
        self.events.log_path()
    }
}