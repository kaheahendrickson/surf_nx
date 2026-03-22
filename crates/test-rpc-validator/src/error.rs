use thiserror::Error;

#[derive(Debug, Error)]
pub enum RpcError {
    #[error("Invalid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),

    #[error("Invalid pubkey: {0}")]
    InvalidPubkey(String),

    #[error("Invalid transaction: {0}")]
    InvalidTransaction(String),

    #[error("Backend error: {0}")]
    Backend(String),

    #[error("Missing required parameter: {0}")]
    MissingParam(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<surf_client::error::Error> for RpcError {
    fn from(err: surf_client::error::Error) -> Self {
        match err {
            surf_client::error::Error::Backend(msg) => RpcError::Backend(msg),
            _ => RpcError::Backend(err.to_string()),
        }
    }
}
