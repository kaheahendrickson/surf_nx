use pinocchio::{
    account_info::AccountInfo,
    entrypoint,
    instruction::Signer,
    program_error::ProgramError,
    pubkey::{find_program_address, Pubkey},
    seeds,
    sysvars::{rent::Rent, Sysvar},
    ProgramResult,
};
use pinocchio_system::instructions::CreateAccount;

entrypoint!(process_instruction);

mod instruction;

use instruction::{InitializeParams, SignalParams, SignalsInstruction};

const CONFIG_SEED: &[u8] = b"config";
const BALANCE_SEED: &[u8] = b"balance";

#[repr(C)]
pub struct SignalsConfig {
    pub authority: Pubkey,
    pub token_program: Pubkey,
    pub min_balance: u64,
    pub bump: u8,
    _padding: [u8; 7],
}

impl SignalsConfig {
    pub const LEN: usize = 32 + 32 + 8 + 1 + 7;

    pub fn from_account_info(account: &AccountInfo) -> Result<&mut Self, ProgramError> {
        if account.data_len() != Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(unsafe { &mut *(account.borrow_mut_data_unchecked().as_mut_ptr() as *mut Self) })
    }
}

#[repr(C)]
pub struct TokenBalance {
    pub owner: Pubkey,
    pub amount: u64,
    pub bump: u8,
    _padding: [u8; 7],
}

impl TokenBalance {
    pub const LEN: usize = 32 + 8 + 1 + 7;

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
    let (ix, params_data) = SignalsInstruction::unpack(instruction_data)?;

    match ix {
        SignalsInstruction::Initialize => {
            let params = InitializeParams::unpack(params_data)?;
            initialize(program_id, accounts, params)
        }
        SignalsInstruction::Signal => {
            let params = SignalParams::unpack(params_data)?;
            signal(program_id, accounts, params)
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

    let lamports = Rent::get()?.minimum_balance(SignalsConfig::LEN);
    let config_bump_seed = [config_bump];
    let config_seeds = seeds!(CONFIG_SEED, &config_bump_seed);
    let config_signer = [Signer::from(&config_seeds)];

    CreateAccount {
        from: authority,
        to: config_pda,
        lamports,
        space: SignalsConfig::LEN as u64,
        owner: program_id,
    }
    .invoke_signed(&config_signer)?;

    let config = SignalsConfig::from_account_info(config_pda)?;
    config.authority = *authority.key();
    config.token_program = params.token_program;
    config.min_balance = params.min_balance;
    config.bump = config_bump;

    Ok(())
}

fn signal(program_id: &Pubkey, accounts: &[AccountInfo], params: SignalParams) -> ProgramResult {
    let [signer, token_balance, config_pda] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !signer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    if signer.key() == &params.target {
        return Err(ProgramError::InvalidArgument);
    }

    let (config_key, _) = find_program_address(&[CONFIG_SEED], program_id);
    if config_pda.key() != &config_key {
        return Err(ProgramError::InvalidSeeds);
    }

    let config = SignalsConfig::from_account_info(config_pda)?;
    let (expected_balance, _) = find_program_address(
        &[BALANCE_SEED, signer.key().as_ref()],
        &config.token_program,
    );
    if token_balance.key() != &expected_balance {
        return Err(ProgramError::InvalidSeeds);
    }

    let balance = TokenBalance::from_account_info(token_balance)?;
    if balance.owner != *signer.key() {
        return Err(ProgramError::InvalidAccountOwner);
    }

    if balance.amount < config.min_balance {
        return Err(ProgramError::InsufficientFunds);
    }

    Ok(())
}
