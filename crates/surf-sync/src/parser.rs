//! Transaction instruction parsing for sync operations.

use solana_pubkey::Pubkey;
use solana_sdk_ids::system_program;
use surf_client::ParsedTransaction;
use surf_events::ActivityKind;
use surf_protocol::SignalKind;

use crate::error::SyncError;

/// Register instruction discriminator.
pub const REGISTER_DISCRIMINATOR: u8 = 1;
pub const SIGNAL_DISCRIMINATOR: u8 = surf_protocol::instruction::signals::SIGNAL_DISCRIMINATOR;
pub const TOKEN_TRANSFER_DISCRIMINATOR: u8 =
    surf_protocol::instruction::token::TRANSFER_DISCRIMINATOR;
pub const TOKEN_MINT_DISCRIMINATOR: u8 = surf_protocol::instruction::token::MINT_DISCRIMINATOR;
pub const TOKEN_BURN_DISCRIMINATOR: u8 = surf_protocol::instruction::token::BURN_DISCRIMINATOR;
pub const SYSTEM_TRANSFER_DISCRIMINATOR: u32 = 2;

/// Parsed Register instruction data.
#[derive(Debug, Clone)]
pub struct ParsedRegister {
    /// The registered name (padded to 32 bytes).
    pub name: [u8; 32],
    /// The actual length of the name.
    pub name_len: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedSignal {
    pub kind: SignalKind,
    pub target: Pubkey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedTransfer {
    pub amount: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedSolTransfer {
    pub amount: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedMint {
    pub amount: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedBurn {
    pub amount: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedActivity {
    pub kind: ActivityKind,
    pub counterparty: Pubkey,
    pub amount: u64,
}

/// Checks if the instruction data is a Register instruction.
pub fn is_register_instruction(data: &[u8]) -> bool {
    !data.is_empty() && data[0] == REGISTER_DISCRIMINATOR
}

/// Parses a Register instruction from instruction data.
///
/// Returns `Ok(ParsedRegister)` if valid, `Err(SyncError::InvalidInstruction)` otherwise.
pub fn parse_register_instruction(data: &[u8]) -> Result<ParsedRegister, SyncError> {
    if !is_register_instruction(data) {
        return Err(SyncError::InvalidInstruction);
    }

    if data.len() < 34 {
        return Err(SyncError::InvalidInstruction);
    }

    let mut name = [0u8; 32];
    name.copy_from_slice(&data[1..33]);
    let name_len = data[33];

    if name_len > 32 {
        return Err(SyncError::InvalidInstruction);
    }

    Ok(ParsedRegister { name, name_len })
}

pub fn is_signal_instruction(data: &[u8]) -> bool {
    !data.is_empty() && data[0] == SIGNAL_DISCRIMINATOR
}

pub fn parse_signal_instruction(data: &[u8]) -> Result<ParsedSignal, SyncError> {
    if !is_signal_instruction(data) || data.len() < 34 {
        return Err(SyncError::InvalidInstruction);
    }

    let kind = match data[1] {
        0 => SignalKind::Follow,
        1 => SignalKind::Unfollow,
        _ => return Err(SyncError::InvalidInstruction),
    };
    let target = Pubkey::try_from(&data[2..34]).map_err(|_| SyncError::InvalidInstruction)?;

    Ok(ParsedSignal { kind, target })
}

pub fn is_token_transfer_instruction(data: &[u8]) -> bool {
    !data.is_empty() && data[0] == TOKEN_TRANSFER_DISCRIMINATOR
}

pub fn parse_token_transfer_instruction(data: &[u8]) -> Result<ParsedTransfer, SyncError> {
    if !is_token_transfer_instruction(data) || data.len() < 9 {
        return Err(SyncError::InvalidInstruction);
    }

    Ok(ParsedTransfer {
        amount: u64::from_le_bytes(
            data[1..9]
                .try_into()
                .map_err(|_| SyncError::InvalidInstruction)?,
        ),
    })
}

pub fn is_token_mint_instruction(data: &[u8]) -> bool {
    !data.is_empty() && data[0] == TOKEN_MINT_DISCRIMINATOR
}

pub fn parse_token_mint_instruction(data: &[u8]) -> Result<ParsedMint, SyncError> {
    if !is_token_mint_instruction(data) || data.len() < 9 {
        return Err(SyncError::InvalidInstruction);
    }

    Ok(ParsedMint {
        amount: u64::from_le_bytes(
            data[1..9]
                .try_into()
                .map_err(|_| SyncError::InvalidInstruction)?,
        ),
    })
}

pub fn is_token_burn_instruction(data: &[u8]) -> bool {
    !data.is_empty() && data[0] == TOKEN_BURN_DISCRIMINATOR
}

pub fn parse_token_burn_instruction(data: &[u8]) -> Result<ParsedBurn, SyncError> {
    if !is_token_burn_instruction(data) || data.len() < 9 {
        return Err(SyncError::InvalidInstruction);
    }

    Ok(ParsedBurn {
        amount: u64::from_le_bytes(
            data[1..9]
                .try_into()
                .map_err(|_| SyncError::InvalidInstruction)?,
        ),
    })
}

pub fn is_system_transfer_instruction(program_id: &Pubkey, data: &[u8]) -> bool {
    if *program_id != system_program::id() || data.len() < 12 {
        return false;
    }

    let Ok(discriminator) = <[u8; 4]>::try_from(&data[0..4]) else {
        return false;
    };

    u32::from_le_bytes(discriminator) == SYSTEM_TRANSFER_DISCRIMINATOR
}

pub fn parse_system_transfer_instruction(data: &[u8]) -> Result<ParsedSolTransfer, SyncError> {
    if data.len() < 12 {
        return Err(SyncError::InvalidInstruction);
    }

    Ok(ParsedSolTransfer {
        amount: u64::from_le_bytes(
            data[4..12]
                .try_into()
                .map_err(|_| SyncError::InvalidInstruction)?,
        ),
    })
}

pub fn extract_instruction_accounts(tx: &ParsedTransaction, account_indexes: &[u8]) -> Vec<Pubkey> {
    account_indexes
        .iter()
        .filter_map(|index| tx.message.account_keys.get(*index as usize).copied())
        .collect()
}

fn transaction_mentions_program(tx: &ParsedTransaction, program_id: &Pubkey) -> bool {
    tx.message.instructions.iter().any(|instruction| {
        tx.message
            .account_keys
            .get(instruction.program_id_index as usize)
            .copied()
            == Some(*program_id)
    })
}

pub fn parse_curated_activity(
    tx: &ParsedTransaction,
    instruction: &surf_client::InstructionInfo,
    tracked_owner: &Pubkey,
    token_program: &Pubkey,
    registry_program: &Pubkey,
    signals_program: &Pubkey,
) -> Result<Option<ParsedActivity>, SyncError> {
    let Some(program_id) = tx
        .message
        .account_keys
        .get(instruction.program_id_index as usize)
        .copied()
    else {
        return Ok(None);
    };

    let accounts = extract_instruction_accounts(tx, &instruction.accounts);

    if program_id == *registry_program && is_register_instruction(&instruction.data) {
        if accounts.first() == Some(tracked_owner) {
            return Ok(Some(ParsedActivity {
                kind: ActivityKind::NameRegistered,
                counterparty: *tracked_owner,
                amount: 0,
            }));
        }
        return Ok(None);
    }

    if program_id == *signals_program && is_signal_instruction(&instruction.data) {
        if accounts.first() != Some(tracked_owner) {
            return Ok(None);
        }

        let parsed = parse_signal_instruction(&instruction.data)?;
        return Ok(Some(ParsedActivity {
            kind: match parsed.kind {
                SignalKind::Follow => ActivityKind::Followed,
                SignalKind::Unfollow => ActivityKind::Unfollowed,
            },
            counterparty: parsed.target,
            amount: 0,
        }));
    }

    if program_id == *token_program && is_token_transfer_instruction(&instruction.data) {
        if accounts.len() < 3 {
            return Ok(None);
        }

        let parsed = parse_token_transfer_instruction(&instruction.data)?;
        let sender = accounts[0];
        let recipient = accounts[2];

        if sender == *tracked_owner {
            return Ok(Some(ParsedActivity {
                kind: ActivityKind::SurfSent,
                counterparty: recipient,
                amount: parsed.amount,
            }));
        }

        if recipient == *tracked_owner {
            return Ok(Some(ParsedActivity {
                kind: ActivityKind::SurfReceived,
                counterparty: sender,
                amount: parsed.amount,
            }));
        }

        return Ok(None);
    }

    if is_system_transfer_instruction(&program_id, &instruction.data) {
        if transaction_mentions_program(tx, token_program) {
            return Ok(None);
        }

        if accounts.len() < 2 {
            return Ok(None);
        }

        let parsed = parse_system_transfer_instruction(&instruction.data)?;
        let sender = accounts[0];
        let recipient = accounts[1];

        if sender == *tracked_owner {
            return Ok(Some(ParsedActivity {
                kind: ActivityKind::SolSent,
                counterparty: recipient,
                amount: parsed.amount,
            }));
        }

        if recipient == *tracked_owner {
            return Ok(Some(ParsedActivity {
                kind: ActivityKind::SolReceived,
                counterparty: sender,
                amount: parsed.amount,
            }));
        }
    }

    Ok(None)
}

/// Extracts the owner pubkey from account metadata.
///
/// In the sbf-surf-name-registry program, the owner is the second account (index 1).
pub fn extract_owner_from_accounts(accounts: &[Pubkey]) -> Option<Pubkey> {
    accounts.get(1).copied()
}

// TODO: Uncomment when test dependencies are available
// #[cfg(test)]
// mod tests {
//     use super::*;
//     use rstest::{fixture, rstest};
//
//     #[fixture]
//     fn register_data() -> Vec<u8> {
//         let mut name = [0u8; 32];
//         name[..5].copy_from_slice(b"alice");
//         let mut data = Vec::with_capacity(34);
//         data.push(REGISTER_DISCRIMINATOR);
//         data.extend_from_slice(&name);
//         data.push(5);
//         data
//     }
//
//     #[rstest]
//     fn test_is_register_instruction() {
//         assert!(is_register_instruction(&[1]));
//         assert!(is_register_instruction(&[1, 0, 0, 0]));
//         assert!(!is_register_instruction(&[0]));
//         assert!(!is_register_instruction(&[2]));
//         assert!(!is_register_instruction(&[]));
//     }
//
//     #[rstest]
//     fn test_parse_register_instruction_valid(register_data: Vec<u8>) {
//         let parsed = parse_register_instruction(&register_data).unwrap();
//         assert_eq!(parsed.name[..5], *b"alice");
//         assert_eq!(parsed.name_len, 5);
//     }
//
//     #[rstest]
//     fn test_parse_register_instruction_invalid_discriminator() {
//         let data = [0u8; 34];
//         assert!(matches!(
//             parse_register_instruction(&data),
//             Err(SyncError::InvalidInstruction)
//         ));
//     }
//
//     #[rstest]
//     fn test_parse_register_instruction_short_data() {
//         let data = [1u8; 10];
//         assert!(matches!(
//             parse_register_instruction(&data),
//             Err(SyncError::InvalidInstruction)
//         ));
//     }
//
//     #[rstest]
//     fn test_extract_owner_from_accounts() {
//         let owner = Pubkey::new_unique();
//         let accounts = [Pubkey::new_unique(), owner, Pubkey::new_unique()];
//         let extracted = extract_owner_from_accounts(&accounts).unwrap();
//         assert_eq!(extracted, accounts[1]);
//     }
//
//     #[rstest]
//     fn test_extract_owner_from_accounts_empty() {
//         let accounts: [Pubkey; 0] = [];
//         assert!(extract_owner_from_accounts(&accounts).is_none());
//     }
//
//     #[rstest]
//     fn test_parse_signal_instruction() {
//         let target = Pubkey::new_unique();
//         let mut data = vec![SIGNAL_DISCRIMINATOR, 0];
//         data.extend_from_slice(target.as_ref());
//         let parsed = parse_signal_instruction(&data).unwrap();
//         assert_eq!(parsed.kind, SignalKind::Follow);
//         assert_eq!(parsed.target, target);
//     }
//
//     #[rstest]
//     fn test_parse_token_transfer_instruction() {
//         let mut data = vec![TOKEN_TRANSFER_DISCRIMINATOR];
//         data.extend_from_slice(&42u64.to_le_bytes());
//         let parsed = parse_token_transfer_instruction(&data).unwrap();
//         assert_eq!(parsed.amount, 42);
//     }
// }
