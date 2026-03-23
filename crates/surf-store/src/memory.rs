use crate::column_families::{is_valid_column_family, ALL_COLUMN_FAMILIES};
use crate::error::StoreError;
use crate::r#trait::KeyValueStore;
use std::collections::HashMap;
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::RwLock;

#[cfg(target_arch = "wasm32")]
use std::sync::RwLock;

#[derive(Clone)]
pub struct MemoryStore {
    data: Arc<RwLock<HashMap<String, HashMap<Vec<u8>, Vec<u8>>>>>,
    closed: Arc<RwLock<bool>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        let mut data = HashMap::new();
        for cf in ALL_COLUMN_FAMILIES {
            data.insert(cf.to_string(), HashMap::new());
        }
        Self {
            data: Arc::new(RwLock::new(data)),
            closed: Arc::new(RwLock::new(false)),
        }
    }
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl KeyValueStore for MemoryStore {
    async fn get(&self, cf: &str, key: &[u8]) -> Result<Option<Vec<u8>>, StoreError> {
        {
            let closed = self.closed.read().await;
            if *closed {
                return Err(StoreError::Closed);
            }
        }
        if !is_valid_column_family(cf) {
            return Err(StoreError::InvalidColumnFamily(cf.to_string()));
        }
        let data = self.data.read().await;
        Ok(data.get(cf).and_then(|cf_data| cf_data.get(key).cloned()))
    }

    async fn set(&self, cf: &str, key: &[u8], value: &[u8]) -> Result<(), StoreError> {
        {
            let closed = self.closed.read().await;
            if *closed {
                return Err(StoreError::Closed);
            }
        }
        if !is_valid_column_family(cf) {
            return Err(StoreError::InvalidColumnFamily(cf.to_string()));
        }
        let mut data = self.data.write().await;
        if let Some(cf_data) = data.get_mut(cf) {
            cf_data.insert(key.to_vec(), value.to_vec());
        }
        Ok(())
    }

    async fn delete(&self, cf: &str, key: &[u8]) -> Result<(), StoreError> {
        {
            let closed = self.closed.read().await;
            if *closed {
                return Err(StoreError::Closed);
            }
        }
        if !is_valid_column_family(cf) {
            return Err(StoreError::InvalidColumnFamily(cf.to_string()));
        }
        let mut data = self.data.write().await;
        if let Some(cf_data) = data.get_mut(cf) {
            cf_data.remove(key);
        }
        Ok(())
    }

    async fn list_keys(&self, cf: &str) -> Result<Vec<Vec<u8>>, StoreError> {
        {
            let closed = self.closed.read().await;
            if *closed {
                return Err(StoreError::Closed);
            }
        }
        if !is_valid_column_family(cf) {
            return Err(StoreError::InvalidColumnFamily(cf.to_string()));
        }
        let data = self.data.read().await;
        Ok(data
            .get(cf)
            .map(|cf_data| cf_data.keys().cloned().collect())
            .unwrap_or_default())
    }

    async fn flush(&self) -> Result<(), StoreError> {
        let closed = self.closed.read().await;
        if *closed {
            Err(StoreError::Closed)
        } else {
            Ok(())
        }
    }

    async fn close(&self) -> Result<(), StoreError> {
        {
            let closed = self.closed.read().await;
            if *closed {
                return Err(StoreError::Closed);
            }
        }
        let mut closed = self.closed.write().await;
        *closed = true;
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
impl KeyValueStore for MemoryStore {
    async fn get(&self, cf: &str, key: &[u8]) -> Result<Option<Vec<u8>>, StoreError> {
        let closed = self.closed.read().unwrap();
        if *closed {
            return Err(StoreError::Closed);
        }
        drop(closed);

        if !is_valid_column_family(cf) {
            return Err(StoreError::InvalidColumnFamily(cf.to_string()));
        }
        let data = self.data.read().unwrap();
        Ok(data.get(cf).and_then(|cf_data| cf_data.get(key).cloned()))
    }

    async fn set(&self, cf: &str, key: &[u8], value: &[u8]) -> Result<(), StoreError> {
        let closed = self.closed.read().unwrap();
        if *closed {
            return Err(StoreError::Closed);
        }
        drop(closed);

        if !is_valid_column_family(cf) {
            return Err(StoreError::InvalidColumnFamily(cf.to_string()));
        }
        let mut data = self.data.write().unwrap();
        if let Some(cf_data) = data.get_mut(cf) {
            cf_data.insert(key.to_vec(), value.to_vec());
        }
        Ok(())
    }

    async fn delete(&self, cf: &str, key: &[u8]) -> Result<(), StoreError> {
        let closed = self.closed.read().unwrap();
        if *closed {
            return Err(StoreError::Closed);
        }
        drop(closed);

        if !is_valid_column_family(cf) {
            return Err(StoreError::InvalidColumnFamily(cf.to_string()));
        }
        let mut data = self.data.write().unwrap();
        if let Some(cf_data) = data.get_mut(cf) {
            cf_data.remove(key);
        }
        Ok(())
    }

    async fn list_keys(&self, cf: &str) -> Result<Vec<Vec<u8>>, StoreError> {
        let closed = self.closed.read().unwrap();
        if *closed {
            return Err(StoreError::Closed);
        }
        drop(closed);

        if !is_valid_column_family(cf) {
            return Err(StoreError::InvalidColumnFamily(cf.to_string()));
        }
        let data = self.data.read().unwrap();
        Ok(data
            .get(cf)
            .map(|cf_data| cf_data.keys().cloned().collect())
            .unwrap_or_default())
    }

    async fn flush(&self) -> Result<(), StoreError> {
        let closed = self.closed.read().unwrap();
        if *closed {
            Err(StoreError::Closed)
        } else {
            Ok(())
        }
    }

    async fn close(&self) -> Result<(), StoreError> {
        {
            let closed = self.closed.read().unwrap();
            if *closed {
                return Err(StoreError::Closed);
            }
        }
        let mut closed = self.closed.write().unwrap();
        *closed = true;
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};

    #[fixture]
    fn store() -> MemoryStore {
        MemoryStore::new()
    }

    #[rstest]
    #[tokio::test]
    async fn test_set_and_get(store: MemoryStore) {
        store.set("names", b"alice", b"data").await.unwrap();
        let result = store.get("names", b"alice").await.unwrap();
        assert_eq!(result, Some(b"data".to_vec()));
    }

    #[rstest]
    #[tokio::test]
    async fn test_get_non_existent(store: MemoryStore) {
        let result = store.get("names", b"nonexistent").await.unwrap();
        assert_eq!(result, None);
    }

    #[rstest]
    #[tokio::test]
    async fn test_delete_existing(store: MemoryStore) {
        store.set("names", b"alice", b"data").await.unwrap();
        store.delete("names", b"alice").await.unwrap();
        let result = store.get("names", b"alice").await.unwrap();
        assert_eq!(result, None);
    }

    #[rstest]
    #[tokio::test]
    async fn test_delete_non_existent(store: MemoryStore) {
        let result = store.delete("names", b"nonexistent").await;
        assert!(result.is_ok());
    }

    #[rstest]
    #[tokio::test]
    async fn test_list_keys(store: MemoryStore) {
        store.set("names", b"alice", b"data1").await.unwrap();
        store.set("names", b"bob", b"data2").await.unwrap();
        let keys = store.list_keys("names").await.unwrap();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&b"alice".to_vec()));
        assert!(keys.contains(&b"bob".to_vec()));
    }

    #[rstest]
    #[tokio::test]
    async fn test_column_family_isolation(store: MemoryStore) {
        store.set("names", b"key", b"names_value").await.unwrap();
        store
            .set("balances", b"key", b"balances_value")
            .await
            .unwrap();
        let names_result = store.get("names", b"key").await.unwrap();
        let balances_result = store.get("balances", b"key").await.unwrap();
        assert_eq!(names_result, Some(b"names_value".to_vec()));
        assert_eq!(balances_result, Some(b"balances_value".to_vec()));
    }

    #[rstest]
    #[tokio::test]
    async fn test_invalid_column_family(store: MemoryStore) {
        let result = store.get("invalid_cf", b"key").await;
        assert!(matches!(result, Err(StoreError::InvalidColumnFamily(_))));
    }

    #[rstest]
    #[tokio::test]
    async fn test_close_prevents_operations(store: MemoryStore) {
        store.close().await.unwrap();
        let result = store.get("names", b"key").await;
        assert!(matches!(result, Err(StoreError::Closed)));
    }
}
