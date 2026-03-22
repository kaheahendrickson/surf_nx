use solana_pubkey::Pubkey;

pub const INITIALIZE_DISCRIMINATOR: u8 = 0;
pub const REGISTER_DISCRIMINATOR: u8 = 1;

pub fn pack_initialize(price: u64, token_program: &Pubkey) -> Vec<u8> {
    let mut data = Vec::with_capacity(41);
    data.push(INITIALIZE_DISCRIMINATOR);
    data.extend_from_slice(&price.to_le_bytes());
    data.extend_from_slice(token_program.as_ref());
    data
}

pub fn pack_register(name: &[u8; 32], name_len: u8) -> Vec<u8> {
    let mut data = Vec::with_capacity(34);
    data.push(REGISTER_DISCRIMINATOR);
    data.extend_from_slice(name);
    data.push(name_len);
    data
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};

    #[fixture]
    fn token_program() -> Pubkey {
        Pubkey::new_unique()
    }

    #[rstest]
    fn test_pack_initialize(token_program: Pubkey) {
        let data = pack_initialize(100_000, &token_program);
        assert_eq!(data[0], 0);
        assert_eq!(u64::from_le_bytes(data[1..9].try_into().unwrap()), 100_000);
        assert_eq!(&data[9..41], token_program.as_ref());
    }

    #[rstest]
    fn test_pack_register() {
        let mut name = [0u8; 32];
        name[..5].copy_from_slice(b"alice");
        let data = pack_register(&name, 5);
        assert_eq!(data[0], 1);
        assert_eq!(&data[1..6], b"alice");
        assert_eq!(data[33], 5);
    }
}
