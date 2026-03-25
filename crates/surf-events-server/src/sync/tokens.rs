use solana_pubkey::Pubkey;
use solana_signature::Signature;
use surf_client::{Backend, InstructionInfo, ParsedTransaction, SignaturesForAddressOptions};
use surf_events::{ActivityRecorded, BalanceUpdated, EventEnvelope, EventPayload};
use surf_protocol::{decode_token_balance, derive_token_balance_pda};
use surf_sync::parser::{
    is_token_burn_instruction, is_token_mint_instruction, is_token_transfer_instruction,
    parse_token_burn_instruction, parse_token_mint_instruction, parse_token_transfer_instruction,
};

use crate::checkpoint::SignatureCursor;
use crate::error::SyncError;
use crate::publisher::{EventPublisher, PublishedEvent};

pub struct TokenSyncService<P> {
    publisher: P,
    token_program: Pubkey,
}

impl<P: EventPublisher> TokenSyncService<P> {
    pub fn new(publisher: P, token_program: Pubkey) -> Self {
        Self {
            publisher,
            token_program,
        }
    }

    pub async fn sync_recent<B: Backend>(
        &self,
        backend: &B,
        limit: usize,
    ) -> Result<Vec<PublishedEvent>, SyncError> {
        Ok(self
            .sync_recent_since(backend, limit, &SignatureCursor::default())
            .await?
            .0)
    }

    pub async fn sync_recent_since<B: Backend>(
        &self,
        backend: &B,
        limit: usize,
        checkpoint: &SignatureCursor,
    ) -> Result<(Vec<PublishedEvent>, SignatureCursor), SyncError> {
        let signatures = backend
            .get_signatures_for_address(
                &self.token_program,
                Some(SignaturesForAddressOptions {
                    limit: Some(limit),
                    ..Default::default()
                }),
            )
            .await?;
        let mut published = Vec::new();
        let mut next = checkpoint.clone();
        let mut processed = Vec::new();

        for sig in signatures.into_iter().rev() {
            if !checkpoint.should_process(&sig.signature, sig.slot) {
                continue;
            }
            if let Some(tx) = backend.get_transaction(&sig.signature).await? {
                published.extend(self.publish_transaction(backend, &tx).await?);
            }
            processed.push((sig.signature, sig.slot));
        }

        next.advance(processed);
        Ok((published, next))
    }

    async fn publish_transaction<B: Backend>(
        &self,
        backend: &B,
        tx: &ParsedTransaction,
    ) -> Result<Vec<PublishedEvent>, SyncError> {
        let signature = tx
            .signatures
            .first()
            .copied()
            .unwrap_or_else(|| Signature::from([0u8; 64]));
        let slot = tx.slot;
        let observed_at = tx.block_time.unwrap_or(-1);
        let mut published = Vec::new();

        for (instruction_index, instruction) in tx.message.instructions.iter().enumerate() {
            let Some(program_id) = tx
                .message
                .account_keys
                .get(instruction.program_id_index as usize)
                .copied()
            else {
                continue;
            };

            if program_id != self.token_program {
                continue;
            }

            if let Some(events) = self
                .process_token_instruction(backend, instruction, &tx.message.account_keys, signature, slot, instruction_index as u8, observed_at)
                .await?
            {
                published.extend(events);
            }
        }

        Ok(published)
    }

    async fn process_token_instruction<B: Backend>(
        &self,
        backend: &B,
        instruction: &InstructionInfo,
        account_keys: &[Pubkey],
        signature: Signature,
        slot: u64,
        instruction_index: u8,
        observed_at: i64,
    ) -> Result<Option<Vec<PublishedEvent>>, SyncError> {
        let accounts: Vec<Pubkey> = instruction
            .accounts
            .iter()
            .filter_map(|idx| account_keys.get(*idx as usize).copied())
            .collect();

        if is_token_mint_instruction(&instruction.data) {
            return self
                .handle_mint(backend, &accounts, &instruction.data, signature, slot, instruction_index, observed_at)
                .await
                .map(Some);
        }

        if is_token_transfer_instruction(&instruction.data) {
            return self
                .handle_transfer(backend, &accounts, &instruction.data, signature, slot, instruction_index, observed_at)
                .await
                .map(Some);
        }

        if is_token_burn_instruction(&instruction.data) {
            return self
                .handle_burn(backend, &accounts, &instruction.data, signature, slot, instruction_index, observed_at)
                .await
                .map(Some);
        }

        Ok(None)
    }

