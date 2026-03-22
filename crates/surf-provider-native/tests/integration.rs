use std::process::Child;
use std::process::Command;
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;

use reqwest::Client;
use solana_hash::Hash;
use solana_keypair::{EncodableKey, Keypair};
use solana_message::Message;
use solana_native_token::LAMPORTS_PER_SOL;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use solana_signer::Signer;
use solana_system_interface::instruction::transfer;
use solana_transaction::Transaction;

use surf_client::backend::{Backend, SignaturesForAddressOptions};
use surf_http_backend_config::HttpBackendConfig;
use surf_provider_native::HttpBackend;

struct ValidatorGuard {
    child: Child,
}

struct TestContext {
    _validator: ValidatorGuard,
    url: String,
    backend: HttpBackend,
}

const HOST: &str = "127.0.0.1";
const INITIAL_PORT: u16 = 38_000;
const MAX_PORT_ATTEMPTS: u16 = 100;
static NEXT_PORT: AtomicU16 = AtomicU16::new(INITIAL_PORT);

impl ValidatorGuard {
    fn start(port: u16) -> Self {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let binary_path = format!("{}/../../target/debug/test-rpc-validator", manifest_dir);

        let child = Command::new(&binary_path)
            .args(["--port", &port.to_string(), "--host", HOST])
            .spawn()
            .expect("Failed to start test-rpc-validator");

        Self { child }
    }

    async fn wait_ready(&mut self, url: &str) -> bool {
        let client = Client::new();

        for _ in 0..30 {
            let body = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "getLatestBlockhash",
                "params": []
            });

            if client
                .post(url)
                .json(&body)
                .timeout(Duration::from_secs(2))
                .send()
                .await
                .is_ok()
            {
                tokio::time::sleep(Duration::from_millis(100)).await;
                return true;
            }

            if let Ok(Some(_)) = self.child.try_wait() {
                return false;
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        false
    }
}

impl Drop for ValidatorGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn next_port() -> u16 {
    NEXT_PORT.fetch_add(1, Ordering::Relaxed)
}

async fn setup() -> TestContext {
    for _ in 0..MAX_PORT_ATTEMPTS {
        let port = next_port();
        let url = format!("http://{HOST}:{port}");
        let mut validator = ValidatorGuard::start(port);

        if validator.wait_ready(&url).await {
            return TestContext {
                _validator: validator,
                url: url.clone(),
                backend: HttpBackend::from_config(HttpBackendConfig::new(&url)),
            };
        }
    }

    panic!("Failed to allocate a test validator port after {MAX_PORT_ATTEMPTS} attempts");
}

fn load_test_keypair() -> Keypair {
    let path = dirs::home_dir()
        .expect("Could not find home directory")
        .join(".config/solana/id.json");
    Keypair::read_from_file(path).expect("Failed to load test keypair")
}

async fn airdrop(url: &str, pubkey: &Pubkey, lamports: u64) {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "requestAirdrop",
        "params": [pubkey.to_string(), lamports]
    });

    let client = Client::new();
    client
        .post(url)
        .json(&body)
        .send()
        .await
        .expect("Airdrop request failed");

    tokio::time::sleep(Duration::from_millis(100)).await;
}

#[tokio::test]
async fn test_get_latest_blockhash() {
    let ctx = setup().await;
    let result = ctx.backend.get_latest_blockhash().await;
    assert!(result.is_ok());
    let hash = result.unwrap();
    assert_ne!(hash, Hash::default());
}

#[tokio::test]
async fn test_get_account_nonexistent() {
    let ctx = setup().await;
    let nonexistent = Pubkey::new_unique();
    let result = ctx.backend.get_account(&nonexistent).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[tokio::test]
async fn test_get_balance_nonexistent() {
    let ctx = setup().await;
    let nonexistent = Pubkey::new_unique();
    let result = ctx.backend.get_balance(&nonexistent).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[tokio::test]
async fn test_get_balance_funded_account() {
    let ctx = setup().await;
    let keypair = load_test_keypair();
    let pubkey = keypair.pubkey();

    airdrop(&ctx.url, &pubkey, LAMPORTS_PER_SOL).await;

    let result = ctx.backend.get_balance(&pubkey).await;
    assert!(result.is_ok());
    let balance = result.unwrap();
    assert!(balance.is_some());
    assert!(balance.unwrap() >= LAMPORTS_PER_SOL);
}

#[tokio::test]
async fn test_minimum_balance_for_rent_exemption() {
    let ctx = setup().await;
    let result = ctx.backend.minimum_balance_for_rent_exemption(100).await;
    assert!(result.is_ok());
    let rent = result.unwrap();
    assert!(rent > 0);
}

#[tokio::test]
async fn test_get_program_accounts() {
    let ctx = setup().await;
    let system_program: Pubkey = "11111111111111111111111111111111".parse().unwrap();
    let result = ctx
        .backend
        .get_program_accounts(&system_program, None)
        .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_get_signatures_for_address_limit() {
    let ctx = setup().await;
    let keypair = load_test_keypair();
    let pubkey = keypair.pubkey();

    airdrop(&ctx.url, &pubkey, LAMPORTS_PER_SOL).await;

    let options = SignaturesForAddressOptions {
        limit: Some(1),
        ..Default::default()
    };

    let result = ctx
        .backend
        .get_signatures_for_address(&pubkey, Some(options))
        .await;
    assert!(result.is_ok());
    let sigs = result.unwrap();
    assert!(sigs.len() <= 1);
}

#[tokio::test]
async fn test_get_transaction_nonexistent() {
    let ctx = setup().await;
    use solana_signature::SIGNATURE_BYTES;
    let sig_bytes = [0u8; SIGNATURE_BYTES];
    let nonexistent_sig = Signature::from(sig_bytes);
    let result = ctx.backend.get_transaction(&nonexistent_sig).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[tokio::test]
async fn test_send_and_confirm() {
    let ctx = setup().await;
    let keypair = load_test_keypair();
    let payer_pubkey = keypair.pubkey();

    airdrop(&ctx.url, &payer_pubkey, 2 * LAMPORTS_PER_SOL).await;

    let blockhash = ctx
        .backend
        .get_latest_blockhash()
        .await
        .expect("Failed to get blockhash");

    let recipient = Pubkey::new_unique();
    let instruction = transfer(&payer_pubkey, &recipient, LAMPORTS_PER_SOL);

    let message = Message::new_with_blockhash(&[instruction], None, &blockhash);

    let mut tx = Transaction::new_unsigned(message);
    tx.sign(&[&keypair], blockhash);

    let result = ctx.backend.send_and_confirm(&tx).await;
    assert!(result.is_ok());
    let sig = result.unwrap();
    assert_ne!(sig, Signature::default());

    tokio::time::sleep(Duration::from_millis(100)).await;
    let recipient_balance = ctx.backend.get_balance(&recipient).await;
    assert!(recipient_balance.is_ok());
    assert_eq!(recipient_balance.unwrap(), Some(LAMPORTS_PER_SOL));
}
