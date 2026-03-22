# SBF SURF Token Program

A native Solana token program built with Pinocchio for efficient SBF execution.

## Overview

SURF is a custom token program implementing basic fungible token functionality. Unlike SPL Token, SURF uses a simplified account model where each user has a single balance PDA derived from their wallet address.

## Instructions

### Initialize

Initializes the token configuration and creates the initial token distribution.

**Accounts:**
| Index | Name | Writable | Signer | Description |
|-------|------|----------|--------|-------------|
| 0 | authority | Yes | Yes | Token authority that can mint tokens |
| 1 | distribution | Yes | No | PDA for initial token distribution (receives total supply) |
| 2 | config_pda | Yes | No | Config PDA (`[b"config"]`) |
| 3 | system_program | No | No | System program |

**Data:** `[0u8] + total_supply (8 bytes LE) + decimals (1 byte)`

---

### Transfer

Transfers tokens from sender to recipient.

**Accounts:**
| Index | Name | Writable | Signer | Description |
|-------|------|----------|--------|-------------|
| 0 | sender | Yes | Yes | Token sender |
| 1 | sender_balance | Yes | No | Sender's balance PDA |
| 2 | recipient | No | No | Token recipient |
| 3 | recipient_balance | Yes | No | Recipient's balance PDA (created if doesn't exist) |
| 4 | system_program | No | No | System program |

**Data:** `[1u8] + amount (8 bytes LE)`

---

### Burn

Permanently removes tokens from circulation.

**Accounts:**
| Index | Name | Writable | Signer | Description |
|-------|------|----------|--------|-------------|
| 0 | holder | Yes | Yes | Token holder burning tokens |
| 1 | holder_balance | Yes | No | Holder's balance PDA |
| 2 | config | Yes | No | Config PDA |
| 3 | system_program | No | No | System program |

**Data:** `[2u8] + amount (8 bytes LE)`

---

### Mint

Creates new tokens (authority only).

**Accounts:**
| Index | Name | Writable | Signer | Description |
|-------|------|----------|--------|-------------|
| 0 | authority | Yes | Yes | Token authority |
| 1 | recipient | No | No | Recipient of minted tokens |
| 2 | recipient_balance | Yes | No | Recipient's balance PDA (created if doesn't exist) |
| 3 | config | Yes | No | Config PDA |
| 4 | system_program | No | No | System program |

**Data:** `[3u8] + amount (8 bytes LE)`

## State

### Config (48 bytes)

Stores global token configuration.

| Offset | Size | Field | Type | Description |
|--------|------|-------|------|-------------|
| 0 | 32 | authority | Pubkey | Token authority |
| 32 | 8 | total_supply | u64 | Current total supply |
| 40 | 1 | decimals | u8 | Token decimals |
| 41 | 1 | bump | u8 | PDA bump |
| 42 | 6 | _padding | [u8; 6] | Alignment padding |

**PDA Seeds:** `[b"config"]`

### Balance (48 bytes)

Stores a user's token balance.

| Offset | Size | Field | Type | Description |
|--------|------|-------|------|-------------|
| 0 | 32 | owner | Pubkey | Balance owner |
| 32 | 8 | amount | u64 | Token amount |
| 40 | 1 | bump | u8 | PDA bump |
| 41 | 7 | _padding | [u8; 7] | Alignment padding |

**PDA Seeds:** `[b"balance", owner_pubkey]`

## Building

```bash
cargo build-sbf --manifest-path sbf-surf-token/Cargo.toml
```

## Usage Example

```javascript
// Derive PDAs
const [configPda] = PublicKey.findProgramAddress([Buffer.from("config")], PROGRAM_ID);
const [balancePda] = PublicKey.findProgramAddress(
  [Buffer.from("balance"), wallet.publicKey.toBuffer()],
  PROGRAM_ID
);

// Transfer tokens
const instructionData = Buffer.alloc(9);
instructionData.writeUInt8(1, 0); // Transfer discriminator
instructionData.writeBigUInt64LE(amount, 1);
```
