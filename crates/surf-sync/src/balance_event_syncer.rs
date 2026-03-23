#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
use async_nats::jetstream::{self, consumer, message::Message};
#[cfg(not(target_arch = "wasm32"))]
use futures::StreamExt;
#[cfg(target_arch = "wasm32")]
use js_sys::{Reflect, Uint8Array};
use surf_events::{user_balance_subject, user_lamports_subject, EventEnvelope, EventPayload};
use surf_store::KeyValueStore;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use crate::balance_syncer::{apply_balance_record, apply_lamports_record};
use crate::checkpoint::{load_event_checkpoint, save_event_checkpoint, EventStreamCheckpoint, BALANCE_EVENT_SYNC_KEY};
use crate::config::EventStreamConfig;
use crate::error::SyncError;

#[cfg(not(target_arch = "wasm32"))]
const FETCH_EXPIRES: Duration = Duration::from_millis(250);

#[cfg(not(target_arch = "wasm32"))]
pub struct BalanceEventSyncer { context: jetstream::Context, config: EventStreamConfig }

#[cfg(not(target_arch = "wasm32"))]
impl BalanceEventSyncer {
    pub async fn connect(config: EventStreamConfig) -> Result<Self, SyncError> {
        Ok(Self { context: jetstream::new(async_nats::connect(&config.nats_url).await.map_err(|e| SyncError::EventStream(e.to_string()))?), config })
    }
    pub async fn sync_available<S: KeyValueStore>(&self, store: &S, owner: &solana_pubkey::Pubkey) -> Result<EventStreamCheckpoint, SyncError> {
        let checkpoint = load_event_checkpoint(store, BALANCE_EVENT_SYNC_KEY).await?.unwrap_or_default();
        let stream = self.context.get_stream(&self.config.stream_name).await.map_err(|e| SyncError::EventStream(e.to_string()))?;
        let consumer_name = format!("{}-balances", self.config.consumer_name);
        let consumer = stream.get_or_create_consumer(&consumer_name, consumer::pull::Config { durable_name: Some(consumer_name.clone()), filter_subjects: vec![user_balance_subject(owner), user_lamports_subject(owner)], ack_policy: consumer::AckPolicy::Explicit, deliver_policy: if checkpoint.last_stream_sequence == 0 { consumer::DeliverPolicy::All } else { consumer::DeliverPolicy::ByStartSequence { start_sequence: checkpoint.last_stream_sequence + 1 } }, max_batch: self.config.batch_size as i64, max_expires: FETCH_EXPIRES, ..Default::default() }).await.map_err(|e| SyncError::EventStream(e.to_string()))?;
        let mut messages = consumer.fetch().max_messages(self.config.batch_size).expires(FETCH_EXPIRES).messages().await.map_err(|e| SyncError::EventStream(e.to_string()))?;
        let mut next = checkpoint;
        while let Some(message) = messages.next().await { next = self.apply_message(store, message.map_err(|e| SyncError::EventStream(e.to_string()))?, next).await?; }
        Ok(next)
    }
    async fn apply_message<S: KeyValueStore>(&self, store: &S, message: Message, mut checkpoint: EventStreamCheckpoint) -> Result<EventStreamCheckpoint, SyncError> {
        let info = message.info().map_err(|e| SyncError::EventStream(e.to_string()))?;
        let envelope: EventEnvelope = serde_json::from_slice(&message.payload).map_err(|e| SyncError::InvalidEvent(e.to_string()))?;
        match &envelope.payload {
            EventPayload::BalanceUpdated(payload) => apply_balance_record(store, &payload.owner, &payload.record).await?,
            EventPayload::LamportsUpdated(payload) => apply_lamports_record(store, &payload.owner, payload.lamports).await?,
            _ => {}
        }
        checkpoint.update(info.stream_sequence, envelope.slot, Some(envelope.event_id.clone()));
        save_event_checkpoint(store, BALANCE_EVENT_SYNC_KEY, &checkpoint).await?;
        message.ack().await.map_err(|e| SyncError::EventStream(e.to_string()))?;
        Ok(checkpoint)
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(module = "/src/follow_event_syncer_wasm.js")]
extern "C" {
    #[wasm_bindgen(catch, js_name = connectEventStreamClient)] async fn js_connect_event_stream_client(url: String) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch, js_name = subscribeEventConsumer)] async fn js_subscribe_event_consumer(client: &JsValue, stream_name: String, consumer_name: String, subjects: js_sys::Array, start_sequence: Option<u64>) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch, js_name = nextEventMessage)] async fn js_next_event_message(client: &JsValue, subscription_id: String) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch, js_name = ackEventMessage)] async fn js_ack_event_message(client: &JsValue, ack_id: String) -> Result<(), JsValue>;
    #[wasm_bindgen(js_name = emitSyncUpdate)] fn js_emit_sync_update(domain: &str);
}

#[cfg(target_arch = "wasm32")]
pub struct BalanceEventSyncer { client: JsValue, config: EventStreamConfig }

#[cfg(target_arch = "wasm32")]
impl BalanceEventSyncer {
    pub async fn connect(config: EventStreamConfig) -> Result<Self, SyncError> { Ok(Self { client: js_connect_event_stream_client(config.nats_url.clone()).await.map_err(js_error)?, config }) }
    pub async fn sync_available<S: KeyValueStore>(&self, store: &S, owner: &solana_pubkey::Pubkey) -> Result<EventStreamCheckpoint, SyncError> {
        let mut checkpoint = load_event_checkpoint(store, BALANCE_EVENT_SYNC_KEY).await?.unwrap_or_default();
        let subscription_id = self.subscribe(owner, &checkpoint).await?;
        let result = js_next_event_message(&self.client, subscription_id).await.map_err(js_error)?;
        checkpoint = self.apply_js_message(store, &result, checkpoint).await?;
        Ok(checkpoint)
    }

