# surf-store

An isomorphic key-value store that works across native (filesystem) and browser (OPFS) environments.

## Features

- **Async trait** - All operations are async for non-blocking I/O
- **Column families** - Predefined namespaces for organizing data
- **Multiple backends**:
  - `MemoryStore` - In-memory storage (all platforms)
  - `NativeStore` - Filesystem storage (native only)
  - `OpfsStore` - OPFS storage (browser/WASM only)

## Usage

```rust
use surf_store::{KeyValueStore, MemoryStore, NAMES};

#[tokio::main]
async fn main() {
    let store = MemoryStore::new();
    
    // Store a value
    store.set(NAMES, b"alice", b"owner_pubkey").await.unwrap();
    
    // Retrieve a value
    let value = store.get(NAMES, b"alice").await.unwrap();
    assert_eq!(value, Some(b"owner_pubkey".to_vec()));
}
```

## Running Tests

```bash
# Run native tests for surf-store
cargo test -p surf-store

# Run browser OPFS integration tests
bash tests/playwright/run-target.sh surf-store
```

## Platform Support

| Platform | MemoryStore | NativeStore | OpfsStore |
|----------|-------------|-------------|-----------|
| Native    | ✓| ✓           | ✗           |
| WASM      | ✓| ✗           | ✓           |

## Column Families

| Constant | Name | Purpose |
|----------|------|---------|
| `NAMES` | `"names"` | Name records |
| `CHECKPOINTS` | `"checkpoints"` | Sync state |
| `BALANCES` | `"balances"` | Token balances |
| `PROPOSALS` | `"proposals"` | Governance |
| `METADATA` | `"metadata"` | Configuration |
