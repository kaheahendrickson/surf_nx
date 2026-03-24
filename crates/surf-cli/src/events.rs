use std::error::Error as StdError;
use std::fmt;
use std::time::Duration;

use async_nats::jetstream::{self, consumer, message::Message};
use futures::StreamExt;
use serde_json::Value;
use solana_pubkey::Pubkey;
use surf_events::{
    global_names_subject, user_activity_subject, user_balance_subject, user_follows_subject,
    user_lamports_subject, EventEnvelope, EventPayload,
};

const DEFAULT_NATS_URL: &str = "nats://127.0.0.1:4222";
const DEFAULT_STREAM_NAME: &str = "surf-events";
const FETCH_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Debug)]
pub struct EventsError(String);

impl fmt::Display for EventsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl StdError for EventsError {}

type Result<T> = std::result::Result<T, Box<dyn StdError>>;

pub enum EventType {
    FollowCreated,
    FollowRemoved,
    NameRegistered,
    BalanceUpdated,
    LamportsUpdated,
    ActivityRecorded,
}

impl EventType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "follow.created" => Some(Self::FollowCreated),
            "follow.removed" => Some(Self::FollowRemoved),
            "name.registered" => Some(Self::NameRegistered),
            "balance.updated" => Some(Self::BalanceUpdated),
            "lamports.updated" => Some(Self::LamportsUpdated),
            "activity.recorded" => Some(Self::ActivityRecorded),
            _ => None,
        }
    }

    pub fn all() -> &'static [&'static str] {
        &[
            "follow.created",
            "follow.removed",
            "name.registered",
            "balance.updated",
            "lamports.updated",
            "activity.recorded",
        ]
    }
}

pub struct EventReader {
    context: jetstream::Context,
    stream_name: String,
}

impl EventReader {
    pub async fn connect(nats_url: Option<&str>) -> Result<Self> {
        let url = nats_url.unwrap_or(DEFAULT_NATS_URL);
        let client = async_nats::connect(url)
            .await
            .map_err(|err| Box::new(EventsError(format!("failed to connect to NATS: {err}"))) as Box<dyn StdError>)?;
        Ok(Self {context: jetstream::new(client),
            stream_name: DEFAULT_STREAM_NAME.to_string(),
        })
    }

    pub fn with_stream_name(mut self, name: String) -> Self {
        self.stream_name = name;
        self
    }

    pub async fn subscribe(
        &self,
        subject: &str,
        json_output: bool,
    ) -> Result<()> {
        let stream = self
            .context
            .get_stream(&self.stream_name)
            .await
            .map_err(|err| Box::new(EventsError(format!("failed to get stream: {err}"))) as Box<dyn StdError>)?;

        let consumer: consumer::Consumer<consumer::pull::Config> = stream
            .create_consumer(consumer::pull::Config {
                filter_subject: subject.to_string(),
                deliver_policy: consumer::DeliverPolicy::All,
                ack_policy: consumer::AckPolicy::Explicit,
                ..Default::default()
            })
            .await
            .map_err(|err| Box::new(EventsError(format!("failed to create consumer: {err}"))) as Box<dyn StdError>)?;

        let mut messages = consumer
            .messages()
            .await
            .map_err(|err| Box::new(EventsError(format!("failed to subscribe: {err}"))) as Box<dyn StdError>)?;

        println!("Subscribed to subject: {subject}");
        println!("Press Ctrl+C to stop...");
        println!();

        while let Some(message) = messages.next().await {
            let message = message.map_err(|err| Box::new(EventsError(format!("message error: {err}"))) as Box<dyn StdError>)?;
            print_event(&message, json_output)?;
            message
                .ack()
                .await
                .map_err(|err| Box::new(EventsError(format!("ack error: {err}"))) as Box<dyn StdError>)?;
        }

        Ok(())
    }

    pub async fn fetch(
        &self,
        subject: &str,
        limit: usize,
        json_output: bool,
    ) -> Result<()> {
        let stream = self
            .context
            .get_stream(&self.stream_name)
            .await
            .map_err(|err| Box::new(EventsError(format!("failed to get stream: {err}"))) as Box<dyn StdError>)?;

        let consumer: consumer::Consumer<consumer::pull::Config> = stream
            .create_consumer(consumer::pull::Config {
                filter_subject: subject.to_string(),
                deliver_policy: consumer::DeliverPolicy::All,
                ack_policy: consumer::AckPolicy::Explicit,
                max_batch: limit as i64,
                ..Default::default()
            })
            .await
            .map_err(|err| Box::new(EventsError(format!("failed to create consumer: {err}"))) as Box<dyn StdError>)?;

        let mut messages = consumer
            .fetch()
            .max_messages(limit)
            .expires(FETCH_TIMEOUT)
            .messages()
            .await
            .map_err(|err| Box::new(EventsError(format!("failed to fetch: {err}"))) as Box<dyn StdError>)?;

        let mut count = 0;
        while let Some(message) = messages.next().await {
            let message = message.map_err(|err| Box::new(EventsError(format!("message error: {err}"))) as Box<dyn StdError>)?;
            print_event(&message, json_output)?;
            message
                .ack()
                .await
                .map_err(|err| Box::new(EventsError(format!("ack error: {err}"))) as Box<dyn StdError>)?;
            count += 1;
        }

        if count == 0 {
            if json_output {
                println!("[]");
            } else {
                println!("No events found.");
            }
        }

        Ok(())
    }

