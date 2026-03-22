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
mod state;

use instruction::{BurnParams, InitializeParams, MintParams, TokenInstruction, TransferParams};
use state::{Balance, Config};

const CONFIG_SEED: &[u8] = b"config";
const BALANCE_SEED: &[u8] = b"balance";

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let (ix, params_data) = TokenInstruction::unpack(instruction_data)?;

    match ix {
        TokenInstruction::Initialize => {
            let params = InitializeParams::unpack(params_data)?;
            initialize(program_id, accounts, params)
        }
        TokenInstruction::Transfer => {
            let params = TransferParams::unpack(params_data)?;
            transfer(program_id, accounts, params)
        }
        TokenInstruction::Burn => {
            let params = BurnParams::unpack(params_data)?;
            burn(program_id, accounts, params)
        }
        TokenInstruction::Mint => {
            let params = MintParams::unpack(params_data)?;
            mint(program_id, accounts, params)
        }
    }
}

fn initialize(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    params: InitializeParams,
) -> ProgramResult {
    let [authority, distribution, config_pda, _system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !authority.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let (config_key, config_bump) = find_program_address(&[CONFIG_SEED], program_id);
    if config_pda.key() != &config_key {
        return Err(ProgramError::InvalidSeeds);
    }

    let config_size = Config::LEN;
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

    let config = Config::from_account_info(config_pda)?;
    config.authority = *authority.key();
    config.total_supply = params.total_supply;
    config.decimals = params.decimals;
    config.bump = config_bump;

    let (balance_key, balance_bump) =
        find_program_address(&[BALANCE_SEED, authority.key().as_ref()], program_id);

    if distribution.key() != &balance_key {
        return Err(ProgramError::InvalidSeeds);
    }

    let balance_size = Balance::LEN;
    let balance_lamports = Rent::get()?.minimum_balance(balance_size);

    let balance_bump_seed = [balance_bump];
    let balance_seeds = seeds!(BALANCE_SEED, authority.key().as_ref(), &balance_bump_seed);
    let balance_signer = [Signer::from(&balance_seeds)];

    CreateAccount {
        from: authority,
        to: distribution,
        lamports: balance_lamports,
        space: balance_size as u64,
        owner: program_id,
    }
    .invoke_signed(&balance_signer)?;

    let balance = Balance::from_account_info(distribution)?;
    balance.owner = *authority.key();
    balance.amount = params.total_supply;
    balance.bump = balance_bump;

    Ok(())
}

fn transfer(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    params: TransferParams,
) -> ProgramResult {
    let [sender, sender_balance, recipient, recipient_balance, _system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !sender.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let (expected_sender_balance, _) =
        find_program_address(&[BALANCE_SEED, sender.key().as_ref()], program_id);
    if sender_balance.key() != &expected_sender_balance {
        return Err(ProgramError::InvalidSeeds);
    }

    let sender_bal = Balance::from_account_info(sender_balance)?;
    if sender_bal.owner != *sender.key() {
        return Err(ProgramError::InvalidAccountOwner);
    }

    if sender_bal.amount < params.amount {
        return Err(ProgramError::InsufficientFunds);
    }

    if sender.key() == recipient.key() {
        return Err(ProgramError::InvalidArgument);
    }

    sender_bal.amount -= params.amount;

    let (expected_recipient_balance, recipient_bump) =
        find_program_address(&[BALANCE_SEED, recipient.key().as_ref()], program_id);

    if recipient_balance.key() != &expected_recipient_balance {
        return Err(ProgramError::InvalidSeeds);
    }

    if recipient_balance.data_len() == 0 {
        let balance_size = Balance::LEN;
        let lamports = Rent::get()?.minimum_balance(balance_size);

        let recipient_bump_seed = [recipient_bump];
        let recipient_seeds = seeds!(BALANCE_SEED, recipient.key().as_ref(), &recipient_bump_seed);
        let recipient_signer = [Signer::from(&recipient_seeds)];

        CreateAccount {
            from: sender,
            to: recipient_balance,
            lamports,
            space: balance_size as u64,
            owner: program_id,
        }
        .invoke_signed(&recipient_signer)?;

        let recipient_bal = Balance::from_account_info(recipient_balance)?;
        recipient_bal.owner = *recipient.key();
        recipient_bal.amount = params.amount;
        recipient_bal.bump = recipient_bump;
    } else {
        let recipient_bal = Balance::from_account_info(recipient_balance)?;
        if recipient_bal.owner != *recipient.key() {
            return Err(ProgramError::InvalidAccountOwner);
        }
        recipient_bal.amount += params.amount;
    }

    Ok(())
}

fn burn(program_id: &Pubkey, accounts: &[AccountInfo], params: BurnParams) -> ProgramResult {
    let [holder, holder_balance, config, _system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !holder.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let (expected_balance, _) =
        find_program_address(&[BALANCE_SEED, holder.key().as_ref()], program_id);
    if holder_balance.key() != &expected_balance {
        return Err(ProgramError::InvalidSeeds);
    }

    let (expected_config, _) = find_program_address(&[CONFIG_SEED], program_id);
    if config.key() != &expected_config {
        return Err(ProgramError::InvalidSeeds);
    }

    let balance = Balance::from_account_info(holder_balance)?;
    if balance.owner != *holder.key() {
        return Err(ProgramError::InvalidAccountOwner);
    }

    if balance.amount < params.amount {
        return Err(ProgramError::InsufficientFunds);
    }

    balance.amount -= params.amount;

    let config_state = Config::from_account_info(config)?;
    config_state.total_supply -= params.amount;

    Ok(())
}

fn mint(program_id: &Pubkey, accounts: &[AccountInfo], params: MintParams) -> ProgramResult {
    let [authority, recipient, recipient_balance, config, _system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !authority.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let (expected_config, _) = find_program_address(&[CONFIG_SEED], program_id);
    if config.key() != &expected_config {
        return Err(ProgramError::InvalidSeeds);
    }

    let config_state = Config::from_account_info(config)?;
    if config_state.authority != *authority.key() {
        return Err(ProgramError::InvalidArgument);
    }

    let (expected_balance, balance_bump) =
        find_program_address(&[BALANCE_SEED, recipient.key().as_ref()], program_id);
    if recipient_balance.key() != &expected_balance {
        return Err(ProgramError::InvalidSeeds);
    }

    if recipient_balance.data_len() == 0 {
        let balance_size = Balance::LEN;
        let lamports = Rent::get()?.minimum_balance(balance_size);

        let balance_bump_seed = [balance_bump];
        let balance_seeds = seeds!(BALANCE_SEED, recipient.key().as_ref(), &balance_bump_seed);
        let balance_signer = [Signer::from(&balance_seeds)];

        CreateAccount {
            from: authority,
            to: recipient_balance,
            lamports,
            space: balance_size as u64,
            owner: program_id,
        }
        .invoke_signed(&balance_signer)?;

        let balance = Balance::from_account_info(recipient_balance)?;
        balance.owner = *recipient.key();
        balance.amount = params.amount;
        balance.bump = balance_bump;
    } else {
        let balance = Balance::from_account_info(recipient_balance)?;
        if balance.owner != *recipient.key() {
            return Err(ProgramError::InvalidAccountOwner);
        }
        balance.amount += params.amount;
    }

    config_state.total_supply += params.amount;

    Ok(())
}
