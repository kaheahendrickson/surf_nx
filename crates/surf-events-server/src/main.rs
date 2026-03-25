use surf_events_server::{ServerConfig, ServerRuntime};

#[tokio::main]
async fn main() {
    let rpc_url = std::env::var("SURF_TEST_VALIDATOR_URL")
        .or_else(|_| std::env::var("SURF_EVENTS_RPC_URL"))
        .unwrap_or_else(|_| "http://127.0.0.1:8899".to_owned());
    let nats_url = std::env::var("SURF_NATS_URL")
        .or_else(|_| std::env::var("SURF_EVENTS_NATS_URL"))
        .unwrap_or_else(|_| "nats://127.0.0.1:4222".to_owned());
    let stream_name =
        std::env::var("SURF_EVENTS_STREAM").unwrap_or_else(|_| "surf-events".to_owned());
    let token_program = std::env::var("SURF_TOKEN_PROGRAM")
        .or_else(|_| std::env::var("SURF_TOKEN_PROGRAM_ID"))
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or_else(solana_pubkey::Pubkey::new_unique);
    let registry_program = std::env::var("SURF_REGISTRY_PROGRAM")
        .or_else(|_| std::env::var("SURF_REGISTRY_PROGRAM_ID"))
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or_else(solana_pubkey::Pubkey::new_unique);
    let signals_program = std::env::var("SURF_SIGNALS_PROGRAM")
        .or_else(|_| std::env::var("SURF_SIGNALS_PROGRAM_ID"))
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or_else(solana_pubkey::Pubkey::new_unique);
    let poll_interval_ms = std::env::var("SURF_EVENTS_POLL_INTERVAL_MS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1_000);
    let signature_batch_limit = std::env::var("SURF_EVENTS_SIGNATURE_BATCH_LIMIT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(100);
    let checkpoint_path = std::env::var("SURF_EVENTS_CHECKPOINT_PATH")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| surf_events_server::checkpoint::default_checkpoint_path());

    let config = ServerConfig::new(
        rpc_url,
        nats_url,
        stream_name,
        token_program,
        registry_program,
        signals_program,
    )
    .with_poll_interval_ms(poll_interval_ms)
    .with_signature_batch_limit(signature_batch_limit)
    .with_checkpoint_path(checkpoint_path);

    match ServerRuntime::connect(config).await {
        Ok(runtime) => {
            if let Err(err) = runtime.run().await {
                eprintln!("surf-events-server failed: {err}");
                std::process::exit(1);
            }
        }
        Err(err) => {
            eprintln!("surf-events-server failed to start: {err}");
            std::process::exit(1);
        }
    }
}