    pub async fn list_subjects(&self, pubkey: &Pubkey, json_output: bool) {
        let subjects = [
            ("follows", user_follows_subject(pubkey)),
            ("activity", user_activity_subject(pubkey)),
            ("balance", user_balance_subject(pubkey)),
            ("lamports", user_lamports_subject(pubkey)),
        ];

        let global_subject = global_names_subject();

        if json_output {
            let value = serde_json::json!({
                "pubkey": pubkey.to_string(),
                "subjects": {
                    "follows": subjects[0].1.clone(),
                    "activity": subjects[1].1.clone(),
                    "balance": subjects[2].1.clone(),
                    "lamports": subjects[3].1.clone(),
                    "names": global_subject,
                }
            });
            println!("{}", serde_json::to_string_pretty(&value).unwrap());
        } else {
            println!("Subjects for pubkey {}:", pubkey);
            for (name, subject) in &subjects {
                println!("  {name}: {subject}");
            }
            println!("  names: {global_subject}");
        }
    }
}

fn print_event(message: &Message, json_output: bool) -> Result<()> {
    let envelope: EventEnvelope = serde_json::from_slice(&message.payload)
        .map_err(|err| Box::new(EventsError(format!("failed to parse event: {err}"))) as Box<dyn StdError>)?;

    if json_output {
        let json: Value = serde_json::to_value(&envelope)
            .map_err(|err| Box::new(EventsError(format!("failed to serialize: {err}"))) as Box<dyn StdError>)?;
        println!("{}", serde_json::to_string(&json).unwrap());
    } else {
        print_event_human(&envelope);
    }

    Ok(())
}

fn print_event_human(envelope: &EventEnvelope) {
    let event_type = envelope.payload.event_type();
    let sig_short = if envelope.signature.len() > 16 {
        &envelope.signature[..16]
    } else {
        &envelope.signature
    };

    print!("[{event_type}] slot={} sig={}.. ", envelope.slot, sig_short);

    match &envelope.payload {
        EventPayload::FollowCreated(payload) => {
            print!("follower={} target={}", payload.follower, payload.target);
        }
        EventPayload::FollowRemoved(payload) => {
            print!("follower={} target={}", payload.follower, payload.target);
        }
        EventPayload::NameRegistered(payload) => {
            print!("name={} owner={}", payload.name, payload.owner);
        }
        EventPayload::BalanceUpdated(payload) => {
            print!("owner={} amount={}", payload.owner, payload.amount);
        }
        EventPayload::LamportsUpdated(payload) => {
            print!("owner={} lamports={}", payload.owner, payload.lamports);
        }
        EventPayload::ActivityRecorded(payload) => {
            print!("owner={} kind={} counterparty={} amount={}", payload.owner, payload.kind, payload.counterparty, payload.amount);
        }
    }

    println!();
}

pub fn build_subject(pubkey: Option<&Pubkey>, event_type: Option<&str>) -> Result<String> {
    let parsed_type = event_type.and_then(EventType::from_str);

    match (pubkey, parsed_type) {
        (Some(pk), Some(EventType::FollowCreated | EventType::FollowRemoved)) => {
            Ok(user_follows_subject(pk))
        }
        (Some(pk), Some(EventType::BalanceUpdated)) => {
            Ok(user_balance_subject(pk))
        }
        (Some(pk), Some(EventType::LamportsUpdated)) => {
            Ok(user_lamports_subject(pk))
        }
        (Some(pk), Some(EventType::ActivityRecorded)) => {
            Ok(user_activity_subject(pk))
        }
        (Some(_), Some(EventType::NameRegistered)) => {
            Err(Box::new(EventsError(
                "name.registered events are global; omit --pubkey for global events".to_string(),
            )))
        }
        (None, Some(EventType::NameRegistered)) => {
            Ok(global_names_subject().to_string())
        }
        (Some(pk), None) => {
            Ok(format!("surf.user.{}.*", pk))
        }
        (None, None) => {
            Ok("surf.>".to_string())
        }
        (None, Some(_)) => {
            let event_type_str = event_type.unwrap_or("unknown");
            let valid = EventType::all().join(", ");
            Err(Box::new(EventsError(format!(
                "event type '{event_type_str}' requires --pubkey; valid types: {valid}"
            ))))
        }
    }
}