//! Predefined column families for key namespacing.
//!
//! Column families provide logical separation of different data types within
//! the same store. Each column family is essentially a separate key-value
//! namespace.
//!
//! # Available Column Families
//!
//! | Constant | Name | Purpose |
//! |----------|------|---------|
//! | [`NAMES`] | `"names"` | Name records for the name registry |
//! | [`CHECKPOINTS`] | `"checkpoints"` | Sync state checkpoints |
//! | [`BALANCES`] | `"balances"` | Token balance caches |
//! | [`LAMPORTS`] | `"lamports"` | Native SOL balance caches |
//! | [`TRANSACTIONS`] | `"transactions"` | Curated activity records |
//! | [`FOLLOWS`] | `"follows"` | Active follow relationships |
//! | [`PROPOSALS`] | `"proposals"` | Governance proposals |
//! | [`METADATA`] | `"metadata"` | Configuration and metadata values |

/// Column family for name records.
///
/// Used to store name-to-owner mappings from the name registry.
pub const NAMES: &str = "names";

/// Column family for sync checkpoints.
///
/// Used to track the last processed slot for incremental syncing.
pub const CHECKPOINTS: &str = "checkpoints";

/// Column family for token balances.
///
/// Used to cache token balance data for accounts.
pub const BALANCES: &str = "balances";

/// Column family for native SOL balances in lamports.
///
/// Used to cache RPC getBalance values for tracked accounts.
pub const LAMPORTS: &str = "lamports";

/// Column family for curated transaction/activity records.
pub const TRANSACTIONS: &str = "transactions";

/// Column family for active follow relationships.
pub const FOLLOWS: &str = "follows";

/// Column family for governance proposals.
///
/// Used to store proposal data for the governance system.
pub const PROPOSALS: &str = "proposals";

/// Column family for metadata.
///
/// Used to store configuration and metadata values.
pub const METADATA: &str = "metadata";

/// All predefined column family names.
pub const ALL_COLUMN_FAMILIES: &[&str] = &[
    NAMES,
    CHECKPOINTS,
    BALANCES,
    LAMPORTS,
    TRANSACTIONS,
    FOLLOWS,
    PROPOSALS,
    METADATA,
];

/// Checks if a column family name is one of the predefined column families.
///
/// # Example
///
/// ```
/// use surf_store::is_valid_column_family;
///
/// assert!(is_valid_column_family("names"));
/// assert!(!is_valid_column_family("unknown"));
/// ```
pub fn is_valid_column_family(cf: &str) -> bool {
    ALL_COLUMN_FAMILIES.contains(&cf)
}
