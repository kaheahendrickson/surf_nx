use solana_pubkey::Pubkey;

pub const INITIALIZE_DISCRIMINATOR: u8 = 0;
pub const SIGNAL_DISCRIMINATOR: u8 = 1;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignalKind {
    Follow = 0,
    Unfollow = 1,
}

pub fn pack_initialize(token_program: &Pubkey, min_balance: u64) -> Vec<u8> {
    let mut data = Vec::with_capacity(41);
    data.push(INITIALIZE_DISCRIMINATOR);
    data.extend_from_slice(token_program.as_ref());
    data.extend_from_slice(&min_balance.to_le_bytes());
    data
}

pub fn pack_signal(kind: SignalKind, target: &Pubkey) -> Vec<u8> {
    let mut data = Vec::with_capacity(34);
    data.push(SIGNAL_DISCRIMINATOR);
    data.push(kind as u8);
    data.extend_from_slice(target.as_ref());
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

    #[fixture]
    fn target() -> Pubkey {
        Pubkey::new_unique()
    }

    #[rstest]
    fn test_pack_initialize(token_program: Pubkey) {
        let data = pack_initialize(&token_program, 1);
        assert_eq!(data[0], 0);
        assert_eq!(&data[1..33], token_program.as_ref());
        assert_eq!(u64::from_le_bytes(data[33..41].try_into().unwrap()), 1);
    }

    #[rstest]
    fn test_pack_signal(target: Pubkey) {
        let data = pack_signal(SignalKind::Follow, &target);
        assert_eq!(data[0], 1);
        assert_eq!(data[1], SignalKind::Follow as u8);
        assert_eq!(&data[2..34], target.as_ref());
    }
}
