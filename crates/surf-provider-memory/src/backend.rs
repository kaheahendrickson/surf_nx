use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::sync::OnceLock;

use mollusk_svm::program::{create_program_account_loader_v3, keyed_account_for_system_program};
use mollusk_svm::Mollusk;
use solana_account::Account;
use solana_hash::Hash;
use solana_instruction::{AccountMeta, Instruction};
use solana_message::compiled_instruction::CompiledInstruction;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use solana_transaction::Transaction;
use tokio::sync::Mutex;

use surf_client::backend::{
    AccountInfo, Backend, ParsedTransaction, ProgramAccountsFilter, SignatureInfo,
    SignaturesForAddressOptions, TestBackend,
};
use surf_client::error::Error;

const DEFAULT_LOADER: Pubkey = solana_sdk_ids::bpf_loader_upgradeable::id();
const SYSTEM_PROGRAM: Pubkey = solana_sdk_ids::system_program::id();
const SYSTEM_TRANSFER_TAG: u32 = 2;
static MOLLUSK_LOG_INIT: OnceLock<()> = OnceLock::new();

fn suppress_mollusk_logs_by_default() {
    MOLLUSK_LOG_INIT.get_or_init(|| {
        if env::var_os("RUST_LOG").is_none() {
            unsafe {
                env::set_var("RUST_LOG", "error");
            }
        }
    });
}

#[derive(Clone)]
struct StoredTransaction {
    slot: u64,
    transaction: Transaction,
}

struct MolluskState {
    accounts: HashMap<Pubkey, Account>,
    programs: HashMap<Pubkey, Vec<u8>>,
    transactions: Vec<StoredTransaction>,
    next_slot: u64,
}

#[derive(Clone)]
pub struct MolluskBackend {
    state: Arc<Mutex<MolluskState>>,
}

impl MolluskBackend {
    pub fn new() -> Self {
        suppress_mollusk_logs_by_default();

        Self {
            state: Arc::new(Mutex::new(MolluskState {
                accounts: HashMap::new(),
                programs: HashMap::new(),
                transactions: Vec::new(),
                next_slot: 1,
            })),
        }
    }
}

impl Default for MolluskBackend {
    fn default() -> Self {
        Self::new()
    }
}

fn decompile_instruction(
    compiled: &CompiledInstruction,
    account_keys: &[Pubkey],
    header: &solana_message::MessageHeader,
) -> Instruction {
    let program_id = account_keys[compiled.program_id_index as usize];

    let num_signers = header.num_required_signatures as usize;
    let num_readonly_signed = header.num_readonly_signed_accounts as usize;
    let num_readonly_unsigned = header.num_readonly_unsigned_accounts as usize;
    let num_accounts = account_keys.len();

    let accounts: Vec<AccountMeta> = compiled
        .accounts
        .iter()
        .map(|&index| {
            let index = index as usize;
            let pubkey = account_keys[index];

            let is_signer = index < num_signers;

            let is_writable = if is_signer {
                index < num_signers - num_readonly_signed
            } else {
                index < num_accounts - num_readonly_unsigned
            };

            AccountMeta {
                pubkey,
                is_signer,
                is_writable,
            }
        })
        .collect();

    Instruction {
        program_id,
        accounts,
        data: compiled.data.clone(),
    }
}

fn decode_system_transfer_lamports(data: &[u8]) -> Option<u64> {
    if data.len() < 12 {
        return None;
    }

    let tag = u32::from_le_bytes(data[0..4].try_into().ok()?);
    if tag != SYSTEM_TRANSFER_TAG {
        return None;
    }

    Some(u64::from_le_bytes(data[4..12].try_into().ok()?))
}

