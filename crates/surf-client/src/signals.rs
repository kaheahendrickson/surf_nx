use solana_instruction::{AccountMeta, Instruction};
use solana_message::Message;
use solana_pubkey::Pubkey;
use solana_transaction::Transaction;

#[cfg(not(target_arch = "wasm32"))]
use crate::backend::Backend;
use crate::error::Error;
use crate::role::{CanInitSignals, CanSendSignal, Sealed};
use solana_signer::Signer;
use surf_protocol::SignalKind;

#[allow(dead_code)]
pub struct SignalsClient<'a, B, R, S> {
    backend: &'a B,
    token_program: &'a Pubkey,
    signals_program: &'a Pubkey,
    signer: &'a S,
    _role: std::marker::PhantomData<R>,
}

#[cfg(not(target_arch = "wasm32"))]
impl<'a, B, R, S> SignalsClient<'a, B, R, S>
where
    B: Backend,
    R: Sealed,
    S: Signer,
{
    pub fn new(
        backend: &'a B,
        token_program: &'a Pubkey,
        signals_program: &'a Pubkey,
        signer: &'a S,
    ) -> Self {
        Self {
            backend,
            token_program,
            signals_program,
            signer,
            _role: std::marker::PhantomData,
        }
    }

    pub fn program_id(&self) -> &Pubkey {
        self.signals_program
    }
}

#[cfg(target_arch = "wasm32")]
impl<'a, B, R, S> SignalsClient<'a, B, R, S>
where
    B: crate::backend::WasmBackend,
    R: Sealed,
    S: Signer,
{
    pub fn new(
        backend: &'a B,
        token_program: &'a Pubkey,
        signals_program: &'a Pubkey,
        signer: &'a S,
    ) -> Self {
        Self {
            backend,
            token_program,
            signals_program,
            signer,
            _role: std::marker::PhantomData,
        }
    }

    pub fn program_id(&self) -> &Pubkey {
        self.signals_program
    }
}

fn validate_signals_program(signals_program: &Pubkey) -> Result<(), Error> {
    if *signals_program == Pubkey::default() {
        return Err(Error::Validation(
            "signals program is not configured on this client".to_string(),
        ));
    }

    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
impl<'a, B, R, S> SignalsClient<'a, B, R, S>
where
    B: Backend,
    R: CanInitSignals,
    S: Signer,
{
    pub async fn initialize(&self, min_balance: u64, token_program: &Pubkey) -> Result<(), Error> {
        validate_signals_program(self.signals_program)?;

        let instruction_data = surf_protocol::pack_signals_initialize(token_program, min_balance);
        let (config_pda, _) = surf_protocol::derive_signals_config_pda(self.signals_program);

        let instruction = Instruction::new_with_bytes(
            *self.signals_program,
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
impl<'a, B, R, S> SignalsClient<'a, B, R, S>
where
    B: crate::backend::WasmBackend,
    R: CanInitSignals,
    S: Signer,
{
    pub async fn initialize(&self, min_balance: u64, token_program: &Pubkey) -> Result<(), Error> {
        validate_signals_program(self.signals_program)?;

        let instruction_data = surf_protocol::pack_signals_initialize(token_program, min_balance);
        let (config_pda, _) = surf_protocol::derive_signals_config_pda(self.signals_program);

        let instruction = Instruction::new_with_bytes(
            *self.signals_program,
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
impl<'a, B, R, S> SignalsClient<'a, B, R, S>
where
    B: Backend,
    R: CanSendSignal,
    S: Signer,
{
    pub async fn follow(&self, target: &Pubkey) -> Result<(), Error> {
        self.signal(SignalKind::Follow, target).await
    }

    pub async fn unfollow(&self, target: &Pubkey) -> Result<(), Error> {
        self.signal(SignalKind::Unfollow, target).await
    }

    pub async fn signal(&self, kind: SignalKind, target: &Pubkey) -> Result<(), Error> {
        validate_signals_program(self.signals_program)?;

        let signer = self.signer.pubkey();
        let (balance_pda, _) = surf_protocol::derive_token_balance_pda(&signer, self.token_program);
        let (config_pda, _) = surf_protocol::derive_signals_config_pda(self.signals_program);
        let instruction_data = surf_protocol::pack_signal(kind, target);

        let instruction = Instruction::new_with_bytes(
            *self.signals_program,
            &instruction_data,
            vec![
                AccountMeta::new(signer, true),
                AccountMeta::new_readonly(balance_pda, false),
                AccountMeta::new_readonly(config_pda, false),
            ],
        );

        let blockhash = self.backend.get_latest_blockhash().await?;
        let message = Message::new_with_blockhash(&[instruction], Some(&signer), &blockhash);
        let mut tx = Transaction::new_unsigned(message);
        tx.sign(&[self.signer], blockhash);

        self.backend.send_and_confirm(&tx).await?;
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
impl<'a, B, R, S> SignalsClient<'a, B, R, S>
where
    B: crate::backend::WasmBackend,
    R: CanSendSignal,
    S: Signer,
{
    pub async fn follow(&self, target: &Pubkey) -> Result<(), Error> {
        self.signal(SignalKind::Follow, target).await
    }

    pub async fn unfollow(&self, target: &Pubkey) -> Result<(), Error> {
        self.signal(SignalKind::Unfollow, target).await
    }

    pub async fn signal(&self, kind: SignalKind, target: &Pubkey) -> Result<(), Error> {
        validate_signals_program(self.signals_program)?;

        let signer = self.signer.pubkey();
        let (balance_pda, _) = surf_protocol::derive_token_balance_pda(&signer, self.token_program);
        let (config_pda, _) = surf_protocol::derive_signals_config_pda(self.signals_program);
        let instruction_data = surf_protocol::pack_signal(kind, target);

        let instruction = Instruction::new_with_bytes(
            *self.signals_program,
            &instruction_data,
            vec![
                AccountMeta::new(signer, true),
                AccountMeta::new_readonly(balance_pda, false),
                AccountMeta::new_readonly(config_pda, false),
            ],
        );

        let blockhash = self.backend.get_latest_blockhash().await?;
        let message = Message::new_with_blockhash(&[instruction], Some(&signer), &blockhash);
        let mut tx = Transaction::new_unsigned(message);
        tx.sign(&[self.signer], blockhash);

        self.backend.send_and_confirm(&tx).await?;
        Ok(())
    }
}
