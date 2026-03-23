use surf_client::Backend;
use surf_events::{BalanceUpdated, EventEnvelope, EventPayload, LamportsUpdated};
use surf_protocol::{derive_token_balance_pda, decode_token_balance};

use crate::checkpoint::BalanceSnapshot;
use crate::error::SyncError;
use crate::publisher::{EventPublisher, PublishedEvent};

pub struct BalanceSyncService<P> { publisher: P, token_program: solana_pubkey::Pubkey }

impl<P: EventPublisher> BalanceSyncService<P> {
    pub fn new(publisher: P, token_program: solana_pubkey::Pubkey) -> Self { Self { publisher, token_program } }

    pub async fn sync_owner<B: Backend>(&self, backend: &B, owner: &solana_pubkey::Pubkey) -> Result<Vec<PublishedEvent>, SyncError> {
        Ok(self.sync_owner_if_changed(backend, owner, &BalanceSnapshot::default()).await?.0)
    }

    pub async fn sync_owner_if_changed<B: Backend>(
        &self,
        backend: &B,
        owner: &solana_pubkey::Pubkey,
        checkpoint: &BalanceSnapshot,
    ) -> Result<(Vec<PublishedEvent>, BalanceSnapshot), SyncError> {
        let mut published = Vec::new();
        let (pda, _) = derive_token_balance_pda(owner, &self.token_program);
        let account = backend.get_account(&pda).await?;
        let balance_record = account.as_ref().map(|a| a.data.clone()).unwrap_or_default();
        let amount = account.as_ref().and_then(|a| decode_token_balance(&a.data).map(|b| b.amount)).unwrap_or(0);
        let lamports = backend.get_balance(owner).await?.unwrap_or(0);
        let signature = solana_signature::Signature::from([0; 64]);
        if checkpoint.amount != Some(amount) {
            let balance_event = EventEnvelope::new(EventPayload::BalanceUpdated(BalanceUpdated { owner: *owner, amount, record: balance_record }), 0, &signature, 0, 0);
            published.push(self.publisher.publish(&balance_event).await?);
        }
        if checkpoint.lamports != Some(lamports) {
            let lamports_event = EventEnvelope::new(EventPayload::LamportsUpdated(LamportsUpdated { owner: *owner, lamports }), 0, &signature, 1, 0);
            published.push(self.publisher.publish(&lamports_event).await?);
        }
        Ok((published, BalanceSnapshot { amount: Some(amount), lamports: Some(lamports) }))
    }
}
