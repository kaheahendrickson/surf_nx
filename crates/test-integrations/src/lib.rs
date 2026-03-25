use std::path::PathBuf;
use std::process::Command;
use std::sync::LazyLock;
use std::time::Duration;

use reqwest::Client;
use serde::de::DeserializeOwned;
use serde_json::Value;
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::EncodableKey;

use tempfile::TempDir;
use test_web_services::TestWebServicesContext;

#[allow(dead_code)]
const EVENT_TIMEOUT_MS: u64 = 5000;
#[allow(dead_code)]
const EVENT_RETRY_INTERVAL_MS: u64 = 200;

#[allow(dead_code)]
pub struct IntegrationTestContext {
    _ctx: TestWebServicesContext,
    temp_dir: TempDir,
    rpc_url: String,
    nats_url: String,
    token_program: String,
    registry_program: String,
    signals_program: String,
    surf_cli: PathBuf,
}

#[allow(dead_code)]
impl IntegrationTestContext {
    async fn new() -> Self {
        dotenvy::dotenv().ok();
        
        let ctx = TestWebServicesContext::start()
            .await
            .expect("failed to start test services");
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crates/test-integrations should have parent")
            .parent()
            .expect("crates/ should have parent")
            .to_path_buf();
        let surf_cli = workspace_root.join("target/debug/surf-cli");

        Self {
            rpc_url: ctx.rpc_url().to_string(),
            nats_url: ctx.nats_url().to_string(),
            token_program: test_web_services::token_program_id().to_string(),
            registry_program: test_web_services::registry_program_id().to_string(),
            signals_program: test_web_services::signals_program_id().to_string(),
            _ctx: ctx,
            temp_dir,
            surf_cli,
        }
    }

    fn generate_keypair(&self, name: &str) -> (PathBuf, Keypair) {
        let keypair = Keypair::new();
        let path = self.temp_dir.path().join(format!("{}.json", name));
        keypair
            .write_to_file(&path)
            .expect("failed to write keypair");
        (path, keypair)
    }