    pub async fn stream_updates<S: KeyValueStore>(&self, store: &S, owner: &solana_pubkey::Pubkey) -> Result<(), SyncError> {
        let mut checkpoint = load_event_checkpoint(store, BALANCE_EVENT_SYNC_KEY).await?.unwrap_or_default();
        let subscription_id = self.subscribe(owner, &checkpoint).await?;
        loop {
            let result = js_next_event_message(&self.client, subscription_id.clone()).await.map_err(js_error)?;
            checkpoint = self.apply_js_message(store, &result, checkpoint).await?;
        }
    }

    async fn subscribe(&self, owner: &solana_pubkey::Pubkey, checkpoint: &EventStreamCheckpoint) -> Result<String, SyncError> {
        let subjects = js_sys::Array::new();
        subjects.push(&JsValue::from_str(&user_balance_subject(owner)));
        subjects.push(&JsValue::from_str(&user_lamports_subject(owner)));
        js_subscribe_event_consumer(
            &self.client,
            self.config.stream_name.clone(),
            format!("{}-balances", self.config.consumer_name),
            subjects,
            if checkpoint.last_stream_sequence == 0 { None } else { Some(checkpoint.last_stream_sequence + 1) },
        ).await.map_err(js_error)?.as_string().ok_or_else(|| SyncError::InvalidEvent("missing subscription id".to_string()))
    }

    async fn apply_js_message<S: KeyValueStore>(&self, store: &S, value: &JsValue, mut checkpoint: EventStreamCheckpoint) -> Result<EventStreamCheckpoint, SyncError> {
        let payload = reflect_bytes(value, "payload")?; let stream_sequence = reflect_u64(value, "streamSequence")?; let ack_id = reflect_string(value, "ackId")?;
        let envelope: EventEnvelope = serde_json::from_slice(&payload).map_err(|e| SyncError::InvalidEvent(e.to_string()))?;
        match &envelope.payload { EventPayload::BalanceUpdated(payload) => apply_balance_record(store, &payload.owner, &payload.record).await?, EventPayload::LamportsUpdated(payload) => apply_lamports_record(store, &payload.owner, payload.lamports).await?, _ => {} }
        checkpoint.update(stream_sequence, envelope.slot, Some(envelope.event_id.clone()));
        save_event_checkpoint(store, BALANCE_EVENT_SYNC_KEY, &checkpoint).await?;
        js_ack_event_message(&self.client, ack_id).await.map_err(js_error)?;
        js_emit_sync_update("balances");
        Ok(checkpoint)
    }
}

#[cfg(target_arch = "wasm32")]
fn js_error(value: JsValue) -> SyncError { SyncError::EventStream(value.as_string().unwrap_or_else(|| format!("{value:?}"))) }
#[cfg(target_arch = "wasm32")]
fn reflect_string(value: &JsValue, key: &str) -> Result<String, SyncError> { Reflect::get(value, &JsValue::from_str(key)).map_err(js_error)?.as_string().ok_or_else(|| SyncError::InvalidEvent(format!("missing string field {key}"))) }
#[cfg(target_arch = "wasm32")]
fn reflect_u64(value: &JsValue, key: &str) -> Result<u64, SyncError> { Reflect::get(value, &JsValue::from_str(key)).map_err(js_error)?.as_f64().map(|v| v as u64).ok_or_else(|| SyncError::InvalidEvent(format!("missing numeric field {key}"))) }
#[cfg(target_arch = "wasm32")]
fn reflect_bytes(value: &JsValue, key: &str) -> Result<Vec<u8>, SyncError> { Ok(Uint8Array::new(&Reflect::get(value, &JsValue::from_str(key)).map_err(js_error)?).to_vec()) }
