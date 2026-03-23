//! Configuration for the sync service.

use solana_pubkey::Pubkey;

use crate::error::SyncError;

/// Default poll interval in milliseconds.
pub const DEFAULT_POLL_INTERVAL_MS: u64 = 1000;

/// Configuration for the sync service.
#[derive(Debug, Clone)]
pub struct SyncConfig {
    /// Token program ID to sync balances from.
    pub token_program: Pubkey,
    /// Name registry program ID to sync names from.
    pub registry_program: Pubkey,
    /// Signals program ID to sync follow relationships from.
    pub signals_program: Pubkey,
    /// Single pubkey whose balance should be tracked.
    pub tracked_balance: Pubkey,
    /// Maximum recent owner transactions to inspect.
    pub transaction_history_limit: usize,
    /// Poll interval in milliseconds for balance updates.
    pub poll_interval_ms: u64,
    /// Optional JetStream event streaming configuration.
    pub event_stream: Option<EventStreamConfig>,
}

/// Configuration for consuming Surf event streams from JetStream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventStreamConfig {
    pub nats_url: String,
    pub stream_name: String,
    pub consumer_name: String,
    pub batch_size: usize,
}

/// Default recent transaction history window.
pub const DEFAULT_TRANSACTION_HISTORY_LIMIT: usize = 100;
pub const DEFAULT_EVENT_STREAM_BATCH_SIZE: usize = 128;

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            token_program: Pubkey::default(),
            registry_program: Pubkey::default(),
            signals_program: Pubkey::default(),
            tracked_balance: Pubkey::default(),
            transaction_history_limit: DEFAULT_TRANSACTION_HISTORY_LIMIT,
            poll_interval_ms: DEFAULT_POLL_INTERVAL_MS,
            event_stream: None,
        }
    }
}

impl EventStreamConfig {
    pub fn new(
        nats_url: impl Into<String>,
        stream_name: impl Into<String>,
        consumer_name: impl Into<String>,
    ) -> Self {
        Self {
            nats_url: nats_url.into(),
            stream_name: stream_name.into(),
            consumer_name: consumer_name.into(),
            batch_size: DEFAULT_EVENT_STREAM_BATCH_SIZE,
        }
    }

    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size;
        self
    }

    pub fn validate(&self) -> Result<(), SyncError> {
        if self.nats_url.trim().is_empty() {
            return Err(SyncError::InvalidConfig(
                "event stream nats_url cannot be empty".to_string(),
            ));
        }
        if self.stream_name.trim().is_empty() {
            return Err(SyncError::InvalidConfig(
                "event stream stream_name cannot be empty".to_string(),
            ));
        }
        if self.consumer_name.trim().is_empty() {
            return Err(SyncError::InvalidConfig(
                "event stream consumer_name cannot be empty".to_string(),
            ));
        }
        if self.batch_size == 0 {
            return Err(SyncError::InvalidConfig(
                "event stream batch_size cannot be zero".to_string(),
            ));
        }
        Ok(())
    }
}

impl SyncConfig {
    /// Creates a new sync configuration with the given programs.
    pub fn new(
        token_program: Pubkey,
        registry_program: Pubkey,
        signals_program: Pubkey,
        tracked_balance: Pubkey,
    ) -> Self {
        Self {
            token_program,
            registry_program,
            signals_program,
            tracked_balance,
            transaction_history_limit: DEFAULT_TRANSACTION_HISTORY_LIMIT,
            poll_interval_ms: DEFAULT_POLL_INTERVAL_MS,
            event_stream: None,
        }
    }

    /// Sets the recent transaction history limit.
    pub fn with_transaction_history_limit(mut self, limit: usize) -> Self {
        self.transaction_history_limit = limit;
        self
    }

    /// Sets the poll interval in milliseconds.
    pub fn with_poll_interval(mut self, ms: u64) -> Self {
        self.poll_interval_ms = ms;
        self
    }

    pub fn with_event_stream(mut self, event_stream: EventStreamConfig) -> Self {
        self.event_stream = Some(event_stream);
        self
    }

    /// Validates the configuration.
    pub fn validate(&self) -> Result<(), SyncError> {
        if self.token_program == Pubkey::default() {
            return Err(SyncError::InvalidConfig(
                "token_program cannot be default pubkey".to_string(),
            ));
        }
        if self.registry_program == Pubkey::default() {
            return Err(SyncError::InvalidConfig(
                "registry_program cannot be default pubkey".to_string(),
            ));
        }
        if self.tracked_balance == Pubkey::default() {
            return Err(SyncError::InvalidConfig(
                "tracked_balance cannot be default pubkey".to_string(),
            ));
        }
        if self.signals_program == Pubkey::default() {
            return Err(SyncError::InvalidConfig(
                "signals_program cannot be default pubkey".to_string(),
            ));
        }
        if self.transaction_history_limit == 0 {
            return Err(SyncError::InvalidConfig(
                "transaction_history_limit cannot be zero".to_string(),
            ));
        }
        if self.poll_interval_ms == 0 {
            return Err(SyncError::InvalidConfig(
                "poll_interval_ms cannot be zero".to_string(),
            ));
        }
        if let Some(event_stream) = &self.event_stream {
            event_stream.validate()?;
        }
        Ok(())
    }
}
