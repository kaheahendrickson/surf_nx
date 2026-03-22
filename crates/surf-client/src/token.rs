use solana_instruction::{AccountMeta, Instruction};
use solana_message::Message;
use solana_pubkey::Pubkey;
use solana_transaction::Transaction;

#[cfg(not(target_arch = "wasm32"))]
use crate::backend::Backend;
use crate::error::Error;
use crate::role::{CanBurn, CanInitToken, CanMint, CanTransfer, Sealed};
use solana_signer::Signer;

#[allow(dead_code)]
pub struct TokenClient<'a, B, R, S> {
    backend: &'a B,
    token_program: &'a Pubkey,
    registry_program: &'a Pubkey,
    signer: &'a S,
    _role: std::marker::PhantomData<R>,
}

#[cfg(not(target_arch = "wasm32"))]
impl<'a, B, R, S> TokenClient<'a, B, R, S>
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
        self.token_program
    }
}

#[cfg(target_arch = "wasm32")]
impl<'a, B, R, S> TokenClient<'a, B, R, S>
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
        self.token_program
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<'a, B, R, S> TokenClient<'a, B, R, S>
where
    B: Backend,
    R: CanInitToken,
    S: Signer,
{
    pub async fn initialize(&self, total_supply: u64, decimals: u8) -> Result<(), Error> {
        let instruction_data = surf_protocol::pack_token_initialize(total_supply, decimals);
        let (config_pda, _) = surf_protocol::derive_token_config_pda(self.token_program);
        let (distribution_pda, _) =
            surf_protocol::derive_token_balance_pda(&self.signer.pubkey(), self.token_program);

        let instruction = Instruction::new_with_bytes(
            *self.token_program,
            &instruction_data,
            vec![
                AccountMeta::new(self.signer.pubkey(), true),
                AccountMeta::new(distribution_pda, false),
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
impl<'a, B, R, S> TokenClient<'a, B, R, S>
where
    B: crate::backend::WasmBackend,
    R: CanInitToken,
    S: Signer,
{
    pub async fn initialize(&self, total_supply: u64, decimals: u8) -> Result<(), Error> {
        let instruction_data = surf_protocol::pack_token_initialize(total_supply, decimals);
        let (config_pda, _) = surf_protocol::derive_token_config_pda(self.token_program);
        let (distribution_pda, _) =
            surf_protocol::derive_token_balance_pda(&self.signer.pubkey(), self.token_program);

        let instruction = Instruction::new_with_bytes(
            *self.token_program,
            &instruction_data,
            vec![
                AccountMeta::new(self.signer.pubkey(), true),
                AccountMeta::new(distribution_pda, false),
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
impl<'a, B, R, S> TokenClient<'a, B, R, S>
where
    B: Backend,
    R: CanMint,
    S: Signer,
{
    pub async fn mint(&self, recipient: &Pubkey, amount: u64) -> Result<(), Error> {
        let (recipient_balance_pda, _) =
            surf_protocol::derive_token_balance_pda(recipient, self.token_program);
        let (config_pda, _) = surf_protocol::derive_token_config_pda(self.token_program);

        let instruction_data = surf_protocol::pack_mint(amount);
        let instruction = Instruction::new_with_bytes(
            *self.token_program,
            &instruction_data,
            vec![
                AccountMeta::new(self.signer.pubkey(), true),
                AccountMeta::new_readonly(*recipient, false),
                AccountMeta::new(recipient_balance_pda, false),
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
impl<'a, B, R, S> TokenClient<'a, B, R, S>
where
    B: crate::backend::WasmBackend,
    R: CanMint,
    S: Signer,
{
    pub async fn mint(&self, recipient: &Pubkey, amount: u64) -> Result<(), Error> {
        let (recipient_balance_pda, _) =
            surf_protocol::derive_token_balance_pda(recipient, self.token_program);
        let (config_pda, _) = surf_protocol::derive_token_config_pda(self.token_program);

        let instruction_data = surf_protocol::pack_mint(amount);
        let instruction = Instruction::new_with_bytes(
            *self.token_program,
            &instruction_data,
            vec![
                AccountMeta::new(self.signer.pubkey(), true),
                AccountMeta::new_readonly(*recipient, false),
                AccountMeta::new(recipient_balance_pda, false),
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
impl<'a, B, R, S> TokenClient<'a, B, R, S>
where
    B: Backend,
    R: CanTransfer,
    S: Signer,
{
    pub async fn transfer(&self, recipient: &Pubkey, amount: u64) -> Result<(), Error> {
        let sender = self.signer.pubkey();

        let (sender_balance_pda, _) =
            surf_protocol::derive_token_balance_pda(&sender, self.token_program);
        let (recipient_balance_pda, _) =
            surf_protocol::derive_token_balance_pda(recipient, self.token_program);

        let instruction_data = surf_protocol::pack_transfer(amount);

        let instruction = Instruction::new_with_bytes(
            *self.token_program,
            &instruction_data,
            vec![
                AccountMeta::new(sender, true),
                AccountMeta::new(sender_balance_pda, false),
                AccountMeta::new_readonly(*recipient, false),
                AccountMeta::new(recipient_balance_pda, false),
                AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
            ],
        );

        let blockhash = self.backend.get_latest_blockhash().await?;
        let message = Message::new_with_blockhash(&[instruction], Some(&sender), &blockhash);
        let mut tx = Transaction::new_unsigned(message);
        tx.sign(&[self.signer], blockhash);

        self.backend.send_and_confirm(&tx).await?;
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
impl<'a, B, R, S> TokenClient<'a, B, R, S>
where
    B: crate::backend::WasmBackend,
    R: CanTransfer,
    S: Signer,
{
    pub async fn transfer(&self, recipient: &Pubkey, amount: u64) -> Result<(), Error> {
        let sender = self.signer.pubkey();

        let (sender_balance_pda, _) =
            surf_protocol::derive_token_balance_pda(&sender, self.token_program);
        let (recipient_balance_pda, _) =
            surf_protocol::derive_token_balance_pda(recipient, self.token_program);

        let instruction_data = surf_protocol::pack_transfer(amount);

        let instruction = Instruction::new_with_bytes(
            *self.token_program,
            &instruction_data,
            vec![
                AccountMeta::new(sender, true),
                AccountMeta::new(sender_balance_pda, false),
                AccountMeta::new_readonly(*recipient, false),
                AccountMeta::new(recipient_balance_pda, false),
                AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
            ],
        );

        let blockhash = self.backend.get_latest_blockhash().await?;
        let message = Message::new_with_blockhash(&[instruction], Some(&sender), &blockhash);
        let mut tx = Transaction::new_unsigned(message);
        tx.sign(&[self.signer], blockhash);

        self.backend.send_and_confirm(&tx).await?;
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<'a, B, R, S> TokenClient<'a, B, R, S>
where
    B: Backend,
    R: CanBurn,
    S: Signer,
{
    pub async fn burn(&self, amount: u64) -> Result<(), Error> {
        let holder = self.signer.pubkey();

        let (holder_balance_pda, _) =
            surf_protocol::derive_token_balance_pda(&holder, self.token_program);
        let (config_pda, _) = surf_protocol::derive_token_config_pda(self.token_program);

        let instruction_data = surf_protocol::pack_burn(amount);

        let instruction = Instruction::new_with_bytes(
            *self.token_program,
            &instruction_data,
            vec![
                AccountMeta::new(holder, true),
                AccountMeta::new(holder_balance_pda, false),
                AccountMeta::new(config_pda, false),
                AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
            ],
        );

        let blockhash = self.backend.get_latest_blockhash().await?;
        let message = Message::new_with_blockhash(&[instruction], Some(&holder), &blockhash);
        let mut tx = Transaction::new_unsigned(message);
        tx.sign(&[self.signer], blockhash);

        self.backend.send_and_confirm(&tx).await?;
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
impl<'a, B, R, S> TokenClient<'a, B, R, S>
where
    B: crate::backend::WasmBackend,
    R: CanBurn,
    S: Signer,
{
    pub async fn burn(&self, amount: u64) -> Result<(), Error> {
        let holder = self.signer.pubkey();

        let (holder_balance_pda, _) =
            surf_protocol::derive_token_balance_pda(&holder, self.token_program);
        let (config_pda, _) = surf_protocol::derive_token_config_pda(self.token_program);

        let instruction_data = surf_protocol::pack_burn(amount);

        let instruction = Instruction::new_with_bytes(
            *self.token_program,
            &instruction_data,
            vec![
                AccountMeta::new(holder, true),
                AccountMeta::new(holder_balance_pda, false),
                AccountMeta::new(config_pda, false),
                AccountMeta::new_readonly(solana_sdk_ids::system_program::id(), false),
            ],
        );

        let blockhash = self.backend.get_latest_blockhash().await?;
        let message = Message::new_with_blockhash(&[instruction], Some(&holder), &blockhash);
        let mut tx = Transaction::new_unsigned(message);
        tx.sign(&[self.signer], blockhash);

        self.backend.send_and_confirm(&tx).await?;
        Ok(())
    }
}