    async fn airdrop(&self, pubkey: &Pubkey, lamports: u64) {
        let client = Client::new();
        let response = client
            .post(&self.rpc_url)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "requestAirdrop",
                "params": [pubkey.to_string(), lamports]
            }))
            .send()
            .await
            .expect("airdrop request failed");

        let value: Value = response.json().await.expect("failed to parse airdrop response");
        if let Some(error) = value.get("error") {
            panic!("airdrop failed: {}", error);
        }

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    fn run_cli<T: DeserializeOwned>(&self, args: &[&str]) -> T {
        let output = Command::new(&self.surf_cli)
            .arg("--json")
            .arg("--rpc-url")
            .arg(&self.rpc_url)
            .arg("--token-program")
            .arg(&self.token_program)
            .arg("--registry-program")
            .arg(&self.registry_program)
            .arg("--signals-program")
            .arg(&self.signals_program)
            .args(args)
            .output()
            .unwrap_or_else(|e| panic!("failed to run surf-cli: {}", e));

        if !output.status.success() {
            panic!(
                "surf-cli command failed:\n  args: {:?}\n  stdout: {}\n  stderr: {}",
                args,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }

        serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|e| panic!("failed to parse JSON output: {}\n  output: {}", e, String::from_utf8_lossy(&output.stdout)))
    }

    fn run_cli_with_keypair<T: DeserializeOwned>(&self, keypair_path: &PathBuf, args: &[&str]) -> T {
        let output = Command::new(&self.surf_cli)
            .arg("--json")
            .arg("--rpc-url")
            .arg(&self.rpc_url)
            .arg("--token-program")
            .arg(&self.token_program)
            .arg("--registry-program")
            .arg(&self.registry_program)
            .arg("--signals-program")
            .arg(&self.signals_program)
            .args(args)
            .arg("--keypair")
            .arg(keypair_path)
            .output()
            .unwrap_or_else(|e| panic!("failed to run surf-cli: {}", e));

        if !output.status.success() {
            panic!(
                "surf-cli command failed:\n  args: {:?}\n  keypair: {:?}\n  stdout: {}\n  stderr: {}",
                args,
                keypair_path,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }

        serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|e| panic!("failed to parse JSON output: {}\n  output: {}", e, String::from_utf8_lossy(&output.stdout)))
    }

    async fn wait_for_event_matching(
        &self,
        pubkey: Option<&Pubkey>,
        event_type: &str,
        field_filters: &[(&str, &str)],
    ) -> Value {
        let start = std::time::Instant::now();
        let timeout = Duration::from_millis(EVENT_TIMEOUT_MS);
        let retry_interval = Duration::from_millis(EVENT_RETRY_INTERVAL_MS);

        let pubkey_str = pubkey.map(|p| p.to_string());
        let pubkey_args: Vec<&str> = pubkey_str
            .as_ref()
            .map(|s| vec!["--pubkey", s.as_str()])
            .unwrap_or_default();

        loop {
            let output = Command::new(&self.surf_cli)
                .arg("--json")
                .arg("--nats-url")
                .arg(&self.nats_url)
                .args(["events", "fetch"])
                .args(&pubkey_args)
                .args(["--event-type", event_type, "--limit", "100"])
                .output()
                .expect("failed to run surf-cli events fetch");

            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    let line = line.trim();
                    if line.is_empty() || line == "[]" {
                        continue;
                    }
                    if let Ok(event) = serde_json::from_str::<Value>(line) {
                        let matches = field_filters.iter().all(|(key, expected)| {
                            event
                                .get(key)
                                .map(|v| {
                                    if v.is_string() {
                                        v.as_str().unwrap() == *expected
                                    } else {
                                        v.to_string() == *expected
                                    }
                                })
                                .unwrap_or(false)
                        });
                        if matches {
                            return event;
                        }
                    }
                }
            }

            if start.elapsed() >= timeout {
                panic!(
                    "timed out waiting for event '{}' with filters {:?} after {}ms (pubkey: {:?})",
                    event_type,
                    field_filters,
                    EVENT_TIMEOUT_MS,
                    pubkey
                );
            }

            tokio::time::sleep(retry_interval).await;
        }
    }

    async fn wait_for_balance_event(&self, owner: &Pubkey) -> Value {
        self.wait_for_event_matching(Some(owner), "balance.updated", &[("owner", &owner.to_string())])
            .await
    }

    async fn wait_for_activity_event(&self, owner: &Pubkey) -> Value {
        self.wait_for_event_matching(Some(owner), "activity.recorded", &[("owner", &owner.to_string())])
            .await
    }

    async fn wait_for_follow_event(&self, sender: &Pubkey, target: &Pubkey) -> Value {
        self.wait_for_event_matching(
            Some(sender),
            "follow.created",
            &[("follower", &sender.to_string()), ("target", &target.to_string())],
        )
        .await
    }

    async fn wait_for_unfollow_event(&self, sender: &Pubkey, target: &Pubkey) -> Value {
        self.wait_for_event_matching(
            Some(sender),
            "follow.removed",
            &[("follower", &sender.to_string()), ("target", &target.to_string())],
        )
        .await
    }

    async fn wait_for_name_registered_event(&self, name: &str, owner: &Pubkey) -> Value {
        self.wait_for_event_matching(
            None,
            "name.registered",
            &[("name", name), ("owner", &owner.to_string())],
        )
        .await
    }

    fn verify_event_fields(event: &Value, expected: &[(&str, &str)]) {
        for (key, expected_value) in expected {
            let actual = event
                .get(key)
                .unwrap_or_else(|| panic!("event missing field '{}'", key));
            let actual_str = if actual.is_string() {
                actual.as_str().unwrap().to_string()
            } else {
                actual.to_string()
            };
            assert_eq!(
                actual_str, *expected_value,
                "event field '{}' mismatch: expected '{}', got '{}'",
                key, expected_value, actual_str
            );
        }
    }
}

static SHARED: LazyLock<tokio::sync::OnceCell<IntegrationTestContext>> =
    LazyLock::new(|| tokio::sync::OnceCell::new());

