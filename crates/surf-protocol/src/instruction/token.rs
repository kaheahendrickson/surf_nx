pub const INITIALIZE_DISCRIMINATOR: u8 = 0;
pub const TRANSFER_DISCRIMINATOR: u8 = 1;
pub const BURN_DISCRIMINATOR: u8 = 2;
pub const MINT_DISCRIMINATOR: u8 = 3;

pub fn pack_initialize(total_supply: u64, decimals: u8) -> Vec<u8> {
    let mut data = Vec::with_capacity(10);
    data.push(INITIALIZE_DISCRIMINATOR);
    data.extend_from_slice(&total_supply.to_le_bytes());
    data.push(decimals);
    data
}

pub fn pack_transfer(amount: u64) -> Vec<u8> {
    let mut data = Vec::with_capacity(9);
    data.push(TRANSFER_DISCRIMINATOR);
    data.extend_from_slice(&amount.to_le_bytes());
    data
}

pub fn pack_burn(amount: u64) -> Vec<u8> {
    let mut data = Vec::with_capacity(9);
    data.push(BURN_DISCRIMINATOR);
    data.extend_from_slice(&amount.to_le_bytes());
    data
}

pub fn pack_mint(amount: u64) -> Vec<u8> {
    let mut data = Vec::with_capacity(9);
    data.push(MINT_DISCRIMINATOR);
    data.extend_from_slice(&amount.to_le_bytes());
    data
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn test_pack_initialize() {
        let data = pack_initialize(1_000_000, 9);
        assert_eq!(data[0], 0);
        assert_eq!(
            u64::from_le_bytes(data[1..9].try_into().unwrap()),
            1_000_000
        );
        assert_eq!(data[9], 9);
    }

    #[rstest]
    fn test_pack_transfer() {
        let data = pack_transfer(500_000);
        assert_eq!(data[0], 1);
        assert_eq!(u64::from_le_bytes(data[1..9].try_into().unwrap()), 500_000);
    }

    #[rstest]
    fn test_pack_burn() {
        let data = pack_burn(100_000);
        assert_eq!(data[0], 2);
        assert_eq!(u64::from_le_bytes(data[1..9].try_into().unwrap()), 100_000);
    }

    #[rstest]
    fn test_pack_mint() {
        let data = pack_mint(250_000);
        assert_eq!(data[0], 3);
        assert_eq!(u64::from_le_bytes(data[1..9].try_into().unwrap()), 250_000);
    }
}
