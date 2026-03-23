use solana_pubkey::Pubkey;

use crate::activity_kind::ActivityKind;

const ACTIVITY_RECORD_LEN: usize = 1 + 32 + 8 + 8 + 8 + 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityRecord {
    pub kind: ActivityKind,
    pub counterparty: Pubkey,
    pub amount: u64,
    pub slot: u64,
    pub block_time: i64,
    pub signature: [u8; 64],
}

#[derive(Debug, thiserror::Error)]
#[error("Invalid activity record")]
pub struct InvalidActivityRecord;

impl ActivityRecord {
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(ACTIVITY_RECORD_LEN);
        bytes.push(self.kind.as_u8());
        bytes.extend_from_slice(self.counterparty.as_ref());
        bytes.extend_from_slice(&self.amount.to_le_bytes());
        bytes.extend_from_slice(&self.slot.to_le_bytes());
        bytes.extend_from_slice(&self.block_time.to_le_bytes());
        bytes.extend_from_slice(&self.signature);
        bytes
    }

    pub fn decode(data: &[u8]) -> Result<Self, InvalidActivityRecord> {
        if data.len() != ACTIVITY_RECORD_LEN {
            return Err(InvalidActivityRecord);
        }

        let kind = ActivityKind::from_u8(data[0]).map_err(|_| InvalidActivityRecord)?;
        let counterparty = Pubkey::try_from(&data[1..33]).map_err(|_| InvalidActivityRecord)?;
        let amount =
            u64::from_le_bytes(data[33..41].try_into().map_err(|_| InvalidActivityRecord)?);
        let slot = u64::from_le_bytes(data[41..49].try_into().map_err(|_| InvalidActivityRecord)?);
        let block_time =
            i64::from_le_bytes(data[49..57].try_into().map_err(|_| InvalidActivityRecord)?);
        let mut signature = [0u8; 64];
        signature.copy_from_slice(&data[57..121]);

        Ok(Self {
            kind,
            counterparty,
            amount,
            slot,
            block_time,
            signature,
        })
    }
}
