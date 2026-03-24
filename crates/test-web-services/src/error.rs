use std::path::PathBuf;
use std::process::ExitStatus;
use std::time::Duration;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum TestWebServicesError {
    #[error("failed to build SBF program {program}: {message}")]
    SbfBuild { program: String, message: String },

    #[error("failed to read program file {path}: {source}")]
    ProgramRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("SBF program file not found: {0}")]
    ProgramNotFound(PathBuf),

    #[error("failed to start {service}: {source}")]
    Spawn {
        service: String,
        #[source]
        source: std::io::Error,
    },

    #[error("{service} exited before ready with status {status}. Logs: {log_path}")]
    ExitedEarly {
        service: String,
        status: ExitStatus,
        log_path: PathBuf,
    },

    #[error(
        "{service} did not become ready within {timeout:?}. Last error: {last_error}. Logs: {log_path}"
    )]
    ReadyTimeout {
        service: String,
        timeout: Duration,
        last_error: String,
        log_path: PathBuf,
    },

    #[error("failed to connect to NATS at {url}: {source}")]
    NatsConnect {
        url: String,
        #[source]
        source: async_nats::ConnectError,
    },

    #[error("failed to connect to RPC at {url}: {message}")]
    RpcConnect { url: String, message: String },

    #[error("failed to create NATS stream {name}: {message}")]
    CreateStream { name: String, message: String },

    #[error("invalid program ID: {0}")]
    InvalidProgramId(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