fn process_system_transfer(
    instruction: &Instruction,
    accounts: &mut HashMap<Pubkey, Account>,
) -> Result<(), Error> {
    if instruction.accounts.len() < 2 {
        return Err(Error::Backend(
            "System transfer requires source and destination".to_string(),
        ));
    }

    let lamports = decode_system_transfer_lamports(&instruction.data).ok_or_else(|| {
        Error::Backend("Unsupported system instruction for Mollusk backend".to_string())
    })?;

    let from = instruction.accounts[0].pubkey;
    let to = instruction.accounts[1].pubkey;

    let sender = accounts
        .get_mut(&from)
        .ok_or_else(|| Error::Backend("Source account not found".to_string()))?;

    if sender.lamports < lamports {
        return Err(Error::Backend("Insufficient funds".to_string()));
    }

    sender.lamports -= lamports;

    let recipient = accounts.entry(to).or_insert_with(|| Account {
        owner: SYSTEM_PROGRAM,
        ..Account::default()
    });
    recipient.lamports += lamports;

    Ok(())
}

fn touches_address(tx: &Transaction, address: &Pubkey) -> bool {
    tx.message.account_keys.iter().any(|key| key == address)
}

impl Backend for MolluskBackend {
    async fn get_account(&self, pubkey: &Pubkey) -> Result<Option<Account>, Error> {
        let state = self.state.lock().await;
        Ok(state.accounts.get(pubkey).cloned())
    }

    async fn get_balance(&self, pubkey: &Pubkey) -> Result<Option<u64>, Error> {
        let state = self.state.lock().await;
        Ok(state.accounts.get(pubkey).map(|a| a.lamports))
    }

    async fn get_latest_blockhash(&self) -> Result<Hash, Error> {
        let state = self.state.lock().await;
        let seed = state.next_slot.to_le_bytes();
        let mut bytes = [0u8; 32];
        bytes[..8].copy_from_slice(&seed);
        Ok(Hash::new_from_array(bytes))
    }

    async fn minimum_balance_for_rent_exemption(&self, size: usize) -> Result<u64, Error> {
        const LAMPORTS_PER_BYTE_YEAR: u64 = 3480;
        const EXEMPTION_THRESHOLD: f64 = 2.0;
        const YEARS: f64 = 1.0;

        let rent = (size as f64 * LAMPORTS_PER_BYTE_YEAR as f64 * YEARS * EXEMPTION_THRESHOLD)
            .ceil() as u64;
        Ok(rent.max(890880))
    }

    async fn send_and_confirm(&self, tx: &Transaction) -> Result<Signature, Error> {
        let state = self.state.lock().await;
        let mut accounts = state.accounts.clone();
        let programs = state.programs.clone();
        let slot = state.next_slot;
        drop(state);

        let instructions: Vec<Instruction> = tx
            .message
            .instructions
            .iter()
            .map(|ci| decompile_instruction(ci, &tx.message.account_keys, &tx.message.header))
            .collect();

        if instructions.is_empty() {
            return Err(Error::Backend("No instructions in transaction".to_string()));
        }

        let system_only = instructions
            .iter()
            .all(|instruction| instruction.program_id == SYSTEM_PROGRAM);

        if system_only {
            for instruction in &instructions {
                process_system_transfer(instruction, &mut accounts)?;
            }
        } else {
            let mut account_list: Vec<(Pubkey, Account)> = accounts.into_iter().collect();

            for account_key in &tx.message.account_keys {
                if account_list.iter().any(|(pubkey, _)| pubkey == account_key) {
                    continue;
                }

                let account = if *account_key == SYSTEM_PROGRAM {
                    keyed_account_for_system_program().1
                } else {
                    Account::default()
                };

                account_list.push((*account_key, account));
            }

            let result = tokio::task::spawn_blocking(move || {
                let mut mollusk = Mollusk::default();

                for (program_id, elf_bytes) in &programs {
                    mollusk.add_program_with_loader_and_elf(program_id, &DEFAULT_LOADER, elf_bytes);
                }

                if instructions.len() == 1 {
                    mollusk.process_instruction(&instructions[0], &account_list)
                } else {
                    mollusk.process_instruction_chain(&instructions, &account_list)
                }
            })
            .await
            .map_err(|e| Error::Backend(format!("Spawn blocking error: {}", e)))?;

            if result.program_result.is_err() {
                return Err(Error::Backend(format!(
                    "Transaction failed: {:?}",
                    result.program_result
                )));
            }

            accounts = result.resulting_accounts.into_iter().collect();
        }

        let mut state = self.state.lock().await;
        state.accounts = accounts;
        state.transactions.push(StoredTransaction {
            slot,
            transaction: tx.clone(),
        });
        state.next_slot += 1;

        Ok(tx.signatures.first().copied().unwrap_or_default())
    }

