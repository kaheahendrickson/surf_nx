use solana_pubkey::Pubkey;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TokenConfig {
    pub authority: Pubkey,
    pub total_supply: u64,
    pub decimals: u8,
    pub bump: u8,
}

impl TokenConfig {
    pub const LEN: usize = 32 + 8 + 1 + 1 + 6;
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TokenBalance {
    pub owner: Pubkey,
    pub amount: u64,
    pub bump: u8,
}

impl TokenBalance {
    pub const LEN: usize = 32 + 8 + 1 + 7;
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegistryConfig {
    pub price: u64,
    pub token_program: Pubkey,
    pub bump: u8,
}

impl RegistryConfig {
    pub const LEN: usize = 8 + 32 + 1 + 7;
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NameRecord {
    pub owner: Pubkey,
    pub name: [u8; 32],
    pub len: u8,
}

impl NameRecord {
    pub const LEN: usize = 32 + 32 + 1 + 7;
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SignalsConfig {
    pub authority: Pubkey,
    pub token_program: Pubkey,
    pub min_balance: u64,
    pub bump: u8,
}

impl SignalsConfig {
    pub const LEN: usize = 32 + 32 + 8 + 1 + 7;
}

pub fn decode_token_config(data: &[u8]) -> Option<TokenConfig> {
    if data.len() < TokenConfig::LEN {
        return None;
    }
    let authority = Pubkey::try_from(&data[0..32]).ok()?;
    let total_supply = u64::from_le_bytes(data[32..40].try_into().ok()?);
    let decimals = data[40];
    let bump = data[41];
    Some(TokenConfig {
        authority,
        total_supply,
        decimals,
        bump,
    })
}

pub fn decode_token_balance(data: &[u8]) -> Option<TokenBalance> {
    if data.len() < TokenBalance::LEN {
        return None;
    }
    let owner = Pubkey::try_from(&data[0..32]).ok()?;
    let amount = u64::from_le_bytes(data[32..40].try_into().ok()?);
    let bump = data[40];
    Some(TokenBalance {
        owner,
        amount,
        bump,
    })
}

pub fn decode_registry_config(data: &[u8]) -> Option<RegistryConfig> {
    if data.len() < RegistryConfig::LEN {
        return None;
    }
    let price = u64::from_le_bytes(data[0..8].try_into().ok()?);
    let token_program = Pubkey::try_from(&data[8..40]).ok()?;
    let bump = data[40];
    Some(RegistryConfig {
        price,
        token_program,
        bump,
    })
}

pub fn decode_name_record(data: &[u8]) -> Option<NameRecord> {
    if data.len() < NameRecord::LEN {
        return None;
    }
    let owner = Pubkey::try_from(&data[0..32]).ok()?;
    let mut name = [0u8; 32];
    name.copy_from_slice(&data[32..64]);
    let len = data[64];
    Some(NameRecord { owner, name, len })
}

pub fn decode_signals_config(data: &[u8]) -> Option<SignalsConfig> {
    if data.len() < SignalsConfig::LEN {
        return None;
    }

    let authority = Pubkey::try_from(&data[0..32]).ok()?;
    let token_program = Pubkey::try_from(&data[32..64]).ok()?;
    let min_balance = u64::from_le_bytes(data[64..72].try_into().ok()?);
    let bump = data[72];

    Some(SignalsConfig {
        authority,
        token_program,
        min_balance,
        bump,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};

    #[fixture]
    fn token_config_data() -> Vec<u8> {
        let mut data = vec![0u8; TokenConfig::LEN];
        let authority = Pubkey::new_unique();
        data[0..32].copy_from_slice(authority.as_ref());
        data[32..40].copy_from_slice(&1_000_000u64.to_le_bytes());
        data[40] = 9;
        data[41] = 255;
        data
    }

    #[fixture]
    fn token_balance_data() -> Vec<u8> {
        let mut data = vec![0u8; TokenBalance::LEN];
        let owner = Pubkey::new_unique();
        data[0..32].copy_from_slice(owner.as_ref());
        data[32..40].copy_from_slice(&500_000u64.to_le_bytes());
        data[40] = 254;
        data
    }

    #[fixture]
    fn registry_config_data() -> Vec<u8> {
        let mut data = vec![0u8; RegistryConfig::LEN];
        data[0..8].copy_from_slice(&100_000u64.to_le_bytes());
        let token_program = Pubkey::new_unique();
        data[8..40].copy_from_slice(token_program.as_ref());
        data[40] = 253;
        data
    }

    #[fixture]
    fn name_record_data() -> Vec<u8> {
        let mut data = vec![0u8; NameRecord::LEN];
        let owner = Pubkey::new_unique();
        data[0..32].copy_from_slice(owner.as_ref());
        data[32..37].copy_from_slice(b"alice");
        data[64] = 5;
        data
    }

    #[fixture]
    fn signals_config_data() -> Vec<u8> {
        let mut data = vec![0u8; SignalsConfig::LEN];
        let authority = Pubkey::new_unique();
        let token_program = Pubkey::new_unique();
        data[0..32].copy_from_slice(authority.as_ref());
        data[32..64].copy_from_slice(token_program.as_ref());
        data[64..72].copy_from_slice(&1u64.to_le_bytes());
        data[72] = 252;
        data
    }

    #[rstest]
    fn test_decode_token_config(token_config_data: Vec<u8>) {
        let config = decode_token_config(&token_config_data).unwrap();
        assert_eq!(config.total_supply, 1_000_000);
        assert_eq!(config.decimals, 9);
        assert_eq!(config.bump, 255);
    }

    #[rstest]
    fn test_decode_token_balance(token_balance_data: Vec<u8>) {
        let balance = decode_token_balance(&token_balance_data).unwrap();
        assert_eq!(balance.amount, 500_000);
        assert_eq!(balance.bump, 254);
    }

    #[rstest]
    fn test_decode_registry_config(registry_config_data: Vec<u8>) {
        let config = decode_registry_config(&registry_config_data).unwrap();
        assert_eq!(config.price, 100_000);
        assert_eq!(config.bump, 253);
    }

    #[rstest]
    fn test_decode_name_record(name_record_data: Vec<u8>) {
        let record = decode_name_record(&name_record_data).unwrap();
        assert_eq!(&record.name[..5], b"alice");
        assert_eq!(record.len, 5);
    }

    #[rstest]
    fn test_decode_signals_config(signals_config_data: Vec<u8>) {
        let config = decode_signals_config(&signals_config_data).unwrap();
        assert_eq!(config.min_balance, 1);
        assert_eq!(config.bump, 252);
    }

    #[rstest]
    fn test_decode_insufficient_data() {
        assert!(decode_token_config(&[]).is_none());
        assert!(decode_token_balance(&[]).is_none());
        assert!(decode_registry_config(&[]).is_none());
        assert!(decode_name_record(&[]).is_none());
        assert!(decode_signals_config(&[]).is_none());
    }
}
