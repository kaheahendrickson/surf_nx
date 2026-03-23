#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
use async_nats::jetstream::{self, consumer, message::Message};
#[cfg(not(target_arch = "wasm32"))]
use futures::StreamExt;
#[cfg(target_arch = "wasm32")]
use js_sys::{Reflect, Uint8Array};
use solana_signature::Signature;
use surf_events::{user_follows_subject, EventEnvelope, EventPayload};
use surf_store::KeyValueStore;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use crate::checkpoint::{
    load_event_checkpoint, save_event_checkpoint, EventStreamCheckpoint, FOLLOW_EVENT_SYNC_KEY,
};
use crate::config::EventStreamConfig;
use crate::error::SyncError;
use crate::follow_syncer::{apply_follow_created, apply_follow_removed, FollowRecord};

#[cfg(not(target_arch = "wasm32"))]
const FETCH_EXPIRES: Duration = Duration::from_millis(250);

#[cfg(not(target_arch = "wasm32"))]
pub struct FollowEventSyncer {
    context: jetstream::Context,
    config: EventStreamConfig,
}

#[cfg(not(target_arch = "wasm32"))]
impl FollowEventSyncer {
    pub async fn connect(config: EventStreamConfig) -> Result<Self, SyncError> {
        let client = async_nats::connect(&config.nats_url)
            .await
            .map_err(|err| SyncError::EventStream(err.to_string()))?;
        Ok(Self {
            context: jetstream::new(client),
            config,
        })
    }

    pub async fn sync_available<S: KeyValueStore>(
        &self,
        store: &S,
        tracked_balance: &solana_pubkey::Pubkey,
    ) -> Result<EventStreamCheckpoint, SyncError> {
        let checkpoint = load_event_checkpoint(store, FOLLOW_EVENT_SYNC_KEY)
            .await?
            .unwrap_or_default();
        let subject = user_follows_subject(tracked_balance);
        let stream = self
            .context
            .get_stream(&self.config.stream_name)
            .await
            .map_err(|err| SyncError::EventStream(err.to_string()))?;
        let consumer = stream
            .get_or_create_consumer(
                &self.config.consumer_name,
                consumer::pull::Config {
                    durable_name: Some(self.config.consumer_name.clone()),
                    filter_subject: subject,
                    ack_policy: consumer::AckPolicy::Explicit,
                    deliver_policy: if checkpoint.last_stream_sequence == 0 {
                        consumer::DeliverPolicy::All
                    } else {
                        consumer::DeliverPolicy::ByStartSequence {
                            start_sequence: checkpoint.last_stream_sequence + 1,
                        }
                    },
                    max_batch: self.config.batch_size as i64,
                    max_expires: FETCH_EXPIRES,
                    ..Default::default()
                },
            )
            .await
            .map_err(|err| SyncError::EventStream(err.to_string()))?;

        let mut messages = consumer
            .fetch()
            .max_messages(self.config.batch_size)
            .expires(FETCH_EXPIRES)
            .messages()
            .await
            .map_err(|err| SyncError::EventStream(err.to_string()))?;

        let mut next = checkpoint;
        while let Some(message) = messages.next().await {
            let message = message.map_err(|err| SyncError::EventStream(err.to_string()))?;
            next = self.apply_message(store, message, next).await?;
        }

        Ok(next)
    }

    async fn apply_message<S: KeyValueStore>(
        &self,
        store: &S,
        message: Message,
        mut checkpoint: EventStreamCheckpoint,
    ) -> Result<EventStreamCheckpoint, SyncError> {
        let info = message
            .info()
            .map_err(|err| SyncError::EventStream(err.to_string()))?;
        let envelope: EventEnvelope = serde_json::from_slice(&message.payload)
            .map_err(|err| SyncError::InvalidEvent(err.to_string()))?;

        apply_envelope(store, &envelope).await?;

        checkpoint.update(
            info.stream_sequence,
            envelope.slot,
            Some(envelope.event_id.clone()),
        );
        save_event_checkpoint(store, FOLLOW_EVENT_SYNC_KEY, &checkpoint).await?;
        message
            .ack()
            .await
            .map_err(|err| SyncError::EventStream(err.to_string()))?;
        Ok(checkpoint)
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(module = "/src/follow_event_syncer_wasm.js")]
extern "C" {
    #[wasm_bindgen(catch, js_name = connectEventStreamClient)]
    async fn js_connect_event_stream_client(url: String) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch, js_name = subscribeEventConsumer)]
    async fn js_subscribe_event_consumer(
        client: &JsValue,
        stream_name: String,
        consumer_name: String,
        subjects: js_sys::Array,
        start_sequence: Option<u64>,
    ) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch, js_name = nextEventMessage)]
    async fn js_next_event_message(client: &JsValue, subscription_id: String) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch, js_name = ackEventMessage)]
    async fn js_ack_event_message(client: &JsValue, ack_id: String) -> Result<(), JsValue>;

    #[wasm_bindgen(js_name = emitSyncUpdate)]
    fn js_emit_sync_update(domain: &str);
}

#[cfg(target_arch = "wasm32")]
pub struct FollowEventSyncer {
    client: JsValue,
    config: EventStreamConfig,
}

#[cfg(target_arch = "wasm32")]
impl FollowEventSyncer {
    pub async fn connect(config: EventStreamConfig) -> Result<Self, SyncError> {
        let client = js_connect_event_stream_client(config.nats_url.clone())
            .await
            .map_err(js_error)?;
        Ok(Self { client, config })
    }

