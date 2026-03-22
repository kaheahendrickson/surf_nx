use solana_pubkey::Pubkey;

use crate::{BALANCE_SEED, CONFIG_SEED, NAME_SEED, SIGNALS_CONFIG_SEED};

pub fn derive_token_config_pda(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[CONFIG_SEED], program_id)
}

pub fn derive_token_balance_pda(owner: &Pubkey, program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[BALANCE_SEED, owner.as_ref()], program_id)
}

pub fn derive_registry_config_pda(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[CONFIG_SEED], program_id)
}

pub fn derive_name_record_pda(name: &[u8], program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[NAME_SEED, name], program_id)
}

pub fn derive_signals_config_pda(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SIGNALS_CONFIG_SEED], program_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};

    #[fixture]
    fn program_id() -> Pubkey {
        Pubkey::new_unique()
    }

    #[fixture]
    fn owner() -> Pubkey {
        Pubkey::new_unique()
    }

    #[rstest]
    fn test_derive_token_config_pda(program_id: Pubkey) {
        let (pda1, bump1) = derive_token_config_pda(&program_id);
        let (pda2, bump2) = derive_token_config_pda(&program_id);
        assert_eq!(pda1, pda2);
        assert_eq!(bump1, bump2);
        assert_ne!(pda1, program_id);
    }

    #[rstest]
    fn test_derive_token_balance_pda(program_id: Pubkey, owner: Pubkey) {
        let (pda1, bump1) = derive_token_balance_pda(&owner, &program_id);
        let (pda2, bump2) = derive_token_balance_pda(&owner, &program_id);
        assert_eq!(pda1, pda2);
        assert_eq!(bump1, bump2);
        assert_ne!(pda1, owner);
    }

    #[rstest]
    fn test_derive_registry_config_pda(program_id: Pubkey) {
        let (pda1, bump1) = derive_registry_config_pda(&program_id);
        let (pda2, bump2) = derive_registry_config_pda(&program_id);
        assert_eq!(pda1, pda2);
        assert_eq!(bump1, bump2);
    }

    #[rstest]
    fn test_derive_name_record_pda(program_id: Pubkey) {
        let name = b"alice";
        let (pda1, bump1) = derive_name_record_pda(name, &program_id);
        let (pda2, bump2) = derive_name_record_pda(name, &program_id);
        assert_eq!(pda1, pda2);
        assert_eq!(bump1, bump2);
    }

    #[rstest]
    fn test_derive_signals_config_pda(program_id: Pubkey) {
        let (pda1, bump1) = derive_signals_config_pda(&program_id);
        let (pda2, bump2) = derive_signals_config_pda(&program_id);
        assert_eq!(pda1, pda2);
        assert_eq!(bump1, bump2);
    }
}
