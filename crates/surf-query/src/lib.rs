mod error;

pub use error::QueryError;

use std::collections::HashMap;
use std::str;

use solana_pubkey::Pubkey;
// TODO: Uncomment when surf-sync is added
// use surf_sync::{ActivityKind, ActivityRecord, FollowRecord};
use surf_protocol::decode_name_record;
use surf_store::{KeyValueStore, NAMES};

// TODO: Uncomment when surf-sync is added
// #[derive(Debug, Clone, PartialEq, Eq)]
// pub struct ActivityView {
//     pub signature: String,
//     pub kind: String,
//     pub counterparty: String,
//     pub counterparty_name: Option<String>,
//     pub amount: u64,
//     pub slot: u64,
//     pub block_time: Option<i64>,
// }

// TODO: Uncomment when surf-sync is added
// #[derive(Debug, Clone, PartialEq, Eq)]
// pub struct FollowingView {
//     pub target: String,
//     pub target_name: Option<String>,
//     pub slot: u64,
//     pub block_time: Option<i64>,
//     pub signature: String,
// }

pub async fn get_names<S: KeyValueStore>(store: &S) -> Result<Vec<String>, QueryError> {
    let keys = store.list_keys(NAMES).await?;
    let mut names = Vec::with_capacity(keys.len());

    for key in keys {
        let Some(raw_record) = store.get(NAMES, &key).await? else {
            continue;
        };

        let record = decode_name_record(&raw_record).ok_or(QueryError::InvalidAccountData)?;
        let name_len = record.len as usize;

        if name_len > record.name.len() {
            return Err(QueryError::InvalidNameLength(record.len));
        }

        let name = str::from_utf8(&record.name[..name_len])
            .map_err(|_| QueryError::InvalidUtf8)?
            .to_owned();

        names.push(name);
    }

    names.sort();
    Ok(names)
}

// TODO: Uncomment when surf-sync is added
// pub async fn get_transactions<S: KeyValueStore>(store: &S) -> Result<Vec<ActivityView>, QueryError> {
//     let owner_names = get_owner_name_map(store).await?;
//     let keys = store.list_keys(TRANSACTIONS).await?;
//     let mut activities = Vec::with_capacity(keys.len());
//
//     for key in keys {
//         let Some(raw) = store.get(TRANSACTIONS, &key).await? else {
//             continue;
//         };
//
//         let record = ActivityRecord::decode(&raw).map_err(|_| QueryError::InvalidAccountData)?;
//         let counterparty_name = owner_names.get(&record.counterparty).cloned();
//         let signature = Signature::try_from(record.signature.as_slice())
//             .map_err(|_| QueryError::InvalidAccountData)?;
//
//         activities.push(ActivityView {
//             signature: signature.to_string(),
//             kind: activity_kind_label(record.kind).to_string(),
//             counterparty: record.counterparty.to_string(),
//             counterparty_name,
//             amount: record.amount,
//             slot: record.slot,
//             block_time: normalize_block_time(record.block_time),
//         });
//     }
//
//     activities.sort_by(|left, right| {
//         right
//             .slot
//             .cmp(&left.slot)
//             .then_with(|| right.signature.cmp(&left.signature))
//     });
//     Ok(activities)
// }

// TODO: Uncomment when surf-sync is added
// pub async fn get_following<S: KeyValueStore>(store: &S) -> Result<Vec<FollowingView>, QueryError> {
//     let owner_names = get_owner_name_map(store).await?;
//     let keys = store.list_keys(FOLLOWS).await?;
//     let mut follows = Vec::with_capacity(keys.len());
//
//     for key in keys {
//         let Some(raw) = store.get(FOLLOWS, &key).await? else {
//             continue;
//         };
//
//         let target = Pubkey::try_from(key.as_slice()).map_err(|_| QueryError::InvalidAccountData)?;
//         let record = FollowRecord::decode(&raw).map_err(|_| QueryError::InvalidAccountData)?;
//         let signature = Signature::try_from(record.signature.as_slice())
//             .map_err(|_| QueryError::InvalidAccountData)?;
//
//         follows.push(FollowingView {
//             target: target.to_string(),
//             target_name: owner_names.get(&target).cloned(),
//             slot: record.slot,
//             block_time: normalize_block_time(record.block_time),
//             signature: signature.to_string(),
//         });
//     }
//
//     follows.sort_by(|left, right| {
//         left.target_name
//             .cmp(&right.target_name)
//             .then_with(|| left.target.cmp(&right.target))
//     });
//     Ok(follows)
// }