    pub async fn sync_available<S: KeyValueStore>(
        &self,
        store: &S,
        tracked_balance: &solana_pubkey::Pubkey,
    ) -> Result<EventStreamCheckpoint, SyncError> {
        let mut checkpoint = load_event_checkpoint(store, FOLLOW_EVENT_SYNC_KEY)
            .await?
            .unwrap_or_default();
        let subscription_id = self.subscribe(tracked_balance, &checkpoint).await?;
        let result = js_next_event_message(&self.client, subscription_id)
            .await
            .map_err(js_error)?;
        checkpoint = self.apply_js_message(store, &result, checkpoint).await?;
        Ok(checkpoint)
    }

    pub async fn stream_updates<S: KeyValueStore>(
        &self,
        store: &S,
        tracked_balance: &solana_pubkey::Pubkey,
    ) -> Result<(), SyncError> {
        let mut checkpoint = load_event_checkpoint(store, FOLLOW_EVENT_SYNC_KEY)
            .await?
            .unwrap_or_default();
        let subscription_id = self.subscribe(tracked_balance, &checkpoint).await?;

        loop {
            let result = js_next_event_message(&self.client, subscription_id.clone())
                .await
                .map_err(js_error)?;
            checkpoint = self.apply_js_message(store, &result, checkpoint).await?;
        }
    }

    async fn subscribe(
        &self,
        tracked_balance: &solana_pubkey::Pubkey,
        checkpoint: &EventStreamCheckpoint,
    ) -> Result<String, SyncError> {
        let subject = user_follows_subject(tracked_balance);
        let subjects = js_sys::Array::new();
        subjects.push(&JsValue::from_str(&subject));

        js_subscribe_event_consumer(
            &self.client,
            self.config.stream_name.clone(),
            self.config.consumer_name.clone(),
            subjects,
            if checkpoint.last_stream_sequence == 0 {
                None
            } else {
                Some(checkpoint.last_stream_sequence + 1)
            },
        )
        .await
        .map_err(js_error)?
        .as_string()
        .ok_or_else(|| SyncError::InvalidEvent("missing subscription id".to_string()))
    }

    async fn apply_js_message<S: KeyValueStore>(
        &self,
        store: &S,
        value: &JsValue,
        mut checkpoint: EventStreamCheckpoint,
    ) -> Result<EventStreamCheckpoint, SyncError> {
        let payload = reflect_bytes(value, "payload")?;
        let stream_sequence = reflect_u64(value, "streamSequence")?;
        let ack_id = reflect_string(value, "ackId")?;
        let envelope: EventEnvelope = serde_json::from_slice(&payload)
            .map_err(|err| SyncError::InvalidEvent(err.to_string()))?;

        apply_envelope(store, &envelope).await?;
        checkpoint.update(
            stream_sequence,
            envelope.slot,
            Some(envelope.event_id.clone()),
        );
        save_event_checkpoint(store, FOLLOW_EVENT_SYNC_KEY, &checkpoint).await?;
        js_ack_event_message(&self.client, ack_id)
            .await
            .map_err(js_error)?;
        js_emit_sync_update("follows");
        Ok(checkpoint)
    }
}

async fn apply_envelope<S: KeyValueStore>(
    store: &S,
    envelope: &EventEnvelope,
) -> Result<(), SyncError> {
    let signature = parse_signature_bytes(&envelope.signature)?;

    match &envelope.payload {
        EventPayload::FollowCreated(payload) => {
            let record = FollowRecord {
                slot: envelope.slot,
                block_time: envelope.observed_at,
                signature,
            };
            apply_follow_created(store, &payload.target, &record).await?;
        }
        EventPayload::FollowRemoved(payload) => {
            apply_follow_removed(store, &payload.target).await?;
        }
        _ => {}
    }

    Ok(())
}

fn parse_signature_bytes(signature: &str) -> Result<[u8; 64], SyncError> {
    let parsed: Signature = signature
        .parse()
        .map_err(|err| SyncError::InvalidEvent(format!("invalid signature: {err}")))?;
    let mut bytes = [0u8; 64];
    bytes.copy_from_slice(parsed.as_ref());
    Ok(bytes)
}

#[cfg(target_arch = "wasm32")]
fn js_error(value: JsValue) -> SyncError {
    if let Some(message) = value.as_string() {
        SyncError::EventStream(message)
    } else {
        SyncError::EventStream(format!("{value:?}"))
    }
}

#[cfg(target_arch = "wasm32")]
fn reflect_string(value: &JsValue, key: &str) -> Result<String, SyncError> {
    Reflect::get(value, &JsValue::from_str(key))
        .map_err(js_error)?
        .as_string()
        .ok_or_else(|| SyncError::InvalidEvent(format!("missing string field {key}")))
}

#[cfg(target_arch = "wasm32")]
fn reflect_u64(value: &JsValue, key: &str) -> Result<u64, SyncError> {
    Reflect::get(value, &JsValue::from_str(key))
        .map_err(js_error)?
        .as_f64()
        .map(|value| value as u64)
        .ok_or_else(|| SyncError::InvalidEvent(format!("missing numeric field {key}")))
}

#[cfg(target_arch = "wasm32")]
fn reflect_bytes(value: &JsValue, key: &str) -> Result<Vec<u8>, SyncError> {
    let raw = Reflect::get(value, &JsValue::from_str(key)).map_err(js_error)?;
    Ok(Uint8Array::new(&raw).to_vec())
}

// TODO: Uncomment when test dependencies are available
// #[cfg(test)]
// mod tests {
//     use super::*;
//
//     #[test]
//     fn parses_signature_bytes() {
//         let signature = Signature::from([3; 64]).to_string();
//         let bytes = parse_signature_bytes(&signature).unwrap();
//         assert_eq!(bytes, [3; 64]);
//     }
// }
