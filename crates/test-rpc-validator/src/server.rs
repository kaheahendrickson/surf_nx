use std::net::SocketAddr;
use std::sync::Arc;

use crate::config::Config;
pub use crate::error::RpcError;
use crate::handlers::handle_rpc;
use axum::routing::post;
use axum::Router;
use surf_client::backend::TestBackend;
use surf_provider_memory::MolluskBackend;
use tower_http::cors::Any;
use tower_http::cors::CorsLayer;

pub struct AppState {
    pub backend: MolluskBackend,
}

pub async fn run_server(config: Config) -> Result<(), RpcError> {
    let backend = MolluskBackend::new();

    let programs = config.parse_programs().map_err(|e| RpcError::Backend(e))?;

    for program in &programs {
        let program_id =
            program
                .program_id
                .parse()
                .map_err(|e: solana_pubkey::ParsePubkeyError| {
                    RpcError::InvalidPubkey(format!("Invalid program ID: {}", e))
                })?;

        let bytes = std::fs::read(&program.path)
            .map_err(|e| RpcError::Backend(format!("Failed to read program file: {}", e)))?;

        backend
            .add_program(&program_id, &bytes)
            .await
            .map_err(|e| RpcError::Backend(format!("Failed to add program: {}", e)))?;

        println!(
            "Loaded program: {} from {}",
            program.program_id,
            program.path.display()
        );
    }

    let state = Arc::new(AppState { backend });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/", post(handle_rpc))
        .layer(cors)
        .with_state(state);

    let addr: SocketAddr = format!("{}:{}", config.host, config.port)
        .parse()
        .map_err(|e| RpcError::Backend(format!("Invalid address: {}", e)))?;

    println!("rpc-test-validator listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| RpcError::Backend(format!("Failed to bind: {}", e)))?;

    axum::serve(listener, app)
        .await
        .map_err(|e| RpcError::Backend(format!("Server error: {}", e)))?;

    Ok(())
}
