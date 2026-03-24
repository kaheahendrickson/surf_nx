use std::path::PathBuf;

use solana_pubkey::Pubkey;

const TOKEN_PROGRAM_BYTES: [u8; 32] = [
    1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

const REGISTRY_PROGRAM_BYTES: [u8; 32] = [
    2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

const SIGNALS_PROGRAM_BYTES: [u8; 32] = [
    3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

const TRACKED_ADDRESS_BYTES: [u8; 32] = [
    4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

pub fn token_program_id() -> Pubkey {
    Pubkey::new_from_array(TOKEN_PROGRAM_BYTES)
}

pub fn registry_program_id() -> Pubkey {
    Pubkey::new_from_array(REGISTRY_PROGRAM_BYTES)
}

pub fn signals_program_id() -> Pubkey {
    Pubkey::new_from_array(SIGNALS_PROGRAM_BYTES)
}

pub fn tracked_address() -> Pubkey {
    Pubkey::new_from_array(TRACKED_ADDRESS_BYTES)
}

#[derive(Debug, Clone)]
pub struct WebServicesConfig {
    pub rpc_port: u16,
    pub nats_port: u16,
    pub rpc_host: String,
    pub nats_host: String,
    pub stream_name: String,
    pub checkpoint_path: PathBuf,
    pub poll_interval_ms: u64,
    pub auto_build_sbf: bool,
}

impl Default for WebServicesConfig {
    fn default() -> Self {
        Self {
            rpc_port: 8899,
            nats_port: 4222,
            rpc_host: "127.0.0.1".to_string(),
            nats_host: "127.0.0.1".to_string(),
            stream_name: "surf-events".to_string(),
            checkpoint_path: std::env::temp_dir().join("surf-events-checkpoint.json"),
            poll_interval_ms: 1000,
            auto_build_sbf: true,
        }
    }
}

impl WebServicesConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_rpc_port(mut self, port: u16) -> Self {
        self.rpc_port = port;
        self
    }

    pub fn with_nats_port(mut self, port: u16) -> Self {
        self.nats_port = port;
        self
    }

    pub fn with_checkpoint_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.checkpoint_path = path.into();
        self
    }

    pub fn with_poll_interval_ms(mut self, interval_ms: u64) -> Self {
        self.poll_interval_ms = interval_ms;
        self
    }

    pub fn rpc_url(&self) -> String {
        format!("http://{}:{}", self.rpc_host, self.rpc_port)
    }

    pub fn nats_url(&self) -> String {
        format!("nats://{}:{}", self.nats_host, self.nats_port)
    }
}
