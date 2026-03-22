# surf-client

Type-state client SDK for the SURF token and name registry programs.

## Overview

This crate provides a type-safe, capability-based client for interacting with SURF programs:

- **Type-state roles** - Compile-time enforcement of permissions
- **Backend abstraction** - Works with test harnesses and production RPC
- **Signer abstraction** - Supports local keypairs and future wallet adapters
- **Domain namespacing** - Organized by token and names domains

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                          Surf<B>                                │
│  Root context holding backend and program IDs                   │
└─────────────────────┬───────────────────────────────────────────┘
                      │
        ┌─────────────┼─────────────┐
        │             │             │
        ▼             ▼             ▼
   authority()      user()       harness()
        │             │             │
        ▼             ▼             ▼
┌───────────┐  ┌───────────┐  ┌───────────┐
│ Authority │  │   User    │  │  Harness  │
│   Client  │  │  Client   │  │  Client   │
└───────────┘  └───────────┘  └───────────┘
```

## Roles and Capabilities

| Capability | Authority | User | Harness |
|------------|:---------:|:----:|:-------:|
| Initialize token | ✓ | | |
| Mint tokens | ✓ | | |
| Initialize registry | ✓ | | |
| Transfer tokens | | ✓ | |
| Burn tokens | | ✓ | |
| Register name | | ✓ | |
| Airdrop SOL | | | ✓ |
| Load programs | | | ✓ |
| Query accounts | ✓ | ✓ | ✓ |

## Usage

### Basic Setup

```rust
use surf_client::{Surf, LocalKeypairSigner};
use surf_provider_memory::MolluskBackend;
use solana_pubkey::Pubkey;

let backend = MolluskBackend::new();
let token_program = Pubkey::new_unique(); // Your deployed token program
let registry_program = Pubkey::new_unique(); // Your deployed registry program

let surf = Surf::new(backend, token_program, registry_program);
```

### Authority Operations

```rust
let authority = LocalKeypairSigner::generate();
let client = surf.authority(authority);

// Initialize token
client.token().initialize(1_000_000, 9).await?;

// Mint tokens to a recipient
let recipient = Pubkey::new_unique();
client.token().mint(&recipient, 100_000).await?;
```

### User Operations

```rust
let user = LocalKeypairSigner::generate();
let client = surf.user(user);

// Transfer tokens
let recipient = Pubkey::new_unique();
client.token().transfer(&recipient, 500).await?;

// Register a name
client.names().register("alice").await?;

// Look up a name
let record = client.names().lookup("bob").await?;
```

### Test Harness

```rust
let client = surf.harness();

// Airdrop SOL for testing
let account = Pubkey::new_unique();
client.airdrop(&account, 1_000_000_000).await?;

// Load a program
client.add_program(&program_id, &program_bytes).await?;
```

## Backends

### MolluskBackend (Testing)

In-process SVM for fast tests:

```rust
use surf_provider_memory::MolluskBackend;

let backend = MolluskBackend::new();
```

### HttpBackend (Production)

RPC client for production:

```rust
use surf_provider_http::HttpBackend;

let backend = HttpBackend::new("https://api.mainnet-beta.solana.com");
```

## Type Aliases

```rust
// Common configurations
type AuthorityClient<B, S> = SurfClient<B, AuthorityRole, S>;
type UserClient<B, S> = SurfClient<B, UserRole, S>;
type HarnessClient<B> = SurfClient<B, HarnessRole, NoSigner>;
```

## Error Handling

```rust
use surf_client::Error;

match client.token().transfer(&recipient, 100).await {
    Ok(()) => println!("Transfer successful"),
    Err(Error::InsufficientFunds) => println!("Not enough tokens"),
    Err(Error::Validation(msg)) => println!("Invalid input: {}", msg),
    Err(e) => println!("Error: {:?}", e),
}
```
