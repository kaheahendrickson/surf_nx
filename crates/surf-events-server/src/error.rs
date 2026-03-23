use thiserror::Error;

#[derive(Debug, Error)]
pub enum EventPublishError {
    #[error("failed to serialize event: {0}")]
    Serialize(#[from] serde_json::Error),

    #[error("failed to publish event: {0}")]
    Publish(String),
}

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("backend request failed: {0}")]
    Backend(#[from] surf_client::Error),

    #[error("invalid signal instruction")]
    InvalidSignalInstruction,

    #[error("failed to publish event: {0}")]
    Publish(#[from] EventPublishError),
}

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("nats connection failed: {0}")]
    NatsConnect(String),

    #[error("jetstream setup failed: {0}")]
    JetStream(String),

    #[error("rpc request failed: {0}")]
    Rpc(String),

    #[error("sync loop failed: {0}")]
    Sync(String),

    #[error("checkpoint error: {0}")]
    Checkpoint(String),
}
