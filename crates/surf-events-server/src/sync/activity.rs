use surf_client::{Backend, SignaturesForAddressOptions};
use surf_events::{ActivityRecorded, EventEnvelope, EventPayload};

use crate::checkpoint::SignatureCursor;
use crate::error::SyncError;
use crate::publisher::{EventPublisher, PublishedEvent};

pub struct ActivitySyncService<P> {
    publisher: P,
    tracked_owner: solana_pubkey::Pubkey,
    token_program: solana_pubkey::Pubkey,
    registry_program: solana_pubkey::Pubkey,
    signals_program: solana_pubkey::Pubkey,
}

impl<P: EventPublisher> ActivitySyncService<P> {
    pub fn new(publisher: P, tracked_owner: solana_pubkey::Pubkey, token_program: solana_pubkey::Pubkey, registry_program: solana_pubkey::Pubkey, signals_program: solana_pubkey::Pubkey) -> Self {
        Self { publisher, tracked_owner, token_program, registry_program, signals_program }
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
        let signatures = backend.get_signatures_for_address(&self.tracked_owner, Some(SignaturesForAddressOptions { limit: Some(limit), ..Default::default() })).await?;
        let mut published = Vec::new();
        let mut next = checkpoint.clone();
        let mut processed = Vec::new();
        for sig in signatures.into_iter().rev() {
            if !checkpoint.should_process(&sig.signature, sig.slot) {
                continue;
            }
            let Some(tx) = backend.get_transaction(&sig.signature).await? else { continue; };
            for (instruction_index, instruction) in tx.message.instructions.iter().enumerate() {
                let Some(activity) = surf_sync::parser::parse_curated_activity(&tx, instruction, &self.tracked_owner, &self.token_program, &self.registry_program, &self.signals_program).ok().flatten() else { continue; };
                let envelope = EventEnvelope::new(EventPayload::ActivityRecorded(ActivityRecorded { owner: self.tracked_owner, kind: activity.kind.as_u8(), counterparty: activity.counterparty, amount: activity.amount }), tx.slot, tx.signatures.first().ok_or(SyncError::InvalidSignalInstruction)?, instruction_index as u8, tx.block_time.unwrap_or(-1));
                published.push(self.publisher.publish(&envelope).await?);
            }
            processed.push((sig.signature, sig.slot));
        }
        next.advance(processed);
        Ok((published, next))
    }
}
