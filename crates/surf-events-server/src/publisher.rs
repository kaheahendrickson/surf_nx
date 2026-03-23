use async_nats::jetstream::{self, Context};
use bytes::Bytes;
use std::future::Future;

use crate::error::EventPublishError;
use surf_events::{subject_for_event, EventEnvelope};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedEvent {
    pub subject: String,
    pub payload: Vec<u8>,
}

pub trait EventPublisher {
    fn publish(
        &self,
        event: &EventEnvelope,
    ) -> impl Future<Output = Result<PublishedEvent, EventPublishError>> + Send;
}

#[derive(Clone)]
pub struct JetStreamPublisher {
    context: Context,
}

impl JetStreamPublisher {
    pub fn new(context: Context) -> Self {
        Self { context }
    }

    pub fn context(&self) -> &Context {
        &self.context
    }

    pub async fn connect(nats_url: &str) -> Result<Self, crate::error::ServerError> {
        let client = async_nats::connect(nats_url)
            .await
            .map_err(|err| crate::error::ServerError::NatsConnect(err.to_string()))?;
        let context = jetstream::new(client);
        Ok(Self::new(context))
    }
}

impl EventPublisher for JetStreamPublisher {
    async fn publish(&self, event: &EventEnvelope) -> Result<PublishedEvent, EventPublishError> {
        let subject = subject_for_event(event);
        let payload = serde_json::to_vec(event)?;
        self.context
            .publish(subject.clone(), Bytes::from(payload.clone()))
            .await
            .map_err(|err| EventPublishError::Publish(err.to_string()))?
            .await
            .map_err(|err| EventPublishError::Publish(err.to_string()))?;

        Ok(PublishedEvent { subject, payload })
    }
}
