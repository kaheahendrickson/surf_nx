use surf_client::{Backend, ParsedTransaction, ProgramAccountsFilter, SignaturesForAddressOptions};
use surf_events::{ActivityKind, ActivityRecorded, EventEnvelope, EventPayload, NameRegistered};
use surf_protocol::{decode_name_record, derive_name_record_pda, NameRecord};

use crate::checkpoint::SignatureCursor;
use crate::error::SyncError;
use crate::publisher::{EventPublisher, PublishedEvent};

pub struct NameSyncService<P> {
    publisher: P,
    registry_program: solana_pubkey::Pubkey,
}

impl<P: EventPublisher> NameSyncService<P> {
    pub fn new(publisher: P, registry_program: solana_pubkey::Pubkey) -> Self {
        Self {
            publisher,
            registry_program,
        }
    }

    pub async fn bootstrap<B: Backend>(&self, backend: &B) -> Result<Vec<PublishedEvent>, SyncError> {
        let accounts = backend.get_program_accounts(&self.registry_program, Some(ProgramAccountsFilter { data_size: Some(NameRecord::LEN) })).await?;
        let mut published = Vec::new();
        for account_info in accounts {
            let Some(record) = decode_name_record(&account_info.account.data) else { continue };
            let name = std::str::from_utf8(&record.name[..record.len as usize])
                .map_err(|_| SyncError::InvalidSignalInstruction)?
                .to_owned();
            let (expected_pda, _) = derive_name_record_pda(name.as_bytes(), &self.registry_program);
            if expected_pda != account_info.pubkey {
                continue;
            }
            let name_event = EventPayload::NameRegistered(NameRegistered {
                name: name.clone(),
                owner: record.owner,
                record: account_info.account.data.clone(),
            });
            let activity_event = EventPayload::ActivityRecorded(ActivityRecorded {
                owner: record.owner,
                kind: ActivityKind::NameRegistered.as_u8(),
                counterparty: record.owner,
                amount: 0,
            });
            let signature = solana_signature::Signature::from([0u8; 64]);
            published.push(self.publisher.publish(&EventEnvelope::new(
                name_event,
                0,
                &signature,
                0,
                0,
            )).await?);
            published.push(self.publisher.publish(&EventEnvelope::new(
                activity_event,
                0,
                &signature,
                1,
                0,
            )).await?);
        }
        Ok(published)
    }

    pub async fn sync_recent<B: Backend>(&self, backend: &B, limit: usize) -> Result<Vec<PublishedEvent>, SyncError> {
        Ok(self.sync_recent_since(backend, limit, &SignatureCursor::default()).await?.0)
    }

    pub async fn sync_recent_since<B: Backend>(
        &self,
        backend: &B,
        limit: usize,
        checkpoint: &SignatureCursor,
    ) -> Result<(Vec<PublishedEvent>, SignatureCursor), SyncError> {
        let signatures = backend.get_signatures_for_address(&self.registry_program, Some(SignaturesForAddressOptions { limit: Some(limit), ..Default::default() })).await?;
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

    async fn publish_transaction<B: Backend>(&self, backend: &B, tx: &ParsedTransaction) -> Result<Vec<PublishedEvent>, SyncError> {
        let mut published = Vec::new();
        for (instruction_index, instruction) in tx.message.instructions.iter().enumerate() {
            if !crate::sync::helpers::instruction_matches_program(tx, instruction, &self.registry_program) { continue; }
            let Some(name) = parse_registered_name(&instruction.data)? else { continue; };
            let (pda, _) = derive_name_record_pda(name.as_bytes(), &self.registry_program);
            let Some(account) = backend.get_account(&pda).await? else { continue; };
            let Some(record) = decode_name_record(&account.data) else { continue; };

            let name_event = EventPayload::NameRegistered(NameRegistered {
                name: name.clone(),
                owner: record.owner,
                record: account.data.clone(),
            });
            let activity_event = EventPayload::ActivityRecorded(ActivityRecorded {
                owner: record.owner,
                kind: ActivityKind::NameRegistered.as_u8(),
                counterparty: record.owner,
                amount: 0,
            });

            published.push(self.publisher.publish(&EventEnvelope::new(
                name_event,
                tx.slot,
                tx.signatures.first().ok_or(SyncError::InvalidSignalInstruction)?,
                instruction_index as u8,
                tx.block_time.unwrap_or(-1),
            )).await?);
                published.push(self.publisher.publish(&EventEnvelope::new(
                activity_event,
                tx.slot,
                tx.signatures.first().ok_or(SyncError::InvalidSignalInstruction)?,
                instruction_index as u8 + 1,
                tx.block_time.unwrap_or(-1),
            )).await?);
        }
        Ok(published)
    }
}

fn parse_registered_name(data: &[u8]) -> Result<Option<String>, SyncError> {
    if data.len() < 34 || data[0] != 1 {
        return Ok(None);
    }
    let name_len = data[33] as usize;
    if name_len > 32 {
        return Err(SyncError::InvalidSignalInstruction);
    }
    let name = std::str::from_utf8(&data[1..1 + name_len]).map_err(|_| SyncError::InvalidSignalInstruction)?;
    Ok(Some(name.to_owned()))
}
