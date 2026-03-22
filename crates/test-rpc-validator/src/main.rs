use clap::Parser;
use test_rpc_validator::{run_server, Config};

#[derive(Parser, Debug)]
#[command(name = "rpc-test-validator")]
#[command(about = "A local Solana RPC test validator backed by Mollusk")]
struct Args {
    #[arg(short, long, default_value = "8899")]
    port: u16,

    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    #[arg(long)]
    program: Vec<String>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let config = Config {
        host: args.host,
        port: args.port,
        programs: args.program,
    };

    if let Err(e) = run_server(config).await {
        eprintln!("Server error: {}", e);
        std::process::exit(1);
    }
}
