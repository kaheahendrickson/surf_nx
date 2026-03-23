use crate::column_families::{is_valid_column_family, ALL_COLUMN_FAMILIES};
use crate::error::StoreError;
use crate::r#trait::KeyValueStore;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;

use tokio::sync::RwLock;

fn key_to_filename(key: &[u8]) -> String {
    hex::encode(key)
}

fn filename_to_key(filename: &str) -> Result<Vec<u8>, StoreError> {
    hex::decode(filename).map_err(|e| StoreError::InvalidKey(e.to_string()))
}

pub struct NativeStore {
    base_path: PathBuf,
    closed: Arc<RwLock<bool>>,
}

impl NativeStore {
    pub async fn open(path: impl Into<PathBuf>) -> Result<Self, StoreError> {
        let base_path = path.into();
        fs::create_dir_all(&base_path).await?;

        for cf in ALL_COLUMN_FAMILIES {
            fs::create_dir_all(base_path.join(cf)).await?;
        }

        Ok(Self {
            base_path,
            closed: Arc::new(RwLock::new(false)),
        })
    }

    fn cf_path(&self, cf: &str) -> Result<PathBuf, StoreError> {
        if !is_valid_column_family(cf) {
            return Err(StoreError::InvalidColumnFamily(cf.to_string()));
        }
        Ok(self.base_path.join(cf))
    }

    async fn check_closed(&self) -> Result<(), StoreError> {
        let closed = self.closed.read().await;
        if *closed {
            Err(StoreError::Closed)
        } else {
            Ok(())
        }
    }
}

impl KeyValueStore for NativeStore {
    async fn get(&self, cf: &str, key: &[u8]) -> Result<Option<Vec<u8>>, StoreError> {
        self.check_closed().await?;
        let path = self.cf_path(cf)?.join(key_to_filename(key));
        match fs::read(&path).await {
            Ok(data) => Ok(Some(data)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(StoreError::Io(e)),
        }
    }

    async fn set(&self, cf: &str, key: &[u8], value: &[u8]) -> Result<(), StoreError> {
        self.check_closed().await?;
        let path = self.cf_path(cf)?.join(key_to_filename(key));
        fs::write(&path, value).await?;
        Ok(())
    }

    async fn delete(&self, cf: &str, key: &[u8]) -> Result<(), StoreError> {
        self.check_closed().await?;
        let path = self.cf_path(cf)?.join(key_to_filename(key));
        match fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(StoreError::Io(e)),
        }
    }

    async fn list_keys(&self, cf: &str) -> Result<Vec<Vec<u8>>, StoreError> {
        self.check_closed().await?;
        let path = self.cf_path(cf)?;
        let mut entries = fs::read_dir(&path).await?;
        let mut keys = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            let filename = entry.file_name().to_string_lossy().into_owned();
            if let Ok(key) = filename_to_key(&filename) {
                keys.push(key);
            }
        }

        Ok(keys)
    }

    async fn flush(&self) -> Result<(), StoreError> {
        self.check_closed().await
    }

    async fn close(&self) -> Result<(), StoreError> {
        self.check_closed().await?;
        let mut closed = self.closed.write().await;
        *closed = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};
    use tempfile::TempDir;

    #[fixture]
    fn temp_dir() -> TempDir {
        tempfile::tempdir().unwrap()
    }

    #[rstest]
    #[tokio::test]
    async fn test_native_store_temp_dir(temp_dir: TempDir) {
        let store: NativeStore = NativeStore::open(temp_dir.path()).await.unwrap();
        store.set("names", b"alice", b"data").await.unwrap();
        let result = store.get("names", b"alice").await.unwrap();
        assert_eq!(result, Some(b"data".to_vec()));
    }

    #[rstest]
    #[tokio::test]
    async fn test_key_hex_encoding() {
        let key = b"\x01\x02\xff";
        let filename = key_to_filename(key);
        assert_eq!(filename, "0102ff");
        let decoded = filename_to_key(&filename).unwrap();
        assert_eq!(decoded, key.to_vec());
    }

    #[rstest]
    #[tokio::test]
    async fn test_persistence_across_open_close() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();

        {
            let store: NativeStore = NativeStore::open(&path).await.unwrap();
            store.set("names", b"alice", b"data").await.unwrap();
            store.close().await.unwrap();
        }

        {
            let store: NativeStore = NativeStore::open(&path).await.unwrap();
            let result = store.get("names", b"alice").await.unwrap();
            assert_eq!(result, Some(b"data".to_vec()));
        }
    }

    #[rstest]
    #[tokio::test]
    async fn test_close_prevents_operations(temp_dir: TempDir) {
        let store: NativeStore = NativeStore::open(temp_dir.path()).await.unwrap();
        store.close().await.unwrap();
        let result = store.get("names", b"key").await;
        assert!(matches!(result, Err(StoreError::Closed)));
    }
}
