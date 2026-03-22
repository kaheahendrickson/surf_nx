# surf-protocol

Pure protocol logic for the SURF token and name registry programs.

## Overview

This crate provides:
- **Instruction encoding** - Pack instruction data for token and registry operations
- **PDA derivation** - Derive program addresses for all account types
- **Account decoding** - Decode on-chain account data into Rust types
- **Name validation** - Validate and normalize name registration strings

## Features

- No I/O dependencies - pure computation
- Works in both native and WASM environments
- Zero-cost abstractions

## Usage

### Instruction Encoding

```rust
use surf_protocol::{pack_token_initialize, pack_transfer};

// Pack token initialize instruction
let data = pack_token_initialize(1_000_000, 9);

// Pack transfer instruction
let data = pack_transfer(500_000);
```

### PDA Derivation

```rust
use surf_protocol::{derive_token_config_pda, derive_token_balance_pda};
use solana_pubkey::Pubkey;

let program_id = Pubkey::new_unique();

// Derive token config PDA
let (config_pda, bump) = derive_token_config_pda(&program_id);

// Derive token balance PDA for an owner
let owner = Pubkey::new_unique();
let (balance_pda, bump) = derive_token_balance_pda(&owner, &program_id);
```

### Account Decoding

```rust
use surf_protocol::{decode_token_config, decode_token_balance};

// Decode token config from account data
let config = decode_token_config(&account_data).expect("Invalid config");

// Decode token balance from account data
let balance = decode_token_balance(&account_data).expect("Invalid balance");
```

### Name Validation

```rust
use surf_protocol::{validate_name, ValidationError};

match validate_name("alice") {
    Ok(normalized) => println!("Valid name"),
    Err(ValidationError::TooShort) => println!("Name too short"),
    Err(ValidationError::TooLong) => println!("Name too long"),
    Err(ValidationError::InvalidCharacters) => println!("Only a-z allowed"),
}
```

## Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `CONFIG_SEED` | `b"config"` | Seed for config PDAs |
| `BALANCE_SEED` | `b"balance"` | Seed for balance PDAs |
| `NAME_SEED` | `b"name"` | Seed for name record PDAs |
| `MIN_NAME_LEN` | `3` | Minimum name length |
| `MAX_NAME_LEN` | `32` | Maximum name length |

## Account Sizes

| Account Type | Size |
|--------------|------|
| `TokenConfig` | 48 bytes |
| `TokenBalance` | 48 bytes |
| `RegistryConfig` | 48 bytes |
| `NameRecord` | 72 bytes |
