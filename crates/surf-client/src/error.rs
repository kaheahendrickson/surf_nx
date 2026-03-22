use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Account not found: {0}")]
    AccountNotFound(solana_pubkey::Pubkey),

    #[error("Transaction failed: {0}")]
    TransactionFailed(String),

    #[error("Invalid account data")]
    InvalidAccountData,

    #[error("Signing failed: {0}")]
    SigningFailed(String),

    #[error("Backend error: {0}")]
    Backend(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Insufficient funds")]
    InsufficientFunds,

    #[error("Invalid signer")]
    InvalidSigner,
}
