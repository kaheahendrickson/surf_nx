use pinocchio::program_error::ProgramError;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenInstruction {
    Initialize,
    Transfer,
    Burn,
    Mint,
}

#[repr(C)]
pub struct InitializeParams {
    pub total_supply: u64,
    pub decimals: u8,
}

#[repr(C)]
pub struct TransferParams {
    pub amount: u64,
}

#[repr(C)]
pub struct BurnParams {
    pub amount: u64,
}

#[repr(C)]
pub struct MintParams {
    pub amount: u64,
}

impl TokenInstruction {
    pub fn unpack(data: &[u8]) -> Result<(Self, &[u8]), ProgramError> {
        if data.is_empty() {
            return Err(ProgramError::InvalidInstructionData);
        }

        let discriminator = data[0];
        let ix = match discriminator {
            0 => TokenInstruction::Initialize,
            1 => TokenInstruction::Transfer,
            2 => TokenInstruction::Burn,
            3 => TokenInstruction::Mint,
            _ => return Err(ProgramError::InvalidInstructionData),
        };

        Ok((ix, &data[1..]))
    }
}

impl InitializeParams {
    pub fn unpack(data: &[u8]) -> Result<Self, ProgramError> {
        if data.len() < 9 {
            return Err(ProgramError::InvalidInstructionData);
        }
        Ok(Self {
            total_supply: u64::from_le_bytes(data[0..8].try_into().unwrap()),
            decimals: data[8],
        })
    }
}

impl TransferParams {
    pub fn unpack(data: &[u8]) -> Result<Self, ProgramError> {
        if data.len() < 8 {
            return Err(ProgramError::InvalidInstructionData);
        }
        Ok(Self {
            amount: u64::from_le_bytes(data[0..8].try_into().unwrap()),
        })
    }
}

impl BurnParams {
    pub fn unpack(data: &[u8]) -> Result<Self, ProgramError> {
        if data.len() < 8 {
            return Err(ProgramError::InvalidInstructionData);
        }
        Ok(Self {
            amount: u64::from_le_bytes(data[0..8].try_into().unwrap()),
        })
    }
}

impl MintParams {
    pub fn unpack(data: &[u8]) -> Result<Self, ProgramError> {
        if data.len() < 8 {
            return Err(ProgramError::InvalidInstructionData);
        }
        Ok(Self {
            amount: u64::from_le_bytes(data[0..8].try_into().unwrap()),
        })
    }
}
