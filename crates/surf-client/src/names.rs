use solana_instruction::{AccountMeta, Instruction};
use solana_message::Message;
use solana_pubkey::Pubkey;
use solana_transaction::Transaction;

#[cfg(not(target_arch = "wasm32"))]
use crate::backend::Backend;
use crate::error::Error;
use crate::role::{CanInitRegistry, CanRegisterName, Sealed};
use solana_signer::Signer;

#[allow(dead_code)]
pub struct NamesClient<'a, B, R, S> {
    backend: &'a B,
    token_program: &'a Pubkey,
    registry_program: &'a Pubkey,
    signer: &'a S,
    _role: std::marker::PhantomData<R>,
}

#[cfg(not(target_arch = "wasm32"))]
impl<'a, B, R, S> NamesClient<'a, B, R, S>
where
    B: Backend,
    R: Sealed,
    S: Signer,
{
    pub fn new(
        backend: &'a B,
        token_program: &'a Pubkey,
        registry_program: &'a Pubkey,
        signer: &'a S,
    ) -> Self {
        Self {
            backend,
            token_program,
            registry_program,
            signer,
            _role: std::marker::PhantomData,
        }
    }

    pub fn program_id(&self) -> &Pubkey {
        self.registry_program
    }
}

#[cfg(target_arch = "wasm32")]
impl<'a, B, R, S> NamesClient<'a, B, R, S>
where
    B: crate::backend::WasmBackend,
    R: Sealed,
    S: Signer,
{
    pub fn new(
        backend: &'a B,
        token_program: &'a Pubkey,
        registry_program: &'a Pubkey,
        signer: &'a S,
    ) -> Self {
        Self {
            backend,
            token_program,
            registry_program,
            signer,
            _role: std::marker::PhantomData,
        }
    }

    pub fn program_id(&self) -> &Pubkey {
        self.registry_program
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<'a, B, R, S> NamesClient<'a, B, R, S>
where
    B: Backend,
    R: CanInitRegistry,
    S: Signer,
{
    pub async fn initialize(&self, price: u64, token_program: &Pubkey) -> Result<(), Error> {
        let instruction_data = surf_protocol::pack_registry_initialize(price, token_program);
        let (config_pda, _) = surf_protocol::derive_registry_config_pda(self.registry_program);

        let instruction = Instruction::new_with_bytes(
            *self.registry_program,
            &instruction_data,
            vec![
                AccountMeta::new(self.signer.pubkey(), true),
                AccountMeta::new(config_pda, false),
                AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
            ],
        );

        let blockhash = self.backend.get_latest_blockhash().await?;
        let signer = self.signer.pubkey();
        let message = Message::new_with_blockhash(&[instruction], Some(&signer), &blockhash);
        let mut tx = Transaction::new_unsigned(message);
        tx.sign(&[self.signer], blockhash);

        self.backend.send_and_confirm(&tx).await?;
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
impl<'a, B, R, S> NamesClient<'a, B, R, S>
where
    B: crate::backend::WasmBackend,
    R: CanInitRegistry,
    S: Signer,
{
    pub async fn initialize(&self, price: u64, token_program: &Pubkey) -> Result<(), Error> {
        let instruction_data = surf_protocol::pack_registry_initialize(price, token_program);
        let (config_pda, _) = surf_protocol::derive_registry_config_pda(self.registry_program);

        let instruction = Instruction::new_with_bytes(
            *self.registry_program,
            &instruction_data,
            vec![
                AccountMeta::new(self.signer.pubkey(), true),
                AccountMeta::new(config_pda, false),
                AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
            ],
        );

        let blockhash = self.backend.get_latest_blockhash().await?;
        let signer = self.signer.pubkey();
        let message = Message::new_with_blockhash(&[instruction], Some(&signer), &blockhash);
        let mut tx = Transaction::new_unsigned(message);
        tx.sign(&[self.signer], blockhash);

        self.backend.send_and_confirm(&tx).await?;
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<'a, B, R, S> NamesClient<'a, B, R, S>
where
    B: Backend,
    R: CanRegisterName,
    S: Signer,
{
    pub async fn register(&self, name: &str) -> Result<(), Error> {
        let normalized =
            surf_protocol::validate_name(name).map_err(|e| Error::Validation(e.to_string()))?;

        let name_len = name.len();
        let mut name_array = [0u8; 32];
        name_array[..name_len].copy_from_slice(&normalized[..name_len]);

        let payer = self.signer.pubkey();

        let (payer_balance_pda, _) =
            surf_protocol::derive_token_balance_pda(&payer, self.token_program);
        let (name_pda, _) =
            surf_protocol::derive_name_record_pda(&normalized[..name_len], self.registry_program);
        let (registry_config_pda, _) =
            surf_protocol::derive_registry_config_pda(self.registry_program);
        let (token_config_pda, _) = surf_protocol::derive_token_config_pda(self.token_program);

        let instruction_data = surf_protocol::pack_register(&name_array, name_len as u8);

        let instruction = Instruction::new_with_bytes(
            *self.registry_program,
            &instruction_data,
            vec![
                AccountMeta::new(payer, true),
                AccountMeta::new(payer_balance_pda, false),
                AccountMeta::new(name_pda, false),
                AccountMeta::new_readonly(registry_config_pda, false),
                AccountMeta::new(token_config_pda, false),
                AccountMeta::new_readonly(*self.token_program, false),
                AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
            ],
        );

        let blockhash = self.backend.get_latest_blockhash().await?;
        let message = Message::new_with_blockhash(&[instruction], Some(&payer), &blockhash);
        let mut tx = Transaction::new_unsigned(message);
        tx.sign(&[self.signer], blockhash);

        self.backend.send_and_confirm(&tx).await?;
        Ok(())
    }

    pub async fn lookup(&self, name: &str) -> Result<Option<surf_protocol::NameRecord>, Error> {
        let normalized =
            surf_protocol::validate_name(name).map_err(|e| Error::Validation(e.to_string()))?;

        let name_len = name.len();
        let (name_pda, _) =
            surf_protocol::derive_name_record_pda(&normalized[..name_len], self.registry_program);

        let account = self.backend.get_account(&name_pda).await?;

        match account {
            Some(acc) => {
                let record = surf_protocol::decode_name_record(&acc.data)
                    .ok_or(Error::InvalidAccountData)?;
                Ok(Some(record))
            }
            None => Ok(None),
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl<'a, B, R, S> NamesClient<'a, B, R, S>
where
    B: crate::backend::WasmBackend,
    R: CanRegisterName,
    S: Signer,
{
    pub async fn register(&self, name: &str) -> Result<(), Error> {
        let normalized =
            surf_protocol::validate_name(name).map_err(|e| Error::Validation(e.to_string()))?;

        let name_len = name.len();
        let mut name_array = [0u8; 32];
        name_array[..name_len].copy_from_slice(&normalized[..name_len]);

        let payer = self.signer.pubkey();

        let (payer_balance_pda, _) =
            surf_protocol::derive_token_balance_pda(&payer, self.token_program);
        let (name_pda, _) =
            surf_protocol::derive_name_record_pda(&normalized[..name_len], self.registry_program);
        let (registry_config_pda, _) =
            surf_protocol::derive_registry_config_pda(self.registry_program);
        let (token_config_pda, _) = surf_protocol::derive_token_config_pda(self.token_program);

        let instruction_data = surf_protocol::pack_register(&name_array, name_len as u8);

        let instruction = Instruction::new_with_bytes(
            *self.registry_program,
            &instruction_data,
            vec![
                AccountMeta::new(payer, true),
                AccountMeta::new(payer_balance_pda, false),
                AccountMeta::new(name_pda, false),
                AccountMeta::new_readonly(registry_config_pda, false),
                AccountMeta::new(token_config_pda, false),
                AccountMeta::new_readonly(*self.token_program, false),
                AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
            ],
        );

        let blockhash = self.backend.get_latest_blockhash().await?;
        let message = Message::new_with_blockhash(&[instruction], Some(&payer), &blockhash);
        let mut tx = Transaction::new_unsigned(message);
        tx.sign(&[self.signer], blockhash);

        self.backend.send_and_confirm(&tx).await?;
        Ok(())
    }

    pub async fn lookup(&self, name: &str) -> Result<Option<surf_protocol::NameRecord>, Error> {
        let normalized =
            surf_protocol::validate_name(name).map_err(|e| Error::Validation(e.to_string()))?;

        let name_len = name.len();
        let (name_pda, _) =
            surf_protocol::derive_name_record_pda(&normalized[..name_len], self.registry_program);

        let account = self.backend.get_account(&name_pda).await?;

        match account {
            Some(acc) => {
                let record = surf_protocol::decode_name_record(&acc.data)
                    .ok_or(Error::InvalidAccountData)?;
                Ok(Some(record))
            }
            None => Ok(None),
        }
    }
}
