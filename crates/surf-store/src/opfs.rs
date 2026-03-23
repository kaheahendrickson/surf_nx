use crate::column_families::{is_valid_column_family, ALL_COLUMN_FAMILIES};
use crate::error::StoreError;
use crate::r#trait::KeyValueStore;
use js_sys::{Array, Error as JsError, Uint8Array};
use std::sync::Arc;
use std::sync::RwLock;
use wasm_bindgen::prelude::*;

const DEFAULT_NAMESPACE: &str = "surf-store";

#[wasm_bindgen(inline_js = r#"
async function getNamespaceRoot(namespace, create) {
  const root = await navigator.storage.getDirectory();
  if (!namespace) {
    return root;
  }

  return await root.getDirectoryHandle(namespace, { create });
}

export async function opfsEnsureDirectories(namespace, columnFamilies) {
  const namespaceRoot = await getNamespaceRoot(namespace, true);
  for (const cf of columnFamilies) {
    await namespaceRoot.getDirectoryHandle(cf, { create: true });
  }
}

export async function opfsReadFile(namespace, cf, filename) {
  try {
    const namespaceRoot = await getNamespaceRoot(namespace, false);
    const directory = await namespaceRoot.getDirectoryHandle(cf, { create: false });
    const handle = await directory.getFileHandle(filename, { create: false });
    const file = await handle.getFile();
    return new Uint8Array(await file.arrayBuffer());
  } catch (error) {
    if (error instanceof DOMException && error.name === 'NotFoundError') {
      return null;
    }
    throw error;
  }
}

export async function opfsWriteFile(namespace, cf, filename, data) {
  const namespaceRoot = await getNamespaceRoot(namespace, true);
  const directory = await namespaceRoot.getDirectoryHandle(cf, { create: true });
  const handle = await directory.getFileHandle(filename, { create: true });
  const writable = await handle.createWritable();

  try {
    await writable.write(data);
  } finally {
    await writable.close();
  }
}

export async function opfsDeleteFile(namespace, cf, filename) {
  try {
    const namespaceRoot = await getNamespaceRoot(namespace, false);
    const directory = await namespaceRoot.getDirectoryHandle(cf, { create: false });
    await directory.removeEntry(filename);
  } catch (error) {
    if (error instanceof DOMException && error.name === 'NotFoundError') {
      return;
    }
    throw error;
  }
}

export async function opfsListFiles(namespace, cf) {
  try {
    const namespaceRoot = await getNamespaceRoot(namespace, false);
    const directory = await namespaceRoot.getDirectoryHandle(cf, { create: false });
    const names = [];

    for await (const [name, handle] of directory.entries()) {
      if (handle.kind === 'file') {
        names.push(name);
      }
    }

    return names;
  } catch (error) {
    if (error instanceof DOMException && error.name === 'NotFoundError') {
      return [];
    }
    throw error;
  }
}

export async function opfsRemoveNamespace(namespace) {
  if (!namespace) {
    return;
  }

  const root = await navigator.storage.getDirectory();
  try {
    await root.removeEntry(namespace, { recursive: true });
  } catch (error) {
    if (error instanceof DOMException && error.name === 'NotFoundError') {
      return;
    }
    throw error;
  }
}
"#)]
extern "C" {
    #[wasm_bindgen(catch, js_name = opfsEnsureDirectories)]
    async fn opfs_ensure_directories(
        namespace: &str,
        column_families: Array,
    ) -> Result<(), JsValue>;

    #[wasm_bindgen(catch, js_name = opfsReadFile)]
    async fn opfs_read_file(namespace: &str, cf: &str, filename: &str) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch, js_name = opfsWriteFile)]
    async fn opfs_write_file(
        namespace: &str,
        cf: &str,
        filename: &str,
        data: &[u8],
    ) -> Result<(), JsValue>;

    #[wasm_bindgen(catch, js_name = opfsDeleteFile)]
    async fn opfs_delete_file(namespace: &str, cf: &str, filename: &str) -> Result<(), JsValue>;

    #[wasm_bindgen(catch, js_name = opfsListFiles)]
    async fn opfs_list_files(namespace: &str, cf: &str) -> Result<Array, JsValue>;

    #[wasm_bindgen(catch, js_name = opfsRemoveNamespace)]
    async fn opfs_remove_namespace(namespace: &str) -> Result<(), JsValue>;
}

