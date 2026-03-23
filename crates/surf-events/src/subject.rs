use crate::event::EventEnvelope;
use crate::event::EventPayload;

pub fn user_follows_subject(viewer: &solana_pubkey::Pubkey) -> String {
    format!("surf.user.{viewer}.follows")
}

pub fn user_activity_subject(owner: &solana_pubkey::Pubkey) -> String {
    format!("surf.user.{owner}.activity")
}

pub fn user_balance_subject(owner: &solana_pubkey::Pubkey) -> String {
    format!("surf.user.{owner}.balance")
}

pub fn user_lamports_subject(owner: &solana_pubkey::Pubkey) -> String {
    format!("surf.user.{owner}.lamports")
}

pub fn global_names_subject() -> &'static str {
    "surf.global.names"
}

pub fn subject_for_event(event: &EventEnvelope) -> String {
    match &event.payload {
        EventPayload::FollowCreated(payload) => user_follows_subject(&payload.follower),
        EventPayload::FollowRemoved(payload) => user_follows_subject(&payload.follower),
        EventPayload::NameRegistered(_) => global_names_subject().to_owned(),
        EventPayload::BalanceUpdated(payload) => user_balance_subject(&payload.owner),
        EventPayload::LamportsUpdated(payload) => user_lamports_subject(&payload.owner),
        EventPayload::ActivityRecorded(payload) => user_activity_subject(&payload.owner),
    }
}

#[cfg(test)]
mod tests {
    use solana_signature::Signature;

    use crate::event::{EventEnvelope, EventPayload, FollowRemoved};

    use super::{subject_for_event, user_follows_subject};

    fn signature_with_byte(byte: u8) -> Signature {
        Signature::from([byte; 64])
    }

    #[test]
    fn builds_follows_subject_from_pubkey() {
        let viewer = solana_pubkey::Pubkey::new_unique();
        assert_eq!(
            user_follows_subject(&viewer),
            format!("surf.user.{viewer}.follows")
        );
    }

    #[test]
    fn derives_subject_from_follow_event() {
        let follower = solana_pubkey::Pubkey::new_unique();
        let target = solana_pubkey::Pubkey::new_unique();
        let event = EventEnvelope::new(
            EventPayload::FollowRemoved(FollowRemoved { follower, target }),
            7,
            &signature_with_byte(4),
            1,
            10,
        );

        assert_eq!(
            subject_for_event(&event),
            format!("surf.user.{follower}.follows")
        );
    }
}
