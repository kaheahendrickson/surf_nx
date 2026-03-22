use pinocchio::program_error::ProgramError;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameRegistryInstruction {
    Initialize,
    Register,
}

#[repr(C)]
pub struct InitializeParams {
    pub price: u64,
    pub token_program: Pubkey,
}

#[repr(C)]
pub struct RegisterParams {
    pub name: [u8; 32],
    pub name_len: u8,
}

use pinocchio::pubkey::Pubkey;

impl NameRegistryInstruction {
    pub fn unpack(data: &[u8]) -> Result<(Self, &[u8]), ProgramError> {
        if data.is_empty() {
            return Err(ProgramError::InvalidInstructionData);
        }

        let discriminator = data[0];
        let ix = match discriminator {
            0 => NameRegistryInstruction::Initialize,
            1 => NameRegistryInstruction::Register,
            _ => return Err(ProgramError::InvalidInstructionData),
        };

        Ok((ix, &data[1..]))
    }
}

impl InitializeParams {
    pub fn unpack(data: &[u8]) -> Result<Self, ProgramError> {
        if data.len() < 40 {
            return Err(ProgramError::InvalidInstructionData);
        }
        let token_program: Pubkey = data[8..40].try_into().unwrap();
        Ok(Self {
            price: u64::from_le_bytes(data[0..8].try_into().unwrap()),
            token_program,
        })
    }
}

impl RegisterParams {
    pub fn unpack(data: &[u8]) -> Result<Self, ProgramError> {
        if data.len() < 33 {
            return Err(ProgramError::InvalidInstructionData);
        }
        let mut name = [0u8; 32];
        name.copy_from_slice(&data[0..32]);
        Ok(Self {
            name,
            name_len: data[32],
        })
    }
}
