use solana_pubkey::Pubkey;
use solana_signature::Signature;
use surf_client::{Backend, ParsedTransaction};
use surf_events::{EventEnvelope, EventPayload, FollowCreated, FollowRemoved};
use surf_protocol::SignalKind;

use crate::error::SyncError;
use crate::publisher::{EventPublisher, PublishedEvent};

pub struct FollowSyncService<P> {
    publisher: P,
    signals_program: Pubkey,
}

impl<P> FollowSyncService<P>
where
    P: EventPublisher,
{
    pub fn new(publisher: P, signals_program: Pubkey) -> Self {
        Self {
            publisher,
            signals_program,
        }
    }

    pub async fn publish_transaction(
        &self,
        transaction: &ParsedTransaction,
    ) -> Result<Vec<PublishedEvent>, SyncError> {
        let events = FollowEventMapper::new(self.signals_program).map_transaction(transaction)?;
        let mut published = Vec::with_capacity(events.len());
        for event in &events {
            published.push(self.publisher.publish(event).await?);
        }
        Ok(published)
    }

    pub async fn sync_signature<B: Backend>(
        &self,
        backend: &B,
        signature: &Signature,
    ) -> Result<Vec<PublishedEvent>, SyncError> {
        let Some(transaction) = backend.get_transaction(signature).await? else {
            return Ok(Vec::new());
        };
        self.publish_transaction(&transaction).await
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FollowEventMapper {
    signals_program: Pubkey,
}

impl FollowEventMapper {
    pub fn new(signals_program: Pubkey) -> Self {
        Self { signals_program }
    }

    pub fn map_transaction(
        &self,
        transaction: &ParsedTransaction,
    ) -> Result<Vec<EventEnvelope>, SyncError> {
        let signature = transaction
            .signatures
            .first()
            .ok_or(SyncError::InvalidSignalInstruction)?;
        let observed_at = transaction.block_time.unwrap_or(-1);
        let mut events = Vec::new();

        for (instruction_index, instruction) in transaction.message.instructions.iter().enumerate() {
            let Some(program_id) = transaction
                .message
                .account_keys
                .get(instruction.program_id_index as usize)
                .copied()
            else {
                continue;
            };
            if program_id != self.signals_program {
                continue;
            }

            let Some(payload) = parse_follow_event_payload(transaction, instruction)? else {
                continue;
            };

            events.push(EventEnvelope::new(
                payload,
                transaction.slot,
                signature,
                instruction_index as u8,
                observed_at,
            ));
        }

        Ok(events)
    }
}

fn parse_follow_event_payload(
    transaction: &ParsedTransaction,
    instruction: &surf_client::InstructionInfo,
) -> Result<Option<EventPayload>, SyncError> {
    if instruction.data.len() < 34
        || instruction.data[0] != surf_protocol::instruction::signals::SIGNAL_DISCRIMINATOR
    {
        return Ok(None);
    }

    let accounts = instruction
        .accounts
        .iter()
        .filter_map(|index| transaction.message.account_keys.get(*index as usize).copied())
        .collect::<Vec<_>>();
    let Some(follower) = accounts.first().copied() else {
        return Err(SyncError::InvalidSignalInstruction);
    };

    let target = Pubkey::try_from(&instruction.data[2..34])
        .map_err(|_| SyncError::InvalidSignalInstruction)?;
    let kind = match instruction.data[1] {
        0 => SignalKind::Follow,
        1 => SignalKind::Unfollow,
        _ => return Err(SyncError::InvalidSignalInstruction),
    };

    let payload = match kind {
        SignalKind::Follow => EventPayload::FollowCreated(FollowCreated { follower, target }),
        SignalKind::Unfollow => EventPayload::FollowRemoved(FollowRemoved { follower, target }),
    };
    Ok(Some(payload))
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use surf_client::{InstructionInfo, TransactionMessage};
    use surf_protocol::{pack_signal, SignalKind};

    use surf_events::{subject_for_event, EventPayload};

    use super::*;

    #[derive(Clone, Default)]
    struct MockPublisher {
        published: Arc<Mutex<Vec<EventEnvelope>>>,
        fail_with: Option<String>,
    }

    impl MockPublisher {
        fn failing(message: &str) -> Self {
            Self {
                published: Arc::new(Mutex::new(Vec::new())),
                fail_with: Some(message.to_owned()),
            }
        }

        fn published(&self) -> Vec<EventEnvelope> {
            self.published.lock().unwrap().clone()
        }
    }

    impl EventPublisher for MockPublisher {
        async fn publish(
            &self,
            event: &EventEnvelope,
        ) -> Result<PublishedEvent, crate::error::EventPublishError> {
            if let Some(message) = &self.fail_with {
                return Err(crate::error::EventPublishError::Publish(message.clone()));
            }
            self.published.lock().unwrap().push(event.clone());
            Ok(PublishedEvent {
                subject: subject_for_event(event),
                payload: serde_json::to_vec(event).unwrap(),
            })
        }
    }

    fn signature_with_byte(byte: u8) -> Signature {
        Signature::from([byte; 64])
    }

    fn follow_transaction(signals_program: Pubkey, kind: SignalKind) -> ParsedTransaction {
        let follower = Pubkey::new_unique();
        let target = Pubkey::new_unique();
        ParsedTransaction {
            slot: 99,
            block_time: Some(1_700_000_000),
            signatures: vec![signature_with_byte(kind as u8 + 1)],
            message: TransactionMessage {
                account_keys: vec![signals_program, follower, target],
                instructions: vec![InstructionInfo {
                    program_id_index: 0,
                    accounts: vec![1, 2],
                    data: pack_signal(kind, &target),
                }],
            },
        }
    }

    #[test]
    fn mapper_creates_follow_created_event() {
        let signals_program = Pubkey::new_unique();
        let transaction = follow_transaction(signals_program, SignalKind::Follow);
        let mapper = FollowEventMapper::new(signals_program);

        let events = mapper.map_transaction(&transaction).unwrap();

        assert_eq!(events.len(), 1);
        match &events[0].payload {
            EventPayload::FollowCreated(payload) => {
                assert_eq!(payload.follower, transaction.message.account_keys[1]);
                assert_eq!(payload.target, transaction.message.account_keys[2]);
            }
            other => panic!("unexpected payload: {other:?}"),
        }
    }

    #[test]
    fn mapper_creates_follow_removed_event() {
        let signals_program = Pubkey::new_unique();
        let transaction = follow_transaction(signals_program, SignalKind::Unfollow);
        let mapper = FollowEventMapper::new(signals_program);

        let events = mapper.map_transaction(&transaction).unwrap();

        assert_eq!(events.len(), 1);
        match &events[0].payload {
            EventPayload::FollowRemoved(payload) => {
                assert_eq!(payload.follower, transaction.message.account_keys[1]);
                assert_eq!(payload.target, transaction.message.account_keys[2]);
            }
            other => panic!("unexpected payload: {other:?}"),
        }
    }

    #[tokio::test]
    async fn service_publishes_all_mapped_events() {
        let signals_program = Pubkey::new_unique();
        let publisher = MockPublisher::default();
        let service = FollowSyncService::new(publisher.clone(), signals_program);
        let transaction = follow_transaction(signals_program, SignalKind::Follow);

        let published = service.publish_transaction(&transaction).await.unwrap();

        assert_eq!(published.len(), 1);
        assert_eq!(publisher.published().len(), 1);
    }

    #[tokio::test]
    async fn service_surfaces_publish_errors() {
        let signals_program = Pubkey::new_unique();
        let publisher = MockPublisher::failing("nats down");
        let service = FollowSyncService::new(publisher, signals_program);
        let transaction = follow_transaction(signals_program, SignalKind::Follow);

        let err = service.publish_transaction(&transaction).await.unwrap_err();

        assert!(matches!(
            err,
            SyncError::Publish(crate::error::EventPublishError::Publish(message)) if message == "nats down"
        ));
    }
}