    async fn handle_mint<B: Backend>(
        &self,
        backend: &B,
        accounts: &[Pubkey],
        data: &[u8],
        signature: Signature,
        slot: u64,
        instruction_index: u8,
        observed_at: i64,
    ) -> Result<Vec<PublishedEvent>, SyncError> {
        let _parsed = parse_token_mint_instruction(data)?;
        let recipient = accounts.get(1).copied();
        let mut published = Vec::new();

        if let Some(recipient) = recipient {
            let (pda, _) = derive_token_balance_pda(&recipient, &self.token_program);
            if let Some(account) = backend.get_account(&pda).await? {
                let record = account.data.clone();
                let amount = decode_token_balance(&account.data).map(|b| b.amount).unwrap_or(0);
                let event = EventEnvelope::new(
                    EventPayload::BalanceUpdated(BalanceUpdated {
                        owner: recipient,
                        amount,
                        record,
                    }),
                    slot,
                    &signature,
                    instruction_index,
                    observed_at,
                );
                published.push(self.publisher.publish(&event).await?);
            }
        }

        Ok(published)
    }

    async fn handle_transfer<B: Backend>(
        &self,
        backend: &B,
        accounts: &[Pubkey],
        data: &[u8],
        signature: Signature,
        slot: u64,
        instruction_index: u8,
        observed_at: i64,
    ) -> Result<Vec<PublishedEvent>, SyncError> {
        let parsed = parse_token_transfer_instruction(data)?;
        let sender = accounts.get(0).copied();
        let recipient = accounts.get(2).copied();
        let mut published = Vec::new();

        if let Some(sender) = sender {
            let (pda, _) = derive_token_balance_pda(&sender, &self.token_program);
            if let Some(account) = backend.get_account(&pda).await? {
                let record = account.data.clone();
                let amount = decode_token_balance(&account.data).map(|b| b.amount).unwrap_or(0);
                let event = EventEnvelope::new(
                    EventPayload::BalanceUpdated(BalanceUpdated {
                        owner: sender,
                        amount,
                        record,
                    }),
                    slot,
                    &signature,
                    instruction_index,
                    observed_at,
                );
                published.push(self.publisher.publish(&event).await?);
            }

            if let Some(recipient) = recipient {
                let activity_event = EventEnvelope::new(
                    EventPayload::ActivityRecorded(ActivityRecorded {
                        owner: sender,
                        kind: surf_events::ActivityKind::SurfSent as u8,
                        counterparty: recipient,
                        amount: parsed.amount,
                    }),
                    slot,
                    &signature,
                    instruction_index,
                    observed_at,
                );
                published.push(self.publisher.publish(&activity_event).await?);
            }
        }

        if let Some(recipient) = recipient {
            let (pda, _) = derive_token_balance_pda(&recipient, &self.token_program);
            if let Some(account) = backend.get_account(&pda).await? {
                let record = account.data.clone();
                let amount = decode_token_balance(&account.data).map(|b| b.amount).unwrap_or(0);
                let event = EventEnvelope::new(
                    EventPayload::BalanceUpdated(BalanceUpdated {
                        owner: recipient,
                        amount,
                        record,
                    }),
                    slot,
                    &signature,
                    instruction_index,
                    observed_at,
                );
                published.push(self.publisher.publish(&event).await?);
            }
        }

        Ok(published)
    }

    async fn handle_burn<B: Backend>(
        &self,
        backend: &B,
        accounts: &[Pubkey],
        data: &[u8],
        signature: Signature,
        slot: u64,
        instruction_index: u8,
        observed_at: i64,
    ) -> Result<Vec<PublishedEvent>, SyncError> {
        let _parsed = parse_token_burn_instruction(data)?;
        let holder = accounts.get(0).copied();
        let mut published = Vec::new();

        if let Some(holder) = holder {
            let (pda, _) = derive_token_balance_pda(&holder, &self.token_program);
            if let Some(account) = backend.get_account(&pda).await? {
                let record = account.data.clone();
                let amount = decode_token_balance(&account.data).map(|b| b.amount).unwrap_or(0);
                let event = EventEnvelope::new(
                    EventPayload::BalanceUpdated(BalanceUpdated {
                        owner: holder,
                        amount,
                        record,
                    }),
                    slot,
                    &signature,
                    instruction_index,
                    observed_at,
                );
                published.push(self.publisher.publish(&event).await?);
            }
        }

        Ok(published)
    }
}