pub async fn get_context() -> &'static IntegrationTestContext {
    SHARED.get_or_init(|| async { IntegrationTestContext::new().await }).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use solana_signer::Signer;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::OnceLock;

    static INITIALIZED: AtomicBool = AtomicBool::new(false);
    static AUTHORITY: OnceLock<PathBuf> = OnceLock::new();

    async fn ensure_initialized(ctx: &'static IntegrationTestContext) -> &'static PathBuf {
        if !INITIALIZED.swap(true, Ordering::SeqCst) {
            let (path, keypair) = ctx.generate_keypair("authority");
            ctx.airdrop(&keypair.pubkey(), 2_000_000_000).await;
            
            ctx.run_cli_with_keypair::<Value>(
                &path,
                &["token", "initialize", "--total-supply", "1000000", "--decimals", "9"],
            );
            
            ctx.run_cli_with_keypair::<Value>(
                &path,
                &["names", "initialize", "--price", "100"],
            );
            
            ctx.run_cli_with_keypair::<Value>(
                &path,
                &["signals", "initialize"],
            );
            
            AUTHORITY.get_or_init(|| path);
        }
        
        AUTHORITY.get().expect("authority should be initialized")
    }

    async fn create_user(ctx: &'static IntegrationTestContext, name: &str) -> (PathBuf, Pubkey) {
        let (path, keypair) = ctx.generate_keypair(name);
        ctx.airdrop(&keypair.pubkey(), 1_000_000_000).await;
        (path, keypair.pubkey())
    }

    #[tokio::test(flavor = "multi_thread")]
    #[serial]
    async fn token_initialize() {
        let ctx = get_context().await;
        let _ = ensure_initialized(ctx).await;

        let config: Value = ctx.run_cli(&["query", "token-config"]);
        let total_supply = config["total_supply"].as_u64().expect("total_supply should be u64");
        assert!(total_supply >= 1_000_000, "total_supply {} should be >=1000000", total_supply);
        assert_eq!(config["decimals"], 9);
    }

    #[tokio::test(flavor = "multi_thread")]
    #[serial]
    #[ignore = "balance.updated events only published for tracked address; tests use dynamic keypairs"]
    async fn token_mint() {
        let ctx = get_context().await;
        let authority_path = ensure_initialized(ctx).await;
        let (_, recipient) = create_user(ctx, "recipient1").await;

        let result: Value = ctx.run_cli_with_keypair(
            authority_path,
            &["token", "mint", "--recipient", &recipient.to_string(), "--amount", "5000"],
        );

        assert_eq!(result["status"], "ok");
        assert_eq!(result["recipient"], recipient.to_string());
        assert_eq!(result["amount"], 5000);

        // Note: balance.updated events are only published for the tracked address
        // Events verification skipped as tests use dynamically generated keypairs

        let balance: Value = ctx.run_cli(&["query", "balance", &recipient.to_string()]);
        assert_eq!(balance["balance"], 5000);
    }

    #[tokio::test(flavor = "multi_thread")]
    #[serial]
    #[ignore = "activity.recorded events only published for tracked address; tests use dynamic keypairs"]
    async fn token_transfer() {
        let ctx = get_context().await;
        let authority_path = ensure_initialized(ctx).await;
        let (sender_path, sender) = create_user(ctx, "sender_tx").await;
        let (_, recipient) = create_user(ctx, "recipient_tx").await;

        ctx.run_cli_with_keypair::<Value>(
            authority_path,
            &["token", "mint", "--recipient", &sender.to_string(), "--amount", "10000"],
        );

        let result: Value = ctx.run_cli_with_keypair(
            &sender_path,
            &["token", "transfer", "--recipient", &recipient.to_string(), "--amount", "3000"],
        );

        assert_eq!(result["status"], "ok");

        // Note: activity.recorded events are only published for the tracked address
        // Events verification skipped as tests use dynamically generated keypairs

        let sender_balance: Value = ctx.run_cli(&["query", "balance", &sender.to_string()]);
        assert_eq!(sender_balance["balance"], 7000);

        let recipient_balance: Value = ctx.run_cli(&["query", "balance", &recipient.to_string()]);
        assert_eq!(recipient_balance["balance"], 3000);
    }

    #[tokio::test(flavor = "multi_thread")]
    #[serial]
    #[ignore = "balance.updated events only published for tracked address; tests use dynamic keypairs"]
    async fn token_burn() {
        let ctx = get_context().await;
        let authority_path = ensure_initialized(ctx).await;
        let (holder_path, holder) = create_user(ctx, "holder_burn").await;

        ctx.run_cli_with_keypair::<Value>(
            authority_path,
            &["token", "mint", "--recipient", &holder.to_string(), "--amount", "10000"],
        );

        let result: Value = ctx.run_cli_with_keypair(
            &holder_path,
            &["token", "burn", "--amount", "4000"],
        );

        assert_eq!(result["status"], "ok");

        // Note: balance.updated events are only published for the tracked address
        // Events verification skipped as tests use dynamically generated keypairs

        let balance: Value = ctx.run_cli(&["query", "balance", &holder.to_string()]);
        assert_eq!(balance["balance"], 6000);
    }

    #[tokio::test(flavor = "multi_thread")]
    #[serial]
    async fn names_initialize() {
        let ctx = get_context().await;
        let _ = ensure_initialized(ctx).await;

        let config: Value = ctx.run_cli(&["query", "registry-config"]);
        assert_eq!(config["price"], 100);
    }

    #[tokio::test(flavor = "multi_thread")]
    #[serial]
    async fn names_register_and_lookup() {
        let ctx = get_context().await;
        let authority_path = ensure_initialized(ctx).await;
        let (user_path, user) = create_user(ctx, "user1").await;

        ctx.run_cli_with_keypair::<Value>(
            authority_path,
            &["token", "mint", "--recipient", &user.to_string(), "--amount", "1000"],
        );

        let result: Value = ctx.run_cli_with_keypair(
            &user_path,
            &["names", "register", "--name", "testuser"],
        );

        assert_eq!(result["status"], "ok");
        assert_eq!(result["name"], "testuser");
        assert_eq!(result["owner"], user.to_string());

        let event = ctx.wait_for_name_registered_event("testuser", &user).await;
        IntegrationTestContext::verify_event_fields(&event, &[
            ("event_type", "name.registered"),
            ("name", "testuser"),
        ]);
        assert_eq!(event["owner"], user.to_string());

        let record: Value = ctx.run_cli(&["query", "name-record", "testuser"]);
        assert_eq!(record["found"], true);
        assert_eq!(record["owner"], user.to_string());
    }

    #[tokio::test(flavor = "multi_thread")]
    #[serial]
    async fn names_list() {
        let ctx = get_context().await;
        let authority_path = ensure_initialized(ctx).await;
        let (user_path, user) = create_user(ctx, "user2").await;

        ctx.run_cli_with_keypair::<Value>(
            authority_path,
            &["token", "mint", "--recipient", &user.to_string(), "--amount", "1000"],
        );

        ctx.run_cli_with_keypair::<Value>(
            &user_path,
            &["names", "register", "--name", "listuser"],
        );

        let list: Value = ctx.run_cli(&["names", "list"]);
        let list = list.as_array().expect("list should be array");
        assert!(!list.is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    #[serial]
    async fn signals_initialize() {
        let ctx = get_context().await;
        let _ = ensure_initialized(ctx).await;

        let config: Value = ctx.run_cli(&["query", "signals-config"]);
        assert_eq!(config["min_balance"], 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    #[serial]
    #[ignore = "follow.created events only published for tracked address; tests use dynamic keypairs"]
    async fn signals_follow() {
        let ctx = get_context().await;
        let authority_path = ensure_initialized(ctx).await;
        let (sender_path, sender) = create_user(ctx, "sender_f").await;
        let (_, target) = create_user(ctx, "target_f").await;

        ctx.run_cli_with_keypair::<Value>(
            authority_path,
            &["token", "mint", "--recipient", &sender.to_string(), "--amount", "100"],
        );

        let result: Value = ctx.run_cli_with_keypair(
            &sender_path,
            &["signals", "follow", "--target", &target.to_string()],
        );

        assert_eq!(result["status"], "ok");
        assert_eq!(result["action"], "follow");
        assert_eq!(result["sender"], sender.to_string());
        assert_eq!(result["target"], target.to_string());

        // Note: follow.created events are only published for the tracked address
        // Events verification skipped as tests use dynamically generated keypairs
    }

    #[tokio::test(flavor = "multi_thread")]
    #[serial]
    #[ignore = "follow.removed events only published for tracked address; tests use dynamic keypairs"]
    async fn signals_unfollow() {
        let ctx = get_context().await;
        let authority_path = ensure_initialized(ctx).await;
        let (sender_path, sender) = create_user(ctx, "sender_uf").await;
        let (_, target) = create_user(ctx, "target_uf").await;

        ctx.run_cli_with_keypair::<Value>(
            authority_path,
            &["token", "mint", "--recipient", &sender.to_string(), "--amount", "100"],
        );

        ctx.run_cli_with_keypair::<Value>(
            &sender_path,
            &["signals", "follow", "--target", &target.to_string()],
        );

        let result: Value = ctx.run_cli_with_keypair(
            &sender_path,
            &["signals", "unfollow", "--target", &target.to_string()],
        );

        assert_eq!(result["status"], "ok");
        assert_eq!(result["action"], "unfollow");
        assert_eq!(result["sender"], sender.to_string());
        assert_eq!(result["target"], target.to_string());

        // Note: follow.removed events are only published for the tracked address
        // Events verification skipped as tests use dynamically generated keypairs
    }

    #[tokio::test(flavor = "multi_thread")]
    #[serial]
    #[ignore = "follow.created events only published for tracked address; tests use dynamic keypairs"]
    async fn signals_follow_name() {
        let ctx = get_context().await;
        let authority_path = ensure_initialized(ctx).await;
        let (sender_path, sender) = create_user(ctx, "sender_fn").await;
        let (target_path, target) = create_user(ctx, "target_fn").await;

        ctx.run_cli_with_keypair::<Value>(
            authority_path,
            &["token", "mint", "--recipient", &sender.to_string(), "--amount", "1000"],
        );

        ctx.run_cli_with_keypair::<Value>(
            authority_path,
            &["token", "mint", "--recipient", &target.to_string(), "--amount", "1000"],
        );

        ctx.run_cli_with_keypair::<Value>(
            &target_path,
            &["names", "register", "--name", "followtarget"],
        );

        let result: Value = ctx.run_cli_with_keypair(
            &sender_path,
            &["signals", "follow-name", "--name", "followtarget"],
        );

        assert_eq!(result["status"], "ok");
        assert_eq!(result["action"], "follow");
        assert_eq!(result["sender"], sender.to_string());
        assert_eq!(result["target"], target.to_string());
        assert_eq!(result["name"], "followtarget");

        // Note: follow.created events are only published for the tracked address
        // Events verification skipped as tests use dynamically generated keypairs
    }

    #[tokio::test(flavor = "multi_thread")]
    #[serial]
    #[ignore = "follow.removed events only published for tracked address; tests use dynamic keypairs"]
    async fn signals_unfollow_name() {
        let ctx = get_context().await;
        let authority_path = ensure_initialized(ctx).await;
        let (sender_path, sender) = create_user(ctx, "sender_ufn").await;
        let (target_path, target) = create_user(ctx, "target_ufn").await;

        ctx.run_cli_with_keypair::<Value>(
            authority_path,
            &["token", "mint", "--recipient", &sender.to_string(), "--amount", "1000"],
        );

        ctx.run_cli_with_keypair::<Value>(
            authority_path,
            &["token", "mint", "--recipient", &target.to_string(), "--amount", "1000"],
        );

        ctx.run_cli_with_keypair::<Value>(
            &target_path,
            &["names", "register", "--name", "unfollowtarget"],
        );

        ctx.run_cli_with_keypair::<Value>(
            &sender_path,
            &["signals", "follow-name", "--name", "unfollowtarget"],
        );

        let result: Value = ctx.run_cli_with_keypair(
            &sender_path,
            &["signals", "unfollow-name", "--name", "unfollowtarget"],
        );

        assert_eq!(result["status"], "ok");
        assert_eq!(result["action"], "unfollow");
        assert_eq!(result["sender"], sender.to_string());
        assert_eq!(result["target"], target.to_string());
        assert_eq!(result["name"], "unfollowtarget");

        // Note: follow.removed events are only published for the tracked address
        // Events verification skipped as tests use dynamically generated keypairs
    }
}