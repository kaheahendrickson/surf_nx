use pinocchio::{
    account_info::AccountInfo,
    entrypoint,
    instruction::{AccountMeta, Instruction, Signer},
    program_error::ProgramError,
    pubkey::{find_program_address, Pubkey},
    seeds,
    sysvars::{rent::Rent, Sysvar},
    ProgramResult,
};
use pinocchio_system::instructions::CreateAccount;

entrypoint!(process_instruction);

mod instruction;

use instruction::{InitializeParams, NameRegistryInstruction, RegisterParams};

const CONFIG_SEED: &[u8] = b"config";
const NAME_SEED: &[u8] = b"name";
const MIN_NAME_LEN: usize = 3;
const MAX_NAME_LEN: usize = 32;

#[repr(C)]
pub struct RegistryConfig {
    pub price: u64,
    pub token_program: Pubkey,
    pub bump: u8,
    _padding: [u8; 7],
}

impl RegistryConfig {
    pub const LEN: usize = 8 + 32 + 1 + 7;

    pub fn from_account_info(account: &AccountInfo) -> Result<&mut Self, ProgramError> {
        if account.data_len() != Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(unsafe { &mut *(account.borrow_mut_data_unchecked().as_mut_ptr() as *mut Self) })
    }
}

#[repr(C)]
pub struct NameRecord {
    pub owner: Pubkey,
    pub name: [u8; 32],
    pub len: u8,
    _padding: [u8; 7],
}

impl NameRecord {
    pub const LEN: usize = 32 + 32 + 1 + 7;

    pub fn from_account_info(account: &AccountInfo) -> Result<&mut Self, ProgramError> {
        if account.data_len() != Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(unsafe { &mut *(account.borrow_mut_data_unchecked().as_mut_ptr() as *mut Self) })
    }
}

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let (ix, params_data) = NameRegistryInstruction::unpack(instruction_data)?;

    match ix {
        NameRegistryInstruction::Initialize => {
            let params = InitializeParams::unpack(params_data)?;
            initialize(program_id, accounts, params)
        }
        NameRegistryInstruction::Register => {
            let params = RegisterParams::unpack(params_data)?;
            register(program_id, accounts, params)
        }
    }
}

fn initialize(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    params: InitializeParams,
) -> ProgramResult {
    let [authority, config_pda, _system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !authority.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let (config_key, config_bump) = find_program_address(&[CONFIG_SEED], program_id);
    if config_pda.key() != &config_key {
        return Err(ProgramError::InvalidSeeds);
    }

    if config_pda.data_len() != 0 {
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    let config_size = RegistryConfig::LEN;
    let lamports = Rent::get()?.minimum_balance(config_size);
    let config_bump_seed = [config_bump];
    let config_seeds = seeds!(CONFIG_SEED, &config_bump_seed);
    let config_signer = [Signer::from(&config_seeds)];

    CreateAccount {
        from: authority,
        to: config_pda,
        lamports,
        space: config_size as u64,
        owner: program_id,
    }
    .invoke_signed(&config_signer)?;

    let config = RegistryConfig::from_account_info(config_pda)?;
    config.price = params.price;
    config.token_program = params.token_program;
    config.bump = config_bump;

    Ok(())
}

fn validate_name(name: &[u8]) -> Result<[u8; 32], ProgramError> {
    let len = name.len();
    if len < MIN_NAME_LEN || len > MAX_NAME_LEN {
        return Err(ProgramError::InvalidInstructionData);
    }

    let mut normalized = [0u8; 32];
    for (i, &byte) in name.iter().enumerate() {
        let c = byte as char;
        if c.is_ascii_lowercase() {
            normalized[i] = byte;
        } else if c.is_ascii_uppercase() {
            normalized[i] = c.to_ascii_lowercase() as u8;
        } else {
            return Err(ProgramError::InvalidInstructionData);
        }
    }
    Ok(normalized)
}

fn register(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    params: RegisterParams,
) -> ProgramResult {
    let [payer, payer_balance, name_pda, registry_config, token_config, token_program, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !payer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let name_slice = &params.name[..params.name_len as usize];
    let normalized_name = validate_name(name_slice)?;
    let name_len = params.name_len as usize;

    let (name_key, name_bump) =
        find_program_address(&[NAME_SEED, &normalized_name[..name_len]], program_id);
    if name_pda.key() != &name_key {
        return Err(ProgramError::InvalidSeeds);
    }

    if name_pda.data_len() != 0 {
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    let (config_key, _) = find_program_address(&[CONFIG_SEED], program_id);
    if registry_config.key() != &config_key {
        return Err(ProgramError::InvalidSeeds);
    }

    let config = RegistryConfig::from_account_info(registry_config)?;
    if token_program.key() != &config.token_program {
        return Err(ProgramError::InvalidArgument);
    }

    let mut burn_data = vec![2u8];
    burn_data.extend_from_slice(&config.price.to_le_bytes());

    let (token_balance_key, _) =
        find_program_address(&[b"balance", payer.key().as_ref()], &config.token_program);
    let (token_config_key, _) = find_program_address(&[b"config"], &config.token_program);

    let burn_accounts: [AccountMeta; 4] = [
        AccountMeta::new(payer.key(), true, true),
        AccountMeta::new(&token_balance_key, true, false),
        AccountMeta::writable(&token_config_key),
        AccountMeta::readonly(system_program.key()),
    ];

    let burn_instruction = Instruction {
        program_id: &config.token_program,
        accounts: &burn_accounts,
        data: &burn_data,
    };

    pinocchio::cpi::invoke(
        &burn_instruction,
        &[payer, payer_balance, token_config, system_program],
    )?;

    let name_size = NameRecord::LEN;
    let lamports = Rent::get()?.minimum_balance(name_size);
    let name_bump_seed = [name_bump];
    let name_seeds = seeds!(NAME_SEED, &normalized_name[..name_len], &name_bump_seed);
    let name_signer = [Signer::from(&name_seeds)];

    CreateAccount {
        from: payer,
        to: name_pda,
        lamports,
        space: name_size as u64,
        owner: program_id,
    }
    .invoke_signed(&name_signer)?;

    let name_record = NameRecord::from_account_info(name_pda)?;
    name_record.owner = *payer.key();
    name_record.name = normalized_name;
    name_record.len = name_len as u8;

    Ok(())
}