#[allow(dead_code)]
async fn get_owner_name_map<S: KeyValueStore>(store: &S) -> Result<HashMap<Pubkey, String>, QueryError> {
    let keys = store.list_keys(NAMES).await?;
    let mut names = HashMap::with_capacity(keys.len());

    for key in keys {
        let Some(raw_record) = store.get(NAMES, &key).await? else {
            continue;
        };

        let record = decode_name_record(&raw_record).ok_or(QueryError::InvalidAccountData)?;
        let name_len = record.len as usize;

        if name_len > record.name.len() {
            return Err(QueryError::InvalidNameLength(record.len));
        }

        let name = str::from_utf8(&record.name[..name_len])
            .map_err(|_| QueryError::InvalidUtf8)?
            .to_owned();
        names.insert(record.owner, name);
    }

    Ok(names)
}

// TODO: Uncomment when surf-sync is added
// fn activity_kind_label(kind: ActivityKind) -> &'static str {
//     match kind {
//         ActivityKind::SolSent => "sol_sent",
//         ActivityKind::SolReceived => "sol_received",
//         ActivityKind::SurfSent => "surf_sent",
//         ActivityKind::SurfReceived => "surf_received",
//         ActivityKind::NameRegistered => "name_registered",
//         ActivityKind::Followed => "followed",
//         ActivityKind::Unfollowed => "unfollowed",
//     }
// }

#[allow(dead_code)]
fn normalize_block_time(value: i64) -> Option<i64> {
    if value < 0 {
        None
    } else {
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};
    use solana_pubkey::Pubkey;
    use surf_store::MemoryStore;

    fn encode_name_record(name: &[u8], len: u8) -> Vec<u8> {
        let mut data = vec![0u8; surf_protocol::NameRecord::LEN];
        let owner = Pubkey::new_unique();
        data[0..32].copy_from_slice(owner.as_ref());

        let copy_len = name.len().min(32);
        data[32..32 + copy_len].copy_from_slice(&name[..copy_len]);
        data[64] = len;
        data
    }

    #[fixture]
    fn store() -> MemoryStore {
        MemoryStore::new()
    }

    #[rstest]
    #[tokio::test]
    async fn test_get_names_empty(store: MemoryStore) {
        let names = get_names(&store).await.unwrap();
        assert!(names.is_empty());
    }

    #[rstest]
    #[tokio::test]
    async fn test_get_names_single(store: MemoryStore) {
        store
            .set(NAMES, b"alice", &encode_name_record(b"alice", 5))
            .await
            .unwrap();

        let names = get_names(&store).await.unwrap();
        assert_eq!(names, vec!["alice".to_string()]);
    }

    #[rstest]
    #[tokio::test]
    async fn test_get_names_sorted(store: MemoryStore) {
        store
            .set(NAMES, b"charlie", &encode_name_record(b"charlie", 7))
            .await
            .unwrap();
        store
            .set(NAMES, b"alice", &encode_name_record(b"alice", 5))
            .await
            .unwrap();
        store
            .set(NAMES, b"bob", &encode_name_record(b"bob", 3))
            .await
            .unwrap();

        let names = get_names(&store).await.unwrap();
        assert_eq!(names, vec!["alice", "bob", "charlie"]);
    }

    #[rstest]
    #[tokio::test]
    async fn test_get_names_invalid_account_data(store: MemoryStore) {
        store.set(NAMES, b"alice", b"bad").await.unwrap();

        let error = get_names(&store).await.unwrap_err();
        assert!(matches!(error, QueryError::InvalidAccountData));
    }

    #[rstest]
    #[tokio::test]
    async fn test_get_names_invalid_name_length(store: MemoryStore) {
        store
            .set(NAMES, b"alice", &encode_name_record(b"alice", 33))
            .await
            .unwrap();

        let error = get_names(&store).await.unwrap_err();
        assert!(matches!(error, QueryError::InvalidNameLength(33)));
    }

    #[rstest]
    #[tokio::test]
    async fn test_get_names_invalid_utf8(store: MemoryStore) {
        let invalid_utf8 = [0xff, 0xfe, 0xfd];
        store
            .set(NAMES, b"bad", &encode_name_record(&invalid_utf8, 3))
            .await
            .unwrap();

        let error = get_names(&store).await.unwrap_err();
        assert!(matches!(error, QueryError::InvalidUtf8));
    }
}
