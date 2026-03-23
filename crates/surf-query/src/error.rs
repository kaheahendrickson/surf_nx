use thiserror::Error;

#[derive(Debug, Error)]
pub enum QueryError {
    #[error(transparent)]
    Store(#[from] surf_store::StoreError),

    #[error("Invalid account data")]
    InvalidAccountData,

    #[error("Invalid name length: {0}")]
    InvalidNameLength(u8),

    #[error("Invalid UTF-8 in name")]
    InvalidUtf8,
}
