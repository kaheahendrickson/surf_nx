use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::ServerError;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SignatureCursor {
    pub last_slot: u64,
    pub signatures_at_slot: Vec<String>,
}

impl SignatureCursor {
    pub fn should_process(&self, signature: &solana_signature::Signature, slot: u64) -> bool {
        slot > self.last_slot
            || (slot == self.last_slot
                && !self
                    .signatures_at_slot
                    .iter()
                    .any(|value| value == &signature.to_string()))
    }

    pub fn advance<I>(&mut self, entries: I)
    where
        I: IntoIterator<Item = (solana_signature::Signature, u64)>,
    {
        for (signature, slot) in entries {
            let encoded = signature.to_string();
            if slot > self.last_slot {
                self.last_slot = slot;
                self.signatures_at_slot = vec![encoded];
            } else if slot == self.last_slot
                && !self
                    .signatures_at_slot
                    .iter()
                    .any(|value| value == &encoded)
            {
                self.signatures_at_slot.push(encoded);
            }
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BalanceSnapshot {
    pub amount: Option<u64>,
    pub lamports: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerCheckpointState {
    pub bootstrapped: bool,
    pub follow: SignatureCursor,
    pub names: SignatureCursor,
    pub tokens: SignatureCursor,
    pub activity: SignatureCursor,
}

pub fn load_checkpoint(path: &Path) -> Result<ServerCheckpointState, ServerError> {
    match std::fs::read(path) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).map_err(|err| ServerError::Checkpoint(err.to_string()))
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            Ok(ServerCheckpointState::default())
        }
        Err(err) => Err(ServerError::Checkpoint(err.to_string())),
    }
}

pub fn save_checkpoint(path: &Path, checkpoint: &ServerCheckpointState) -> Result<(), ServerError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| ServerError::Checkpoint(err.to_string()))?;
    }
    let data = serde_json::to_vec_pretty(checkpoint)
        .map_err(|err| ServerError::Checkpoint(err.to_string()))?;
    std::fs::write(path, data).map_err(|err| ServerError::Checkpoint(err.to_string()))
}

pub fn default_checkpoint_path() -> PathBuf {
    PathBuf::from(".surf-events-server/checkpoints.json")
}
