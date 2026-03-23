#![cfg(target_arch = "wasm32")]

use crate::{KeyValueStore, OpfsStore, StoreError, NAMES};
use wasm_bindgen::prelude::*;

fn js_error(message: impl Into<String>) -> JsValue {
    JsValue::from_str(&message.into())
}

fn ensure(condition: bool, message: impl Into<String>) -> Result<(), JsValue> {
    if condition {
        Ok(())
    } else {
        Err(js_error(message))
    }
}

fn map_store_error(context: &str, error: StoreError) -> JsValue {
    js_error(format!("{context}: {error}"))
}

fn sorted(mut keys: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    keys.sort();
    keys
}

#[wasm_bindgen]
pub async fn run_surf_store_browser_integration_tests(namespace: String) -> Result<(), JsValue> {
    OpfsStore::remove_namespace(&namespace)
        .await
        .map_err(|err| map_store_error("cleanup before test failed", err))?;

    let store = OpfsStore::open_with_namespace(namespace.clone())
        .await
        .map_err(|err| map_store_error("open failed", err))?;

    let missing = store
        .get(NAMES, b"missing")
        .await
        .map_err(|err| map_store_error("initial get failed", err))?;
    ensure(missing.is_none(), "expected missing key to return None")?;

    store
        .set(NAMES, b"alice", b"first")
        .await
        .map_err(|err| map_store_error("set alice failed", err))?;
    let alice = store
        .get(NAMES, b"alice")
        .await
        .map_err(|err| map_store_error("get alice failed", err))?;
    ensure(
        alice == Some(b"first".to_vec()),
        "expected roundtrip value for alice",
    )?;

    store
        .set(NAMES, b"alice", b"updated")
        .await
        .map_err(|err| map_store_error("overwrite alice failed", err))?;
    let updated = store
        .get(NAMES, b"alice")
        .await
        .map_err(|err| map_store_error("get updated alice failed", err))?;
    ensure(
        updated == Some(b"updated".to_vec()),
        "expected overwrite to replace existing value",
    )?;

    let binary_key = [0x00_u8, 0x7f, 0xfe];
    let binary_value = [0xde_u8, 0xad, 0xbe, 0xef];
    store
        .set(NAMES, &binary_key, &binary_value)
        .await
        .map_err(|err| map_store_error("set binary key failed", err))?;
    let binary = store
        .get(NAMES, &binary_key)
        .await
        .map_err(|err| map_store_error("get binary key failed", err))?;
    ensure(
        binary == Some(binary_value.to_vec()),
        "expected binary key/value roundtrip",
    )?;

    let keys = sorted(
        store
            .list_keys(NAMES)
            .await
            .map_err(|err| map_store_error("list keys failed", err))?,
    );
    ensure(
        keys == vec![binary_key.to_vec(), b"alice".to_vec()],
        format!("unexpected keys returned: {keys:?}"),
    )?;

    store
        .delete(NAMES, b"alice")
        .await
        .map_err(|err| map_store_error("delete alice failed", err))?;
    let deleted = store
        .get(NAMES, b"alice")
        .await
        .map_err(|err| map_store_error("get deleted alice failed", err))?;
    ensure(deleted.is_none(), "expected deleted key to be missing")?;

    store
        .delete(NAMES, b"alice")
        .await
        .map_err(|err| map_store_error("delete missing key failed", err))?;

    let invalid = store.get("not-a-cf", b"k").await;
    ensure(
        matches!(invalid, Err(StoreError::InvalidColumnFamily(_))),
        format!("expected invalid column family error, got {invalid:?}"),
    )?;

    store
        .close()
        .await
        .map_err(|err| map_store_error("close failed", err))?;
    let closed = store.get(NAMES, b"after-close").await;
    ensure(
        matches!(closed, Err(StoreError::Closed)),
        format!("expected closed store error, got {closed:?}"),
    )?;

    let reopened = OpfsStore::open_with_namespace(namespace.clone())
        .await
        .map_err(|err| map_store_error("reopen failed", err))?;
    let persisted = reopened
        .get(NAMES, &binary_key)
        .await
        .map_err(|err| map_store_error("get persisted binary key failed", err))?;
    ensure(
        persisted == Some(binary_value.to_vec()),
        "expected data to persist across reopen",
    )?;

    let reopened_keys = reopened
        .list_keys(NAMES)
        .await
        .map_err(|err| map_store_error("list keys after reopen failed", err))?;
    ensure(
        reopened_keys == vec![binary_key.to_vec()],
        format!("expected only binary key after reopen, got {reopened_keys:?}"),
    )?;

    reopened
        .close()
        .await
        .map_err(|err| map_store_error("close reopened store failed", err))?;

    OpfsStore::remove_namespace(&namespace)
        .await
        .map_err(|err| map_store_error("cleanup after test failed", err))?;

    Ok(())
}
