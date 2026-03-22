use solana_account::Account;
use solana_hash::Hash;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use solana_transaction::Transaction;
use std::future::Future;

use crate::error::Error;

/// Account with its pubkey, returned from get_program_accounts.
#[derive(Debug, Clone)]
pub struct AccountInfo {
    pub pubkey: Pubkey,
    pub account: Account,
}

/// Transaction signature info for address monitoring.
#[derive(Debug, Clone)]
pub struct SignatureInfo {
    pub signature: Signature,
    pub slot: u64,
    pub block_time: Option<i64>,
}

/// Options for filtering get_program_accounts results.
#[derive(Debug, Clone, Default)]
pub struct ProgramAccountsFilter {
    pub data_size: Option<usize>,
}

/// Options for get_signatures_for_address.
#[derive(Debug, Clone, Default)]
pub struct SignaturesForAddressOptions {
    pub before: Option<Signature>,
    pub until: Option<u64>,
    pub limit: Option<usize>,
}

/// Parsed transaction with metadata.
#[derive(Debug, Clone)]
pub struct ParsedTransaction {
    pub slot: u64,
    pub block_time: Option<i64>,
    pub signatures: Vec<Signature>,
    pub message: TransactionMessage,
}

/// Transaction message with account keys and instructions.
#[derive(Debug, Clone)]
pub struct TransactionMessage {
    pub account_keys: Vec<Pubkey>,
    pub instructions: Vec<InstructionInfo>,
}

/// Instruction information extracted from a transaction.
#[derive(Debug, Clone)]
pub struct InstructionInfo {
    pub program_id_index: u8,
    pub accounts: Vec<u8>,
    pub data: Vec<u8>,
}

pub trait Backend: Send + Sync {
    fn get_account(
        &self,
        pubkey: &Pubkey,
    ) -> impl Future<Output = Result<Option<Account>, Error>> + Send;
    fn get_balance(
        &self,
        pubkey: &Pubkey,
    ) -> impl Future<Output = Result<Option<u64>, Error>> + Send;
    fn get_latest_blockhash(&self) -> impl Future<Output = Result<Hash, Error>> + Send;
    fn minimum_balance_for_rent_exemption(
        &self,
        size: usize,
    ) -> impl Future<Output = Result<u64, Error>> + Send;
    fn send_and_confirm(
        &self,
        tx: &Transaction,
    ) -> impl Future<Output = Result<Signature, Error>> + Send;

    /// Returns all accounts owned by a program with optional filters.
    fn get_program_accounts(
        &self,
        program_id: &Pubkey,
        filters: Option<ProgramAccountsFilter>,
    ) -> impl Future<Output = Result<Vec<AccountInfo>, Error>> + Send;

    /// Returns signatures for an address in reverse chronological order.
    fn get_signatures_for_address(
        &self,
        address: &Pubkey,
        options: Option<SignaturesForAddressOptions>,
    ) -> impl Future<Output = Result<Vec<SignatureInfo>, Error>> + Send;

    /// Returns transaction details for a signature.
    fn get_transaction(
        &self,
        signature: &Signature,
    ) -> impl Future<Output = Result<Option<ParsedTransaction>, Error>> + Send;
}

/// WASM-compatible backend trait without Send + Sync bounds.
/// Use this for browser and web worker implementations.
#[cfg(target_arch = "wasm32")]
pub trait WasmBackend {
    fn get_account(&self, pubkey: &Pubkey) -> impl Future<Output = Result<Option<Account>, Error>>;
    fn get_balance(&self, pubkey: &Pubkey) -> impl Future<Output = Result<Option<u64>, Error>>;
    fn get_latest_blockhash(&self) -> impl Future<Output = Result<Hash, Error>>;
    fn minimum_balance_for_rent_exemption(
        &self,
        size: usize,
    ) -> impl Future<Output = Result<u64, Error>>;
    fn send_and_confirm(&self, tx: &Transaction) -> impl Future<Output = Result<Signature, Error>>;

    fn get_program_accounts(
        &self,
        program_id: &Pubkey,
        filters: Option<ProgramAccountsFilter>,
    ) -> impl Future<Output = Result<Vec<AccountInfo>, Error>>;

    fn get_signatures_for_address(
        &self,
        address: &Pubkey,
        options: Option<SignaturesForAddressOptions>,
    ) -> impl Future<Output = Result<Vec<SignatureInfo>, Error>>;

    fn get_transaction(
        &self,
        signature: &Signature,
    ) -> impl Future<Output = Result<Option<ParsedTransaction>, Error>>;
}

pub trait TestBackend: Backend {
    fn airdrop(
        &self,
        pubkey: &Pubkey,
        lamports: u64,
    ) -> impl Future<Output = Result<(), Error>> + Send;
    fn add_program(
        &self,
        program_id: &Pubkey,
        bytes: &[u8],
    ) -> impl Future<Output = Result<(), Error>> + Send;
}
