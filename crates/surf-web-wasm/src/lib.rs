#[cfg(target_arch = "wasm32")]
use std::sync::Arc;

#[cfg(target_arch = "wasm32")]
use js_sys::Array;
#[cfg(target_arch = "wasm32")]
use js_sys::Object;
#[cfg(target_arch = "wasm32")]
use solana_pubkey::Pubkey;
#[cfg(target_arch = "wasm32")]
use surf_client::QueryClient;
#[cfg(target_arch = "wasm32")]
use surf_client_backend_http::HttpBackend;
#[cfg(target_arch = "wasm32")]
use surf_client_http_config::HttpBackendConfig;
#[cfg(target_arch = "wasm32")]
use surf_protocol::decode_token_balance;
#[cfg(target_arch = "wasm32")]
use surf_store::{KeyValueStore, OpfsStore, BALANCES, LAMPORTS};
#[cfg(target_arch = "wasm32")]
use surf_sync::{EventStreamConfig, SyncConfig, SyncService, WasmSleep};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
fn parse_pubkey(value: &str, label: &str) -> Result<Pubkey, JsValue> {
    value
        .parse()
        .map_err(|err| JsValue::from_str(&format!("invalid {label}: {err}")))
}

#[cfg(target_arch = "wasm32")]
fn backend(url: &str) -> HttpBackend {
    HttpBackend::from_config(HttpBackendConfig::new(url))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn run_sync_worker(
    validator_url: String,
    token_program: String,
    registry_program: String,
    signals_program: String,
    owner_pubkey: String,
    namespace: String,
    nats_ws_url: Option<String>,
    events_stream: Option<String>,
    events_consumer: Option<String>,
) -> Result<(), JsValue> {
    let token_program = parse_pubkey(&token_program, "token program id")?;
    let registry_program = parse_pubkey(&registry_program, "registry program id")?;
    let signals_program = parse_pubkey(&signals_program, "signals program id")?;
    let tracked_balance = parse_pubkey(&owner_pubkey, "owner pubkey")?;

    let store = OpfsStore::open_with_namespace(namespace)
        .await
        .map_err(|err| JsValue::from_str(&format!("failed to open OPFS store: {err}")))?;

    let mut config = SyncConfig::new(token_program, registry_program, signals_program, tracked_balance)
        .with_poll_interval(1_000);
    if let (Some(nats_ws_url), Some(events_stream), Some(events_consumer)) =
        (nats_ws_url, events_stream, events_consumer)
    {
        config = config.with_event_stream(EventStreamConfig::new(
            nats_ws_url,
            events_stream,
            events_consumer,
        ));
    }

    let mut sync_service = SyncService::new(
        Arc::new(backend(&validator_url)),
        store,
        config,
        WasmSleep,
    )
    .map_err(|err| JsValue::from_str(&format!("failed to create sync service: {err}")))?;

    sync_service
        .sync()
        .await
        .map_err(|err| JsValue::from_str(&format!("sync worker failed: {err}")))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn get_synced_names(namespace: String) -> Result<Array, JsValue> {
    let store = OpfsStore::open_with_namespace(namespace)
        .await
        .map_err(|err| JsValue::from_str(&format!("failed to open OPFS store: {err}")))?;
    let names = surf_query::get_names(&store)
        .await
        .map_err(|err| JsValue::from_str(&format!("failed to query names: {err}")))?;

    let result = Array::new();
    for name in names {
        result.push(&JsValue::from_str(&name));
    }

    Ok(result)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn get_synced_transactions(namespace: String) -> Result<Array, JsValue> {
    let store = OpfsStore::open_with_namespace(namespace)
        .await
        .map_err(|err| JsValue::from_str(&format!("failed to open OPFS store: {err}")))?;
    let transactions = surf_query::get_transactions(&store)
        .await
        .map_err(|err| JsValue::from_str(&format!("failed to query transactions: {err}")))?;

    let result = Array::new();
    for transaction in transactions {
        let object = Object::new();
        js_sys::Reflect::set(&object, &JsValue::from_str("signature"), &JsValue::from_str(&transaction.signature))?;
        js_sys::Reflect::set(&object, &JsValue::from_str("kind"), &JsValue::from_str(&transaction.kind))?;
        js_sys::Reflect::set(&object, &JsValue::from_str("counterparty"), &JsValue::from_str(&transaction.counterparty))?;
        js_sys::Reflect::set(
            &object,
            &JsValue::from_str("counterpartyName"),
            &transaction
                .counterparty_name
                .map(|value| JsValue::from_str(&value))
                .unwrap_or(JsValue::NULL),
        )?;
        js_sys::Reflect::set(&object, &JsValue::from_str("amount"), &JsValue::from_f64(transaction.amount as f64))?;
        js_sys::Reflect::set(&object, &JsValue::from_str("slot"), &JsValue::from_f64(transaction.slot as f64))?;
        js_sys::Reflect::set(
            &object,
            &JsValue::from_str("blockTime"),
            &transaction
                .block_time
                .map(|value| JsValue::from_f64(value as f64))
                .unwrap_or(JsValue::NULL),
        )?;
        result.push(&object);
    }

    Ok(result)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn get_synced_following(namespace: String) -> Result<Array, JsValue> {
    let store = OpfsStore::open_with_namespace(namespace)
        .await
        .map_err(|err| JsValue::from_str(&format!("failed to open OPFS store: {err}")))?;
    let follows = surf_query::get_following(&store)
        .await
        .map_err(|err| JsValue::from_str(&format!("failed to query following: {err}")))?;

    let result = Array::new();
    for follow in follows {
        let object = Object::new();
        js_sys::Reflect::set(&object, &JsValue::from_str("target"), &JsValue::from_str(&follow.target))?;
        js_sys::Reflect::set(
            &object,
            &JsValue::from_str("targetName"),
            &follow
                .target_name
                .map(|value| JsValue::from_str(&value))
                .unwrap_or(JsValue::NULL),
        )?;
        js_sys::Reflect::set(&object, &JsValue::from_str("slot"), &JsValue::from_f64(follow.slot as f64))?;
        js_sys::Reflect::set(
            &object,
            &JsValue::from_str("blockTime"),
            &follow
                .block_time
                .map(|value| JsValue::from_f64(value as f64))
                .unwrap_or(JsValue::NULL),
        )?;
        js_sys::Reflect::set(&object, &JsValue::from_str("signature"), &JsValue::from_str(&follow.signature))?;
        result.push(&object);
    }

    Ok(result)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn clear_synced_data(namespace: String) -> Result<(), JsValue> {
    OpfsStore::remove_namespace(&namespace)
        .await
        .map_err(|err| JsValue::from_str(&format!("failed to clear OPFS namespace: {err}")))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn get_owner_balance(
    validator_url: String,
    token_program: String,
    registry_program: String,
    owner_pubkey: String,
) -> Result<String, JsValue> {
    let token_program = parse_pubkey(&token_program, "token program id")?;
    let registry_program = parse_pubkey(&registry_program, "registry program id")?;
    let owner_pubkey = parse_pubkey(&owner_pubkey, "owner pubkey")?;

    let query = QueryClient::new(backend(&validator_url), token_program, registry_program);
    let balance = query
        .balance(&owner_pubkey)
        .await
        .map_err(|err| JsValue::from_str(&format!("failed to query owner balance: {err}")))?;

    Ok(balance.to_string())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn get_synced_balance(namespace: String, owner_pubkey: String) -> Result<String, JsValue> {
    let owner_pubkey = parse_pubkey(&owner_pubkey, "owner pubkey")?;
    let store = OpfsStore::open_with_namespace(namespace)
        .await
        .map_err(|err| JsValue::from_str(&format!("failed to open OPFS store: {err}")))?;
    let data = store
        .get(BALANCES, owner_pubkey.as_ref())
        .await
        .map_err(|err| JsValue::from_str(&format!("failed to read synced balance: {err}")))?
        .ok_or_else(|| JsValue::from_str("synced balance not found"))?;
    let balance = decode_token_balance(&data).ok_or_else(|| JsValue::from_str("invalid synced balance data"))?;
    Ok(balance.amount.to_string())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn get_synced_lamports(
    namespace: String,
    owner_pubkey: String,
) -> Result<String, JsValue> {
    let owner_pubkey = parse_pubkey(&owner_pubkey, "owner pubkey")?;
    let store = OpfsStore::open_with_namespace(namespace)
        .await
        .map_err(|err| JsValue::from_str(&format!("failed to open OPFS store: {err}")))?;
    let data = store
        .get(LAMPORTS, owner_pubkey.as_ref())
        .await
        .map_err(|err| JsValue::from_str(&format!("failed to read synced lamports: {err}")))?
        .ok_or_else(|| JsValue::from_str("synced lamports not found"))?;

    if data.len() != 8 {
        return Err(JsValue::from_str("invalid synced lamports length"));
    }

    Ok(u64::from_le_bytes(
        data.as_slice()
            .try_into()
            .map_err(|_| JsValue::from_str("invalid synced lamports bytes"))?,
    )
    .to_string())
}
