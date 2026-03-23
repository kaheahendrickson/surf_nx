use std::time::Duration;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_nats::jetstream::{consumer, stream};
use bytes::Bytes;
use futures::StreamExt;
use test_nats_server::{NatsServerGuard, NatsTestContext};

static NEXT_NAME: AtomicUsize = AtomicUsize::new(0);

fn unique_name(prefix: &str) -> String {
    format!("{prefix}_{}_{}", std::process::id(), NEXT_NAME.fetch_add(1, Ordering::Relaxed))
}

#[tokio::test]
async fn starts_server_and_connects() {
    let server = NatsServerGuard::start()
        .await
        .expect("nats-server should start successfully");
    let context = NatsTestContext::connect(server)
        .await
        .expect("context should connect to running server");

    assert!(context.url().starts_with("nats://127.0.0.1:"));
    assert!(context.server().port() >= 41_000);
}

#[tokio::test]
async fn creates_memory_backed_stream() {
    let server = NatsServerGuard::start().await.expect("nats-server should start");
    let context = NatsTestContext::connect(server)
        .await
        .expect("context should connect");
    let stream_name = unique_name("surf_events_create");

    let mut created = context
        .create_memory_stream(&stream_name, &["surf.>"])
        .await
        .expect("memory stream should be created");
    let info = created.info().await.expect("stream info should be available");

    assert_eq!(info.config.name, stream_name);
    assert_eq!(info.config.storage, stream::StorageType::Memory);
}

#[tokio::test]
async fn publishes_and_consumes_a_message() {
    let server = NatsServerGuard::start().await.expect("nats-server should start");
    let context = NatsTestContext::connect(server)
        .await
        .expect("context should connect");
    let stream_name = unique_name("surf_events_publish");
    let stream = context
        .create_memory_stream(&stream_name, &["surf.>"])
        .await
        .expect("memory stream should be created");

    context
        .jetstream()
        .publish("surf.user.test.follows", Bytes::from_static(b"hello"))
        .await
        .expect("publish should be accepted")
        .await
        .expect("publish ack should succeed");

    let consumer_name = unique_name("consumer");
    let consumer = stream
        .get_or_create_consumer(
            &consumer_name,
            consumer::pull::Config {
                durable_name: Some(consumer_name.clone()),
                ..Default::default()
            },
        )
        .await
        .expect("consumer should be created");

    let mut messages = consumer.messages().await.expect("messages should open");
    let message = tokio::time::timeout(Duration::from_secs(5), messages.next())
        .await
        .expect("message receive should not time out")
        .expect("stream should yield one message")
        .expect("message should decode");

    assert_eq!(message.subject.as_str(), "surf.user.test.follows");
    assert_eq!(message.payload.as_ref(), b"hello");
    message.ack().await.expect("ack should succeed");
}

#[tokio::test]
async fn stream_info_reports_memory_storage() {
    let server = NatsServerGuard::start().await.expect("nats-server should start");
    let context = NatsTestContext::connect(server)
        .await
        .expect("context should connect");
    let stream_name = unique_name("surf_events_storage");

    let mut stream = context
        .create_memory_stream(&stream_name, &["surf.>"])
        .await
        .expect("memory stream should be created");
    let info = stream.info().await.expect("stream info should be available");

    assert_eq!(info.config.storage, stream::StorageType::Memory);
}
