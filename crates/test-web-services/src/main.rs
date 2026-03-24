use std::path::PathBuf;

use clap::Parser;
use test_web_services::{TestWebServicesContext, WebServicesConfig};

#[derive(Parser, Debug)]
#[command(name = "test-web-services")]
#[command(about = "Start test-rpc-validator, NATS, and surf-events-server for integration testing")]
struct Args {
    #[arg(short, long, default_value = "8899")]
    rpc_port: u16,

    #[arg(short, long, default_value = "4222")]
    nats_port: u16,

    #[arg(long, default_value = "surf-events")]
    stream: String,

    #[arg(long)]
    checkpoint_path: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();let args = Args::parse();

    println!("Starting test-web-services...");

    let mut config = WebServicesConfig::new()
        .with_rpc_port(args.rpc_port)
        .with_nats_port(args.nats_port);

    if let Some(path) = args.checkpoint_path {
        config = config.with_checkpoint_path(path);
    }

    let ctx = match TestWebServicesContext::start_with_config(config).await {
        Ok(ctx) => ctx,
        Err(err) => {
            eprintln!("Failed to start services: {}", err);
            std::process::exit(1);
        }
    };

    println!("Services started:");
    println!("  RPC URL: {}", ctx.rpc_url());
    println!("  NATS URL: {}", ctx.nats_url());
    println!("");
println!("Program IDs:");
println!("  Token: {}", test_web_services::token_program_id());
println!("  Registry: {}", test_web_services::registry_program_id());
println!("  Signals: {}", test_web_services::signals_program_id());
println!("  Tracked: {}", test_web_services::tracked_address());
println!("");
println!("Press Ctrl+C to stop...");

    wait_for_shutdown().await;
    drop(ctx);
    println!("Services stopped.");
}

async fn wait_for_shutdown() {
    tokio::signal::ctrl_c().await.expect("Failed to install Ctrl+C handler");
}