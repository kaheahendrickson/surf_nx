#![cfg(target_arch = "wasm32")]

#[cfg(test)]
mod tests {
    use crate::{KeyValueStore, OpfsStore, StoreError, NAMES};

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_worker);

    fn sorted(mut keys: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
        keys.sort();
        keys
    }

    #[wasm_bindgen_test::wasm_bindgen_test]
    async fn integration_tests() {
        let namespace = format!("test-{}", js_sys::Date::now());

        OpfsStore::remove_namespace(&namespace)
            .await
            .expect("cleanup before test failed");

        let store = OpfsStore::open_with_namespace(namespace.clone())
            .await
            .expect("open failed");

        let missing = store.get(NAMES, b"missing").await.expect("initial get failed");
        assert!(missing.is_none(), "expected missing key to return None");

        store
            .set(NAMES, b"alice", b"first")
            .await
            .expect("set alice failed");
        let alice = store.get(NAMES, b"alice").await.expect("get alice failed");
        assert_eq!(alice, Some(b"first".to_vec()), "expected roundtrip value for alice");

        store
            .set(NAMES, b"alice", b"updated")
            .await
            .expect("overwrite alice failed");
        let updated = store.get(NAMES, b"alice").await.expect("get updated alice failed");
        assert_eq!(
            updated,
            Some(b"updated".to_vec()),
            "expected overwrite to replace existing value"
        );

        let binary_key = [0x00_u8, 0x7f, 0xfe];
        let binary_value = [0xde_u8, 0xad, 0xbe, 0xef];
        store
            .set(NAMES, &binary_key, &binary_value)
            .await
            .expect("set binary key failed");
        let binary = store.get(NAMES, &binary_key).await.expect("get binary key failed");
        assert_eq!(
            binary,
            Some(binary_value.to_vec()),
            "expected binary key/value roundtrip"
        );

        let keys = sorted(store.list_keys(NAMES).await.expect("list keys failed"));
        assert_eq!(
            keys,
            vec![binary_key.to_vec(), b"alice".to_vec()],
            "unexpected keys returned"
        );

        store
            .delete(NAMES, b"alice")
            .await
            .expect("delete alice failed");
        let deleted = store.get(NAMES, b"alice").await.expect("get deleted alice failed");
        assert!(deleted.is_none(), "expected deleted key to be missing");

        store
            .delete(NAMES, b"alice")
            .await
            .expect("delete missing key failed");

        let invalid = store.get("not-a-cf", b"k").await;
        assert!(
            matches!(invalid, Err(StoreError::InvalidColumnFamily(_))),
            "expected invalid column family error, got {invalid:?}"
        );

        store.close().await.expect("close failed");
        let closed = store.get(NAMES, b"after-close").await;
        assert!(
            matches!(closed, Err(StoreError::Closed)),
            "expected closed store error, got {closed:?}"
        );

        let reopened = OpfsStore::open_with_namespace(namespace.clone())
            .await
            .expect("reopen failed");
        let persisted = reopened
            .get(NAMES, &binary_key)
            .await
            .expect("get persisted binary key failed");
        assert_eq!(
            persisted,
            Some(binary_value.to_vec()),
            "expected data to persist across reopen"
        );

        let reopened_keys = reopened.list_keys(NAMES).await.expect("list keys after reopen failed");
        assert_eq!(
            reopened_keys,
            vec![binary_key.to_vec()],
            "expected only binary key after reopen"
        );

        reopened.close().await.expect("close reopened store failed");

        OpfsStore::remove_namespace(&namespace)
            .await
            .expect("cleanup after test failed");
    }
}
