#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
use async_nats::jetstream::{self, consumer, message::Message};
#[cfg(not(target_arch = "wasm32"))]
use futures::StreamExt;
#[cfg(target_arch = "wasm32")]
use js_sys::{Reflect, Uint8Array};
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use surf_events::{user_activity_subject, ActivityRecorded, EventEnvelope, EventPayload};
use surf_store::KeyValueStore;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use crate::activity_syncer::{apply_activity_record, ActivityRecord};
use crate::checkpoint::{load_event_checkpoint, save_event_checkpoint, EventStreamCheckpoint, ACTIVITY_EVENT_SYNC_KEY};
use crate::config::EventStreamConfig;
use crate::error::SyncError;
use crate::parser::ActivityKind;

#[cfg(not(target_arch = "wasm32"))]
const FETCH_EXPIRES: Duration = Duration::from_millis(250);

#[cfg(not(target_arch = "wasm32"))]
pub struct ActivityEventSyncer { context: jetstream::Context, config: EventStreamConfig }

#[cfg(not(target_arch = "wasm32"))]
impl ActivityEventSyncer {
    pub async fn connect(config: EventStreamConfig) -> Result<Self, SyncError> { Ok(Self { context: jetstream::new(async_nats::connect(&config.nats_url).await.map_err(|e| SyncError::EventStream(e.to_string()))?), config }) }
    pub async fn sync_available<S: KeyValueStore>(&self, store: &S, owner: &Pubkey) -> Result<EventStreamCheckpoint, SyncError> {
        let checkpoint = load_event_checkpoint(store, ACTIVITY_EVENT_SYNC_KEY).await?.unwrap_or_default();
        let stream = self.context.get_stream(&self.config.stream_name).await.map_err(|e| SyncError::EventStream(e.to_string()))?;
        let consumer_name = format!("{}-activity", self.config.consumer_name);
        let consumer = stream.get_or_create_consumer(&consumer_name, consumer::pull::Config { durable_name: Some(consumer_name.clone()), filter_subject: user_activity_subject(owner), ack_policy: consumer::AckPolicy::Explicit, deliver_policy: if checkpoint.last_stream_sequence == 0 { consumer::DeliverPolicy::All } else { consumer::DeliverPolicy::ByStartSequence { start_sequence: checkpoint.last_stream_sequence + 1 } }, max_batch: self.config.batch_size as i64, max_expires: FETCH_EXPIRES, ..Default::default() }).await.map_err(|e| SyncError::EventStream(e.to_string()))?;
        let mut messages = consumer.fetch().max_messages(self.config.batch_size).expires(FETCH_EXPIRES).messages().await.map_err(|e| SyncError::EventStream(e.to_string()))?;
        let mut next = checkpoint;
        while let Some(message) = messages.next().await { next = self.apply_message(store, message.map_err(|e| SyncError::EventStream(e.to_string()))?, next).await?; }
        Ok(next)
    }
    async fn apply_message<S: KeyValueStore>(&self, store: &S, message: Message, mut checkpoint: EventStreamCheckpoint) -> Result<EventStreamCheckpoint, SyncError> {
        let info = message.info().map_err(|e| SyncError::EventStream(e.to_string()))?;
        let envelope: EventEnvelope = serde_json::from_slice(&message.payload).map_err(|e| SyncError::InvalidEvent(e.to_string()))?;
        if let EventPayload::ActivityRecorded(payload) = &envelope.payload {
            let record = decode_activity_record(payload, envelope.slot, envelope.observed_at, &envelope.signature)?;
            let signature = parse_signature(&envelope.signature)?;
            apply_activity_record(store, &signature, &record).await?;
        }
        checkpoint.update(info.stream_sequence, envelope.slot, Some(envelope.event_id.clone()));
        save_event_checkpoint(store, ACTIVITY_EVENT_SYNC_KEY, &checkpoint).await?;
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
pub struct ActivityEventSyncer { client: JsValue, config: EventStreamConfig }

#[cfg(target_arch = "wasm32")]
impl ActivityEventSyncer {
    pub async fn connect(config: EventStreamConfig) -> Result<Self, SyncError> { Ok(Self { client: js_connect_event_stream_client(config.nats_url.clone()).await.map_err(js_error)?, config }) }
    pub async fn sync_available<S: KeyValueStore>(&self, store: &S, owner: &Pubkey) -> Result<EventStreamCheckpoint, SyncError> {
        let mut checkpoint = load_event_checkpoint(store, ACTIVITY_EVENT_SYNC_KEY).await?.unwrap_or_default();
        let subscription_id = self.subscribe(owner, &checkpoint).await?;
        let result = js_next_event_message(&self.client, subscription_id).await.map_err(js_error)?;
        checkpoint = self.apply_js_message(store, &result, checkpoint).await?;
        Ok(checkpoint)
    }

    pub async fn stream_updates<S: KeyValueStore>(&self, store: &S, owner: &Pubkey) -> Result<(), SyncError> {
        let mut checkpoint = load_event_checkpoint(store, ACTIVITY_EVENT_SYNC_KEY).await?.unwrap_or_default();
        let subscription_id = self.subscribe(owner, &checkpoint).await?;
        loop {
            let result = js_next_event_message(&self.client, subscription_id.clone()).await.map_err(js_error)?;
            checkpoint = self.apply_js_message(store, &result, checkpoint).await?;
        }
    }

    async fn subscribe(&self, owner: &Pubkey, checkpoint: &EventStreamCheckpoint) -> Result<String, SyncError> {
        let subjects = js_sys::Array::new();
        subjects.push(&JsValue::from_str(&user_activity_subject(owner)));
        js_subscribe_event_consumer(
            &self.client,
            self.config.stream_name.clone(),
            format!("{}-activity", self.config.consumer_name),
            subjects,
            if checkpoint.last_stream_sequence == 0 { None } else { Some(checkpoint.last_stream_sequence + 1) },
        ).await.map_err(js_error)?.as_string().ok_or_else(|| SyncError::InvalidEvent("missing subscription id".to_string()))
    }

    async fn apply_js_message<S: KeyValueStore>(&self, store: &S, value: &JsValue, mut checkpoint: EventStreamCheckpoint) -> Result<EventStreamCheckpoint, SyncError> {
        let payload = reflect_bytes(value, "payload")?; let stream_sequence = reflect_u64(value, "streamSequence")?; let ack_id = reflect_string(value, "ackId")?;
        let envelope: EventEnvelope = serde_json::from_slice(&payload).map_err(|e| SyncError::InvalidEvent(e.to_string()))?;
        if let EventPayload::ActivityRecorded(payload) = &envelope.payload { let record = decode_activity_record(payload, envelope.slot, envelope.observed_at, &envelope.signature)?; let signature = parse_signature(&envelope.signature)?; apply_activity_record(store, &signature, &record).await?; }
        checkpoint.update(stream_sequence, envelope.slot, Some(envelope.event_id.clone()));
        save_event_checkpoint(store, ACTIVITY_EVENT_SYNC_KEY, &checkpoint).await?;
        js_ack_event_message(&self.client, ack_id).await.map_err(js_error)?;
        js_emit_sync_update("activity");
        Ok(checkpoint)
    }
}

fn decode_activity_record(payload: &ActivityRecorded, slot: u64, observed_at: i64, signature: &str) -> Result<ActivityRecord, SyncError> {
    let kind = ActivityKind::from_u8(payload.kind)?;
    Ok(ActivityRecord { kind, counterparty: payload.counterparty, amount: payload.amount, slot, block_time: observed_at, signature: parse_signature_bytes(signature)? })
}

fn parse_signature(value: &str) -> Result<Signature, SyncError> { value.parse().map_err(|e| SyncError::InvalidEvent(format!("invalid signature: {e}"))) }
fn parse_signature_bytes(value: &str) -> Result<[u8; 64], SyncError> { let sig = parse_signature(value)?; let mut bytes = [0u8; 64]; bytes.copy_from_slice(sig.as_ref()); Ok(bytes) }

#[cfg(target_arch = "wasm32")]
fn js_error(value: JsValue) -> SyncError { SyncError::EventStream(value.as_string().unwrap_or_else(|| format!("{value:?}"))) }
#[cfg(target_arch = "wasm32")]
fn reflect_string(value: &JsValue, key: &str) -> Result<String, SyncError> { Reflect::get(value, &JsValue::from_str(key)).map_err(js_error)?.as_string().ok_or_else(|| SyncError::InvalidEvent(format!("missing string field {key}"))) }
#[cfg(target_arch = "wasm32")]
fn reflect_u64(value: &JsValue, key: &str) -> Result<u64, SyncError> { Reflect::get(value, &JsValue::from_str(key)).map_err(js_error)?.as_f64().map(|v| v as u64).ok_or_else(|| SyncError::InvalidEvent(format!("missing numeric field {key}"))) }
#[cfg(target_arch = "wasm32")]
fn reflect_bytes(value: &JsValue, key: &str) -> Result<Vec<u8>, SyncError> { Ok(Uint8Array::new(&Reflect::get(value, &JsValue::from_str(key)).map_err(js_error)?).to_vec()) }
