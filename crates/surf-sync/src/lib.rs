//! # surf-sync
//!
//! Data synchronization service for Solana programs.
//!
//! This crate provides isomorphic sync capabilities for both native (tokio)
//! and browser/WASM environments.
//!
//! ## Overview
//!
//! - `SyncService` - Orchestrates bootstrap and streaming sync phases
//! - `NameSyncer` - Synchronizes name registry accounts
//! - `BalanceSyncer` - Synchronizes single pubkey token balance
//!
//! ## Platform Support
//!
//! | Platform | Sleep Provider | Notes |
//! |----------|----------------|-------|
//! | Native   | `TokioSleep`   | Uses tokio::time::sleep |
//! | WASM     | `WasmSleep`    | Uses gloo timers |

pub mod activity_syncer;
pub mod activity_event_syncer;
pub mod balance_syncer;
pub mod balance_event_syncer;
pub mod checkpoint;
pub mod config;
pub mod error;
pub mod follow_event_syncer;
pub mod follow_syncer;
pub mod name_syncer;
pub mod name_event_syncer;
pub mod parser;
pub mod sleep;
pub mod sync_service;

pub use activity_syncer::{ActivityRecord, ActivitySyncer};
pub use balance_syncer::BalanceSyncer;
pub use checkpoint::{SyncCheckpoint, SyncServiceState, SyncState};
pub use config::{EventStreamConfig, SyncConfig};
pub use error::SyncError;
pub use activity_event_syncer::ActivityEventSyncer;
pub use balance_event_syncer::BalanceEventSyncer;
pub use follow_event_syncer::FollowEventSyncer;
pub use follow_syncer::{FollowRecord, FollowSyncer};
pub use name_event_syncer::NameEventSyncer;
pub use parser::ActivityKind;
pub use name_syncer::NameSyncer;
pub use sync_service::SyncService;

#[cfg(not(target_arch = "wasm32"))]
pub use sleep::TokioSleep;

#[cfg(target_arch = "wasm32")]
pub use sleep::WasmSleep;

pub use sleep::SleepProvider;
