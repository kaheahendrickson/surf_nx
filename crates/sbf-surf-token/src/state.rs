use pinocchio::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};

#[repr(C)]
pub struct Config {
    pub authority: Pubkey,
    pub total_supply: u64,
    pub decimals: u8,
    pub bump: u8,
    _padding: [u8; 6],
}

impl Config {
    pub const LEN: usize = 32 + 8 + 1 + 1 + 6;

    pub fn from_account_info(account: &AccountInfo) -> Result<&mut Self, ProgramError> {
        if account.data_len() != Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(unsafe { &mut *(account.borrow_mut_data_unchecked().as_mut_ptr() as *mut Self) })
    }
}

#[repr(C)]
pub struct Balance {
    pub owner: Pubkey,
    pub amount: u64,
    pub bump: u8,
    _padding: [u8; 7],
}

impl Balance {
    pub const LEN: usize = 32 + 8 + 1 + 7;

    pub fn from_account_info(account: &AccountInfo) -> Result<&mut Self, ProgramError> {
        if account.data_len() != Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(unsafe { &mut *(account.borrow_mut_data_unchecked().as_mut_ptr() as *mut Self) })
    }
}
