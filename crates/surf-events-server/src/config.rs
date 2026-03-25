#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerConfig {
    pub rpc_url: String,
    pub nats_url: String,
    pub stream_name: String,
    pub checkpoint_path: std::path::PathBuf,
    pub tracked_address: Option<solana_pubkey::Pubkey>,
    pub token_program: solana_pubkey::Pubkey,
    pub registry_program: solana_pubkey::Pubkey,
    pub signals_program: solana_pubkey::Pubkey,
    pub poll_interval_ms: u64,
    pub signature_batch_limit: usize,
    pub transaction_history_limit: usize,
}

impl ServerConfig {
    pub fn new(
        rpc_url: impl Into<String>,
        nats_url: impl Into<String>,
        stream_name: impl Into<String>,
        tracked_address: Option<solana_pubkey::Pubkey>,
        token_program: solana_pubkey::Pubkey,
        registry_program: solana_pubkey::Pubkey,
        signals_program: solana_pubkey::Pubkey,
    ) -> Self {
        Self {
            rpc_url: rpc_url.into(),
            nats_url: nats_url.into(),
            stream_name: stream_name.into(),
            checkpoint_path: crate::checkpoint::default_checkpoint_path(),
            tracked_address,
            token_program,
            registry_program,
            signals_program,
            poll_interval_ms: 1_000,
            signature_batch_limit: 100,
            transaction_history_limit: 100,
        }
    }

    pub fn with_poll_interval_ms(mut self, poll_interval_ms: u64) -> Self {
        self.poll_interval_ms = poll_interval_ms;
        self
    }

    pub fn with_signature_batch_limit(mut self, signature_batch_limit: usize) -> Self {
        self.signature_batch_limit = signature_batch_limit;
        self
    }

    pub fn with_transaction_history_limit(mut self, transaction_history_limit: usize) -> Self {
        self.transaction_history_limit = transaction_history_limit;
        self
    }

    pub fn with_checkpoint_path(mut self, checkpoint_path: impl Into<std::path::PathBuf>) -> Self {
        self.checkpoint_path = checkpoint_path.into();
        self
    }
}