fn key_to_filename(key: &[u8]) -> String {
    hex::encode(key)
}

fn filename_to_key(filename: &str) -> Result<Vec<u8>, StoreError> {
    hex::decode(filename).map_err(|e| StoreError::InvalidKey(e.to_string()))
}

fn js_error_message(error: JsValue) -> String {
    if let Some(message) = error.as_string() {
        return message;
    }

    if error.is_instance_of::<JsError>() {
        return JsError::from(error).message().into();
    }

    format!("{error:?}")
}

fn map_js_error(error: JsValue) -> StoreError {
    StoreError::opfs(js_error_message(error))
}

pub struct OpfsStore {
    namespace: Arc<String>,
    closed: Arc<RwLock<bool>>,
}

impl OpfsStore {
    pub async fn open() -> Result<Self, StoreError> {
        Self::open_with_namespace(DEFAULT_NAMESPACE).await
    }

    pub async fn open_with_namespace(namespace: impl Into<String>) -> Result<Self, StoreError> {
        let namespace = namespace.into();
        let column_families = Array::new();

        for cf in ALL_COLUMN_FAMILIES {
            column_families.push(&JsValue::from_str(cf));
        }

        opfs_ensure_directories(&namespace, column_families)
            .await
            .map_err(map_js_error)?;

        Ok(Self {
            namespace: Arc::new(namespace),
            closed: Arc::new(RwLock::new(false)),
        })
    }

    pub async fn remove_namespace(namespace: &str) -> Result<(), StoreError> {
        opfs_remove_namespace(namespace).await.map_err(map_js_error)
    }

    fn validate_cf(&self, cf: &str) -> Result<(), StoreError> {
        if !is_valid_column_family(cf) {
            return Err(StoreError::InvalidColumnFamily(cf.to_string()));
        }

        Ok(())
    }

    fn check_closed(&self) -> Result<(), StoreError> {
        let closed = self.closed.read().unwrap();
        if *closed {
            Err(StoreError::Closed)
        } else {
            Ok(())
        }
    }

    fn namespace(&self) -> &str {
        self.namespace.as_str()
    }
}

impl KeyValueStore for OpfsStore {
    async fn get(&self, cf: &str, key: &[u8]) -> Result<Option<Vec<u8>>, StoreError> {
        self.check_closed()?;
        self.validate_cf(cf)?;

        let filename = key_to_filename(key);
        let value = opfs_read_file(self.namespace(), cf, &filename)
            .await
            .map_err(map_js_error)?;

        if value.is_null() || value.is_undefined() {
            return Ok(None);
        }

        Ok(Some(Uint8Array::new(&value).to_vec()))
    }

    async fn set(&self, cf: &str, key: &[u8], value: &[u8]) -> Result<(), StoreError> {
        self.check_closed()?;
        self.validate_cf(cf)?;

        let filename = key_to_filename(key);
        opfs_write_file(self.namespace(), cf, &filename, value)
            .await
            .map_err(map_js_error)
    }

    async fn delete(&self, cf: &str, key: &[u8]) -> Result<(), StoreError> {
        self.check_closed()?;
        self.validate_cf(cf)?;

        let filename = key_to_filename(key);
        opfs_delete_file(self.namespace(), cf, &filename)
            .await
            .map_err(map_js_error)
    }

    async fn list_keys(&self, cf: &str) -> Result<Vec<Vec<u8>>, StoreError> {
        self.check_closed()?;
        self.validate_cf(cf)?;

        let file_names = opfs_list_files(self.namespace(), cf)
            .await
            .map_err(map_js_error)?;
        let mut keys = Vec::new();

        for entry in file_names.iter() {
            if let Some(file_name) = entry.as_string() {
                if let Ok(key) = filename_to_key(&file_name) {
                    keys.push(key);
                }
            }
        }

        Ok(keys)
    }

    async fn flush(&self) -> Result<(), StoreError> {
        self.check_closed()
    }

    async fn close(&self) -> Result<(), StoreError> {
        self.check_closed()?;
        let mut closed = self.closed.write().unwrap();
        *closed = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_hex_encoding() {
        let key = b"\x01\x02\xff";
        let filename = key_to_filename(key);
        assert_eq!(filename, "0102ff");
        let decoded = filename_to_key(&filename).unwrap();
        assert_eq!(decoded, key.to_vec());
    }
}