    async fn get_program_accounts(
        &self,
        program_id: &Pubkey,
        filters: Option<ProgramAccountsFilter>,
    ) -> Result<Vec<AccountInfo>, Error> {
        let state = self.state.lock().await;

        let data_size = filters.and_then(|filter| filter.data_size);

        let accounts = state
            .accounts
            .iter()
            .filter(|(_, account)| account.owner == *program_id)
            .filter(|(_, account)| match data_size {
                Some(size) => account.data.len() == size,
                None => true,
            })
            .map(|(pubkey, account)| AccountInfo {
                pubkey: *pubkey,
                account: account.clone(),
            })
            .collect();

        Ok(accounts)
    }

    async fn get_signatures_for_address(
        &self,
        address: &Pubkey,
        options: Option<SignaturesForAddressOptions>,
    ) -> Result<Vec<SignatureInfo>, Error> {
        let state = self.state.lock().await;

        let mut signatures: Vec<SignatureInfo> = state
            .transactions
            .iter()
            .rev()
            .filter(|stored| touches_address(&stored.transaction, address))
            .filter_map(|stored| {
                stored
                    .transaction
                    .signatures
                    .first()
                    .copied()
                    .map(|signature| SignatureInfo {
                        signature,
                        slot: stored.slot,
                        block_time: None,
                    })
            })
            .collect();

        if let Some(limit) = options.and_then(|opts| opts.limit) {
            signatures.truncate(limit);
        }

        Ok(signatures)
    }

    async fn get_transaction(
        &self,
        signature: &Signature,
    ) -> Result<Option<ParsedTransaction>, Error> {
        let state = self.state.lock().await;

        let Some(stored) = state.transactions.iter().find(|stored| {
            stored
                .transaction
                .signatures
                .first()
                .is_some_and(|candidate| candidate == signature)
        }) else {
            return Ok(None);
        };

        let message = &stored.transaction.message;
        let instructions = message
            .instructions
            .iter()
            .map(|instruction| surf_client::backend::InstructionInfo {
                program_id_index: instruction.program_id_index,
                accounts: instruction.accounts.clone(),
                data: instruction.data.clone(),
            })
            .collect();

        Ok(Some(ParsedTransaction {
            slot: stored.slot,
            block_time: None,
            signatures: stored.transaction.signatures.clone(),
            message: surf_client::backend::TransactionMessage {
                account_keys: message.account_keys.clone(),
                instructions,
            },
        }))
    }
}

impl TestBackend for MolluskBackend {
    async fn airdrop(&self, pubkey: &Pubkey, lamports: u64) -> Result<(), Error> {
        let mut state = self.state.lock().await;
        let account = state.accounts.entry(*pubkey).or_insert_with(|| Account {
            owner: SYSTEM_PROGRAM,
            ..Account::default()
        });
        account.lamports += lamports;
        Ok(())
    }

    async fn add_program(&self, program_id: &Pubkey, bytes: &[u8]) -> Result<(), Error> {
        let mut state = self.state.lock().await;
        state.programs.insert(*program_id, bytes.to_vec());

        let program_account = create_program_account_loader_v3(program_id);
        state.accounts.insert(*program_id, program_account);

        Ok(())
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
