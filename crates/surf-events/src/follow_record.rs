const FOLLOW_RECORD_LEN: usize = 8 + 8 + 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FollowRecord {
    pub slot: u64,
    pub block_time: i64,
    pub signature: [u8; 64],
}

#[derive(Debug, thiserror::Error)]
#[error("Invalid follow record")]
pub struct InvalidFollowRecord;

impl FollowRecord {
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(FOLLOW_RECORD_LEN);
        bytes.extend_from_slice(&self.slot.to_le_bytes());
        bytes.extend_from_slice(&self.block_time.to_le_bytes());
        bytes.extend_from_slice(&self.signature);
        bytes
    }

    pub fn decode(data: &[u8]) -> Result<Self, InvalidFollowRecord> {
        if data.len() != FOLLOW_RECORD_LEN {
            return Err(InvalidFollowRecord);
        }

        let slot = u64::from_le_bytes(data[0..8].try_into().map_err(|_| InvalidFollowRecord)?);
        let block_time =
            i64::from_le_bytes(data[8..16].try_into().map_err(|_| InvalidFollowRecord)?);
        let mut signature = [0u8; 64];
        signature.copy_from_slice(&data[16..80]);
        Ok(Self {
            slot,
            block_time,
            signature,
        })
    }
}
