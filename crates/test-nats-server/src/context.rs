use async_nats::jetstream::{self, stream, Context};

use crate::error::NatsServerTestError;
use crate::server::NatsServerGuard;

#[derive(Debug)]
pub struct NatsTestContext {
    server: NatsServerGuard,
    client: async_nats::Client,
    jetstream: Context,
}

impl NatsTestContext {
    pub async fn connect(server: NatsServerGuard) -> Result<Self, NatsServerTestError> {
        let client = async_nats::connect(server.url())
            .await
            .map_err(|source| NatsServerTestError::Connect {
                url: server.url().to_owned(),
                source,
            })?;
        let jetstream = jetstream::new(client.clone());

        Ok(Self {
            server,
            client,
            jetstream,
        })
    }

    pub async fn create_memory_stream(
        &self,
        name: &str,
        subjects: &[&str],
    ) -> Result<stream::Stream, NatsServerTestError> {
        self.jetstream
            .get_or_create_stream(stream::Config {
                name: name.to_owned(),
                subjects: subjects.iter().map(|subject| (*subject).to_owned()).collect(),
                storage: stream::StorageType::Memory,
                ..Default::default()
            })
            .await
            .map_err(|err| NatsServerTestError::CreateStream {
                name: name.to_owned(),
                message: err.to_string(),
            })
    }

    pub fn url(&self) -> &str {
        self.server.url()
    }

    pub fn server(&self) -> &NatsServerGuard {
        &self.server
    }

    pub fn client(&self) -> &async_nats::Client {
        &self.client
    }

    pub fn jetstream(&self) -> &Context {
        &self.jetstream
    }
}
