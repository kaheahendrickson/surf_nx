use std::env;
use std::path::PathBuf;

use solana_pubkey::Pubkey;

pub fn token_program_id() -> Pubkey {
    env::var("SURF_TOKEN_PROGRAM_ID")
        .expect("SURF_TOKEN_PROGRAM_ID must be set in .env")
        .parse()
        .expect("SURF_TOKEN_PROGRAM_ID must be a valid Pubkey")
}

pub fn registry_program_id() -> Pubkey {
    env::var("SURF_REGISTRY_PROGRAM_ID")
        .expect("SURF_REGISTRY_PROGRAM_ID must be set in .env")
        .parse()
        .expect("SURF_REGISTRY_PROGRAM_ID must be a valid Pubkey")
}

pub fn signals_program_id() -> Pubkey {
    env::var("SURF_SIGNALS_PROGRAM_ID")
        .expect("SURF_SIGNALS_PROGRAM_ID must be set in .env")
        .parse()
        .expect("SURF_SIGNALS_PROGRAM_ID must be a valid Pubkey")
}

pub fn tracked_address() -> Pubkey {
    env::var("SURF_TRACKED_ADDRESS")
        .expect("SURF_TRACKED_ADDRESS must be set in .env")
        .parse()
        .expect("SURF_TRACKED_ADDRESS must be a valid Pubkey")
}

fn parse_url_host_and_port(url: &str, default_host: &str, default_port: u16) -> (String, u16) {
    url.strip_prefix("http://")
        .or_else(|| url.strip_prefix("nats://"))
        .or_else(|| url.strip_prefix("https://"))
        .and_then(|host_port| {
            if host_port.starts_with('[') {
                let end_bracket = host_port.find(']')?;
                let host = &host_port[..=end_bracket];
                let port: Option<u16> = host_port
                    .get(end_bracket + 2..)
                    .and_then(|p| p.parse().ok());
                Some((host.to_string(), port))
            } else if let Some(colon_pos) = host_port.rfind(':') {
                let host = host_port[..colon_pos].to_string();
                let port: Option<u16> = host_port[colon_pos + 1..].parse().ok();
                Some((host, port))
            } else {
                Some((host_port.to_string(), None))
            }
        })
        .map(|(host, port)| (host, port.unwrap_or(default_port)))
        .unwrap_or((default_host.to_string(), default_port))
}

fn rpc_url() -> String {
    env::var("SURF_TEST_VALIDATOR_URL").expect("SURF_TEST_VALIDATOR_URL must be set in .env")
}

fn nats_url() -> String {
    env::var("SURF_NATS_URL").expect("SURF_NATS_URL must be set in .env")
}

pub fn rpc_host() -> String {
    parse_url_host_and_port(&rpc_url(), "127.0.0.1", 8899).0
}

pub fn rpc_port() -> u16 {
    parse_url_host_and_port(&rpc_url(), "127.0.0.1", 8899).1
}

pub fn nats_host() -> String {
    parse_url_host_and_port(&nats_url(), "127.0.0.1", 4222).0
}

pub fn nats_port() -> u16 {
    parse_url_host_and_port(&nats_url(), "127.0.0.1", 4222).1
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
            rpc_port: rpc_port(),
            nats_port: nats_port(),
            rpc_host: rpc_host(),
            nats_host: nats_host(),
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
