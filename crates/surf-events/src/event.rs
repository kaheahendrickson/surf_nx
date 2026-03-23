use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

pub type SchemaVersion = u16;
pub const SCHEMA_VERSION_V1: SchemaVersion = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum EventPayload {
    #[serde(rename = "follow.created")]
    FollowCreated(FollowCreated),
    #[serde(rename = "follow.removed")]
    FollowRemoved(FollowRemoved),
    #[serde(rename = "name.registered")]
    NameRegistered(NameRegistered),
    #[serde(rename = "balance.updated")]
    BalanceUpdated(BalanceUpdated),
    #[serde(rename = "lamports.updated")]
    LamportsUpdated(LamportsUpdated),
    #[serde(rename = "activity.recorded")]
    ActivityRecorded(ActivityRecorded),
}

impl EventPayload {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::FollowCreated(_) => "follow.created",
            Self::FollowRemoved(_) => "follow.removed",
            Self::NameRegistered(_) => "name.registered",
            Self::BalanceUpdated(_) => "balance.updated",
            Self::LamportsUpdated(_) => "lamports.updated",
            Self::ActivityRecorded(_) => "activity.recorded",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub schema_version: SchemaVersion,
    pub event_id: String,
    pub slot: u64,
    pub signature: String,
    pub instruction_index: u8,
    pub observed_at: i64,
    #[serde(flatten)]
    pub payload: EventPayload,
}

impl EventEnvelope {
    pub fn new(
        payload: EventPayload,
        slot: u64,
        signature: &Signature,
        instruction_index: u8,
        observed_at: i64,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION_V1,
            event_id: event_id(payload.event_type(), signature, instruction_index),
            slot,
            signature: signature.to_string(),
            instruction_index,
            observed_at,
            payload,
        }
    }
}

pub fn event_id(event_type: &str, signature: &Signature, instruction_index: u8) -> String {
    format!("{event_type}:{signature}:{instruction_index}")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FollowCreated {
    #[serde(
        serialize_with = "serialize_pubkey",
        deserialize_with = "deserialize_pubkey"
    )]
    pub follower: Pubkey,
    #[serde(
        serialize_with = "serialize_pubkey",
        deserialize_with = "deserialize_pubkey"
    )]
    pub target: Pubkey,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FollowRemoved {
    #[serde(
        serialize_with = "serialize_pubkey",
        deserialize_with = "deserialize_pubkey"
    )]
    pub follower: Pubkey,
    #[serde(
        serialize_with = "serialize_pubkey",
        deserialize_with = "deserialize_pubkey"
    )]
    pub target: Pubkey,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NameRegistered {
    pub name: String,
    #[serde(
        serialize_with = "serialize_pubkey",
        deserialize_with = "deserialize_pubkey"
    )]
    pub owner: Pubkey,
    pub record: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BalanceUpdated {
    #[serde(
        serialize_with = "serialize_pubkey",
        deserialize_with = "deserialize_pubkey"
    )]
    pub owner: Pubkey,
    pub amount: u64,
    pub record: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LamportsUpdated {
    #[serde(
        serialize_with = "serialize_pubkey",
        deserialize_with = "deserialize_pubkey"
    )]
    pub owner: Pubkey,
    pub lamports: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivityRecorded {
    #[serde(
        serialize_with = "serialize_pubkey",
        deserialize_with = "deserialize_pubkey"
    )]
    pub owner: Pubkey,
    pub kind: u8,
    #[serde(
        serialize_with = "serialize_pubkey",
        deserialize_with = "deserialize_pubkey"
    )]
    pub counterparty: Pubkey,
    pub amount: u64,
}

fn serialize_pubkey<S>(pubkey: &Pubkey, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&pubkey.to_string())
}

fn deserialize_pubkey<'de, D>(deserializer: D) -> Result<Pubkey, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    value.parse().map_err(serde::de::Error::custom)
}

#[cfg(test)]
mod tests {
    use solana_signature::Signature;

    use super::{event_id, EventEnvelope, EventPayload, FollowCreated, SCHEMA_VERSION_V1};

    fn signature_with_byte(byte: u8) -> Signature {
        Signature::from([byte; 64])
    }

    #[test]
    fn event_id_uses_event_type_signature_and_instruction_index() {
        let signature = signature_with_byte(7);
        let id = event_id("follow.created", &signature, 3);
        assert_eq!(id, format!("follow.created:{signature}:3"));
    }

    #[test]
    fn envelope_serializes_payload_shape() {
        let follower = solana_pubkey::Pubkey::new_unique();
        let target = solana_pubkey::Pubkey::new_unique();
        let signature = signature_with_byte(9);
        let envelope = EventEnvelope::new(
            EventPayload::FollowCreated(FollowCreated { follower, target }),
            42,
            &signature,
            0,
            1_700_000_000,
        );

        let json = serde_json::to_value(&envelope).unwrap();
        assert_eq!(json["schema_version"], SCHEMA_VERSION_V1);
        assert_eq!(json["event_type"], "follow.created");
        assert_eq!(json["follower"], follower.to_string());
        assert_eq!(json["target"], target.to_string());
    }
}
