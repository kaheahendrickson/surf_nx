pub mod backend;
pub mod client;
pub mod error;
pub mod names;
pub mod query;
pub mod role;
pub mod rpc_types;
pub mod signals;
pub mod signer;
pub mod token;

#[cfg(target_arch = "wasm32")]
pub use backend::WasmBackend;
pub use backend::{
    AccountInfo, Backend, InstructionInfo, ParsedTransaction, ProgramAccountsFilter, SignatureInfo,
    SignaturesForAddressOptions, TestBackend, TransactionMessage,
};
pub use client::{Surf, SurfClient};
pub use error::Error;
pub use names::NamesClient;
pub use query::QueryClient;
pub use role::{AuthorityRole, HarnessRole, NoSigner, UserRole};
pub use rpc_types::{
    AccountInfoResult, BlockhashResult, JsonRpcError, JsonRpcRequest, JsonRpcResponse,
    ParsedInstructionResult, ParsedMessageResult, ParsedTransactionResult, ProgramAccountResult,
    PubkeyStrings, RpcContextResult, SignatureInfoResult, TransactionMetaInfo,
    METHOD_GET_ACCOUNT_INFO, METHOD_GET_BALANCE, METHOD_GET_LATEST_BLOCKHASH,
    METHOD_GET_MINIMUM_BALANCE_FOR_RENT_EXEMPTION, METHOD_GET_PROGRAM_ACCOUNTS,
    METHOD_GET_SIGNATURES_FOR_ADDRESS, METHOD_GET_TRANSACTION, METHOD_SEND_TRANSACTION,
};
pub use signals::SignalsClient;
pub use signer::LocalKeypairSigner;
pub use solana_signer::Signer;
pub use token::TokenClient;
