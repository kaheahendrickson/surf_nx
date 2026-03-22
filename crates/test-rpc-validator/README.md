# rpc-test-validator

A local Solana RPC test validator backed by Mollusk. This server exposes the MolluskBackend via HTTP JSON-RPC, enabling browser/WASM tests to run against an in-memory test validator.

## Features

- HTTP JSON-RPC 2.0 endpoint
- Core Solana RPC methods: `getAccountInfo`, `getBalance`, `getLatestBlockhash`, `getMinimumBalanceForRentExemption`, `sendTransaction`
- Test method: `requestAirdrop`
- Pre-load programs at startup via CLI

## Installation

```bash
cargo build -p rpc-test-validator
```

## Usage

Start the server:
```bash
cargo run -p rpc-test-validator -- --port 8899 --program "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA=/path/to/program.so"
```

### CLI Arguments

| Argument | Default | Description |
|----------|---------|-------------|
| `-p, --port <PORT>` | 8899 | Port to listen on |
| `--host <HOST>` | 127.0.0.1 | Host to bind to |
| `--program <PROGRAM>` | - | Program to load (format: PUBKEY=PATH). Can be specified multiple times. |

## Supported RPC Methods

### getAccountInfo
Returns account information for a given pubkey.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "getAccountInfo",
  "params": ["11111111111111111111111111111111"]
}
```

### getBalance
Returns the balance in lamports for a given pubkey.

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "getBalance",
  "params": ["11111111111111111111111111111111"]
}
```

### getLatestBlockhash
Returns the latest blockhash.

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "getLatestBlockhash",
  "params": []
}
```

### getMinimumBalanceForRentExemption
Returns the minimum balance required for rent exemption.

```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "getMinimumBalanceForRentExemption",
  "params": [100]
}
```

### sendTransaction
Submits a transaction (base64-encoded, bincode-serialized).

```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "method": "sendTransaction",
  "params": ["<base64-encoded-transaction>"]
}
```

### requestAirdrop
Airdrops lamports to a pubkey (test validator only).

```json
{
  "jsonrpc": "2.0",
  "id": 6,
  "method": "requestAirdrop",
  "params": ["11111111111111111111111111111111", 1000000000]
}
```

## Integration with surf-client-backend-native

Point `surf-client-backend-native` at the validator:

```rust
use surf_client_backend_native::HttpBackend;
use surf_client::backend::Backend;

let backend = HttpBackend::new("http://127.0.0.1:8899");
let balance = backend.get_balance(&pubkey).await?;
```

## Use Case

This validator enables browser/WASM tests to use `surf-client-backend-native` to connect to a local test validator, providing the same testing experience as native code with `MolluskBackend`.
