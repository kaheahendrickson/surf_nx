use pinocchio::{program_error::ProgramError, pubkey::Pubkey};

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignalsInstruction {
    Initialize,
    Signal,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignalKind {
    Follow,
    Unfollow,
}

#[repr(C)]
pub struct InitializeParams {
    pub token_program: Pubkey,
    pub min_balance: u64,
}

#[repr(C)]
pub struct SignalParams {
    pub kind: SignalKind,
    pub target: Pubkey,
}

impl SignalsInstruction {
    pub fn unpack(data: &[u8]) -> Result<(Self, &[u8]), ProgramError> {
        if data.is_empty() {
            return Err(ProgramError::InvalidInstructionData);
        }

        let ix = match data[0] {
            0 => Self::Initialize,
            1 => Self::Signal,
            _ => return Err(ProgramError::InvalidInstructionData),
        };

        Ok((ix, &data[1..]))
    }
}

impl SignalKind {
    pub fn unpack(discriminator: u8) -> Result<Self, ProgramError> {
        match discriminator {
            0 => Ok(Self::Follow),
            1 => Ok(Self::Unfollow),
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}

impl InitializeParams {
    pub fn unpack(data: &[u8]) -> Result<Self, ProgramError> {
        if data.len() < 40 {
            return Err(ProgramError::InvalidInstructionData);
        }

        let token_program: Pubkey = data[0..32].try_into().unwrap();
        let min_balance = u64::from_le_bytes(data[32..40].try_into().unwrap());

        Ok(Self {
            token_program,
            min_balance,
        })
    }
}

impl SignalParams {
    pub fn unpack(data: &[u8]) -> Result<Self, ProgramError> {
        if data.len() < 33 {
            return Err(ProgramError::InvalidInstructionData);
        }

        let kind = SignalKind::unpack(data[0])?;
        let target: Pubkey = data[1..33].try_into().unwrap();

        Ok(Self { kind, target })
    }
}
