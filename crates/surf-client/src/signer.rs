use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use solana_signer::Signer;

use crate::error::Error;

pub struct LocalKeypairSigner {
    keypair: Keypair,
}

impl LocalKeypairSigner {
    pub fn new(keypair: Keypair) -> Self {
        Self { keypair }
    }

    pub fn generate() -> Self {
        Self {
            keypair: Keypair::new(),
        }
    }
}

impl Signer for LocalKeypairSigner {
    fn pubkey(&self) -> Pubkey {
        self.keypair.pubkey()
    }

    fn try_pubkey(&self) -> Result<Pubkey, solana_signer::SignerError> {
        self.keypair.try_pubkey()
    }

    fn sign_message(&self, message: &[u8]) -> Signature {
        self.keypair.sign_message(message)
    }

    fn try_sign_message(&self, message: &[u8]) -> Result<Signature, solana_signer::SignerError> {
        self.keypair.try_sign_message(message)
    }

    fn is_interactive(&self) -> bool {
        self.keypair.is_interactive()
    }
}

impl LocalKeypairSigner {
    pub fn sign(&self, message: &[u8]) -> Result<Signature, Error> {
        self.try_sign_message(message)
            .map_err(|e| Error::SigningFailed(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};

    #[fixture]
    fn signer() -> LocalKeypairSigner {
        LocalKeypairSigner::generate()
    }

    #[rstest]
    fn test_local_keypair_signer_pubkey(signer: LocalKeypairSigner) {
        let pubkey = signer.pubkey();
        assert_ne!(pubkey, Pubkey::default());
    }

    #[rstest]
    fn test_local_keypair_signer_sign(signer: LocalKeypairSigner) {
        let message = b"test message";
        let signature = signer.sign(message).unwrap();
        assert!(!signature.as_ref().iter().all(|&b| b == 0));
    }

    #[rstest]
    fn test_local_keypair_signer_consistent_pubkey(signer: LocalKeypairSigner) {
        let pubkey1 = signer.pubkey();
        let pubkey2 = signer.pubkey();
        assert_eq!(pubkey1, pubkey2);
    }
}
