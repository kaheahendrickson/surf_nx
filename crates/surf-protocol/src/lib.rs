pub mod instruction;
pub mod pda;
pub mod state;
pub mod validation;

pub use instruction::{
    pack_burn, pack_mint, pack_register, pack_registry_initialize, pack_signal,
    pack_signals_initialize, pack_token_initialize, pack_transfer, SignalKind,
};

pub use pda::{
    derive_name_record_pda, derive_registry_config_pda, derive_signals_config_pda,
    derive_token_balance_pda, derive_token_config_pda,
};

pub use state::{
    decode_name_record, decode_registry_config, decode_signals_config, decode_token_balance,
    decode_token_config, NameRecord, RegistryConfig, SignalsConfig, TokenBalance, TokenConfig,
};

pub use validation::{validate_name, ValidationError};

pub const CONFIG_SEED: &[u8] = b"config";
pub const BALANCE_SEED: &[u8] = b"balance";
pub const NAME_SEED: &[u8] = b"name";
pub const SIGNALS_CONFIG_SEED: &[u8] = b"config";

pub const MIN_NAME_LEN: usize = 3;
pub const MAX_NAME_LEN: usize = 32;
