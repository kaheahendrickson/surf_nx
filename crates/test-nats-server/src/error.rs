use std::path::PathBuf;
use std::process::ExitStatus;
use std::time::Duration;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum NatsServerTestError {
    #[error("failed to create log directory: {0}")]
    TempDir(#[from] std::io::Error),

    #[error("failed to start /usr/local/bin/nats-server: {source}")]
    Spawn {
        #[source]
        source: std::io::Error,
    },

    #[error("nats-server exited before becoming ready with status {status}. Logs: {log_path}")]
    ExitedEarly {
        status: ExitStatus,
        log_path: PathBuf,
    },

    #[error(
        "nats-server did not become ready within {timeout:?}. Last connection error: {last_error}. Logs: {log_path}"
    )]
    ReadyTimeout {
        timeout: Duration,
        last_error: String,
        log_path: PathBuf,
    },

    #[error("failed to connect to nats server at {url}: {source}")]
    Connect {
        url: String,
        #[source]
        source: async_nats::ConnectError,
    },

    #[error("failed to create memory stream {name}: {message}")]
    CreateStream { name: String, message: String },
}
