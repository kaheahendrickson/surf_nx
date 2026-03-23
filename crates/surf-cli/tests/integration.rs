use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::Once;
use std::time::Duration;

use reqwest::Client;
use serde_json::Value;
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::{EncodableKey, Signer};
use tempfile::TempDir;

const HOST: &str = "127.0.0.1";
const STARTUP_ATTEMPTS: usize = 20;
static BUILD_VALIDATOR: Once = Once::new();

struct ValidatorGuard {
    child: Child,
}

struct TestContext {
    _validator: ValidatorGuard,
    _temp_dir: TempDir,
    rpc_url: String,
    config_path: PathBuf,
    signer_pubkey: Pubkey,
}

impl ValidatorGuard {
    fn start(
        port: u16,
        token_program: &Pubkey,
        registry_program: &Pubkey,
        signals_program: &Pubkey,
    ) -> Self {
        let binary_path = validator_binary_path();
        let deploy_dir = deploy_dir();

        let child = Command::new(binary_path)
            .args([
                "--host",
                HOST,
                "--port",
                &port.to_string(),
                "--program",
                &format!(
                    "{}={}",
                    token_program,
                    deploy_dir.join("sbf_surf_token.so").display()
                ),
                "--program",
                &format!(
                    "{}={}",
                    registry_program,
                    deploy_dir.join("sbf_surf_name_registry.so").display()
                ),
                "--program",
                &format!(
                    "{}={}",
                    signals_program,
                    deploy_dir.join("sbf_surf_signals.so").display()
                ),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to start rpc-test-validator");

        Self { child }
    }

    async fn wait_ready(&mut self, url: &str) -> bool {
        let client = Client::new();

        for _ in 0..40 {
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

            tokio::time::sleep(Duration::from_millis(250)).await;
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

#[tokio::test]
#[ignore = "requires built SBF artifacts and rpc-test-validator"]
async fn config_show_prints_resolved_values() {
    let ctx = setup().await;

    let output = run_cli_success(&ctx.config_path, &["config", "show"]);

    assert!(output.contains("rpc_url:"));
    assert!(output.contains(&ctx.rpc_url));
    assert!(output.contains("token_program:"));
    assert!(output.contains("registry_program:"));
    assert!(output.contains("signals_program:"));
    assert!(output.contains("keypair:"));
}

#[tokio::test]
#[ignore = "requires built SBF artifacts and rpc-test-validator"]
async fn token_and_registry_initialize_are_queryable() {
    let ctx = setup().await;

    bootstrap_surf_state(&ctx).await;

    let token_output = run_cli_success(&ctx.config_path, &["query", "token-config"]);
    assert!(token_output.contains("authority: "));
    assert!(token_output.contains("total_supply: 1000"));
    assert!(token_output.contains("decimals: 9"));

    let registry_output = run_cli_success(&ctx.config_path, &["query", "registry-config"]);
    assert!(registry_output.contains("price: 50"));
    assert!(registry_output.contains("token_program: "));

    let signals_output = run_cli_success(&ctx.config_path, &["query", "signals-config"]);
    assert!(signals_output.contains("authority: "));
    assert!(signals_output.contains("token_program: "));
    assert!(signals_output.contains("min_balance: 1"));
}

#[tokio::test]
#[ignore = "requires built SBF artifacts and rpc-test-validator"]
async fn follow_and_unfollow_succeed_against_ephemeral_validator() {
    let ctx = setup().await;
    let target = Keypair::new();

    bootstrap_surf_state(&ctx).await;
    request_airdrop(&ctx.rpc_url, &target.pubkey(), 1_000_000_000)
        .await
        .expect("target airdrop should succeed");

    let follow_output = run_cli_success(
        &ctx.config_path,
        &[
            "signals",
            "follow",
            "--target",
            &target.pubkey().to_string(),
        ],
    );
    assert!(follow_output.contains("status: ok"));
    assert!(follow_output.contains("action: follow"));
    assert!(follow_output.contains(&format!("sender: {}", ctx.signer_pubkey)));
    assert!(follow_output.contains(&format!("target: {}", target.pubkey())));

    let unfollow_output = run_cli_success(
        &ctx.config_path,
        &[
            "signals",
            "unfollow",
            "--target",
            &target.pubkey().to_string(),
        ],
    );
    assert!(unfollow_output.contains("status: ok"));
    assert!(unfollow_output.contains("action: unfollow"));
    assert!(unfollow_output.contains(&format!("sender: {}", ctx.signer_pubkey)));
    assert!(unfollow_output.contains(&format!("target: {}", target.pubkey())));
}

#[tokio::test]
#[ignore = "requires built SBF artifacts and rpc-test-validator"]
async fn follow_and_unfollow_by_name_succeed_against_ephemeral_validator() {
    let ctx = setup().await;
    let target = Keypair::new();
    let target_keypair_path = ctx._temp_dir.path().join("target.json");
    let target_name = "targetuser";

    target
        .write_to_file(&target_keypair_path)
        .expect("target keypair should be written");

    bootstrap_surf_state(&ctx).await;
    request_airdrop(&ctx.rpc_url, &target.pubkey(), 1_000_000_000)
        .await
        .expect("target airdrop should succeed");
    wait_for_balance(&ctx.rpc_url, &target.pubkey(), 1_000_000_000)
        .await
        .expect("target airdrop should land");

    run_cli_success(
        &ctx.config_path,
        &[
            "token",
            "mint",
            "--recipient",
            &target.pubkey().to_string(),
            "--amount",
            "1000",
        ],
    );
    run_cli_success(
        &ctx.config_path,
        &[
            "names",
            "register",
            "--keypair",
            target_keypair_path.to_str().expect("path should be utf-8"),
            "--name",
            target_name,
        ],
    );

    let follow_output = run_cli_success(
        &ctx.config_path,
        &["signals", "follow-name", "--name", target_name],
    );
    assert!(follow_output.contains("status: ok"));
    assert!(follow_output.contains("action: follow"));
    assert!(follow_output.contains(&format!("sender: {}", ctx.signer_pubkey)));
    assert!(follow_output.contains(&format!("target: {}", target.pubkey())));
    assert!(follow_output.contains(&format!("name: {target_name}")));

    let unfollow_output = run_cli_success(
        &ctx.config_path,
        &["signals", "unfollow-name", "--name", target_name],
    );
    assert!(unfollow_output.contains("status: ok"));
    assert!(unfollow_output.contains("action: unfollow"));
    assert!(unfollow_output.contains(&format!("sender: {}", ctx.signer_pubkey)));
    assert!(unfollow_output.contains(&format!("target: {}", target.pubkey())));
    assert!(unfollow_output.contains(&format!("name: {target_name}")));
}

#[tokio::test]
#[ignore = "requires built SBF artifacts and rpc-test-validator"]
async fn signals_commands_emit_expected_json() {
    let ctx = setup().await;
    let target = Keypair::new();
    let target_keypair_path = ctx._temp_dir.path().join("json-target.json");
    let target_name = "jsonfriend";

    target
        .write_to_file(&target_keypair_path)
        .expect("target keypair should be written");

    bootstrap_surf_state(&ctx).await;
    request_airdrop(&ctx.rpc_url, &target.pubkey(), 1_000_000_000)
        .await
        .expect("target airdrop should succeed");
    wait_for_balance(&ctx.rpc_url, &target.pubkey(), 1_000_000_000)
        .await
        .expect("target airdrop should land");

    run_cli_success(
        &ctx.config_path,
        &[
            "token",
            "mint",
            "--recipient",
            &target.pubkey().to_string(),
            "--amount",
            "1000",
        ],
    );
    run_cli_success(
        &ctx.config_path,
        &[
            "names",
            "register",
            "--keypair",
            target_keypair_path.to_str().expect("path should be utf-8"),
            "--name",
            target_name,
        ],
    );

    let config_json = run_cli_json_success(&ctx.config_path, &["query", "signals-config"]);
    assert_eq!(config_json["min_balance"], 1);
    assert_eq!(config_json["authority"], ctx.signer_pubkey.to_string());

    let follow_json = run_cli_json_success(
        &ctx.config_path,
        &["signals", "follow-name", "--name", target_name],
    );
    assert_eq!(follow_json["status"], "ok");
    assert_eq!(follow_json["action"], "follow");
    assert_eq!(follow_json["sender"], ctx.signer_pubkey.to_string());
    assert_eq!(follow_json["target"], target.pubkey().to_string());
    assert_eq!(follow_json["name"], target_name);

    let unfollow_json = run_cli_json_success(
        &ctx.config_path,
        &["signals", "unfollow-name", "--name", target_name],
    );
    assert_eq!(unfollow_json["status"], "ok");
    assert_eq!(unfollow_json["action"], "unfollow");
    assert_eq!(unfollow_json["sender"], ctx.signer_pubkey.to_string());
    assert_eq!(unfollow_json["target"], target.pubkey().to_string());
    assert_eq!(unfollow_json["name"], target_name);
}

#[tokio::test]
#[ignore = "requires built SBF artifacts and rpc-test-validator"]
async fn follow_name_reports_clear_errors() {
    let ctx = setup().await;

    bootstrap_surf_state(&ctx).await;
    run_cli_success(
        &ctx.config_path,
        &["names", "register", "--name", "selfname"],
    );

    let missing = run_cli_failure(
        &ctx.config_path,
        &["signals", "follow-name", "--name", "missingname"],
    );
    assert!(missing.contains("name 'missingname' was not found in the SURF registry"));

    let self_follow = run_cli_failure(
        &ctx.config_path,
        &["signals", "follow-name", "--name", "selfname"],
    );
    assert!(
        self_follow.contains("name 'selfname' resolves to your own account; use another user name")
    );
}

#[tokio::test]
#[ignore = "requires built SBF artifacts and rpc-test-validator"]
async fn names_list_is_empty_before_any_registration() {
    let ctx = setup().await;

    bootstrap_surf_state(&ctx).await;

    let list_output = run_cli_success(&ctx.config_path, &["names", "list"]);
    assert_eq!(list_output.trim(), "no names found");
}

#[tokio::test]
#[ignore = "requires built SBF artifacts and rpc-test-validator"]
async fn register_and_list_names_against_ephemeral_validator() {
    let ctx = setup().await;

    bootstrap_surf_state(&ctx).await;

    run_cli_success(&ctx.config_path, &["names", "register", "--name", "foobar"]);
    run_cli_success(&ctx.config_path, &["names", "register", "--name", "what"]);

    let list_output = run_cli_success(&ctx.config_path, &["names", "list"]);
    assert!(list_output.contains("foobar ->"));
    assert!(list_output.contains("what ->"));
}

#[tokio::test]
#[ignore = "requires built SBF artifacts and rpc-test-validator"]
async fn register_and_lookup_name_via_both_commands() {
    let ctx = setup().await;

    bootstrap_surf_state(&ctx).await;

    let register_output =
        run_cli_success(&ctx.config_path, &["names", "register", "--name", "foobar"]);
    assert!(register_output.contains("status: ok"));
    assert!(register_output.contains("name: foobar"));
    assert!(register_output.contains(&format!("owner: {}", ctx.signer_pubkey)));

    let lookup_output = run_cli_success(&ctx.config_path, &["names", "lookup", "foobar"]);
    assert!(lookup_output.contains("found: true"));
    assert!(lookup_output.contains("name: foobar"));
    assert!(lookup_output.contains(&format!("owner: {}", ctx.signer_pubkey)));

    let query_output = run_cli_success(
        &ctx.config_path,
        &["query", "name-record", "foobar"],
    );
    assert!(query_output.contains("found: true"));
    assert!(query_output.contains("name: foobar"));
    assert!(query_output.contains(&format!("owner: {}", ctx.signer_pubkey)));
}

#[tokio::test]
#[ignore = "requires built SBF artifacts and rpc-test-validator"]
async fn registering_name_updates_paid_balance() {
    let ctx = setup().await;

    bootstrap_surf_state(&ctx).await;

    run_cli_success(&ctx.config_path, &["names", "register", "--name", "foobar"]);

    let balance_output = run_cli_success(
        &ctx.config_path,
        &[
            "query",
            "balance",
            "--owner",
            &ctx.signer_pubkey.to_string(),
        ],
    );
    assert!(balance_output.contains("balance: 950"));
}

#[tokio::test]
#[ignore = "requires built SBF artifacts and rpc-test-validator"]
async fn register_new_name_succeeds_against_ephemeral_validator() {
    let ctx = setup().await;

    bootstrap_surf_state(&ctx).await;

    let register_output =
        run_cli_success(&ctx.config_path, &["names", "register", "--name", "foobar"]);
    assert!(register_output.contains("status: ok"));
    assert!(register_output.contains("name: foobar"));
    assert!(register_output.contains(&format!("owner: {}", ctx.signer_pubkey)));
}

async fn bootstrap_surf_state(ctx: &TestContext) {
    request_airdrop(&ctx.rpc_url, &ctx.signer_pubkey, 1_000_000_000)
        .await
        .expect("airdrop should succeed");
    wait_for_balance(&ctx.rpc_url, &ctx.signer_pubkey, 1_000_000_000)
        .await
        .expect("airdrop should land");

    run_cli_success(
        &ctx.config_path,
        &[
            "token",
            "initialize",
            "--total-supply",
            "1000",
            "--decimals",
            "9",
        ],
    );
    run_cli_success(&ctx.config_path, &["names", "initialize", "--price", "50"]);
    run_cli_success(&ctx.config_path, &["signals", "initialize"]);
    run_cli_success(
        &ctx.config_path,
        &[
            "token",
            "mint",
            "--recipient",
            &ctx.signer_pubkey.to_string(),
            "--amount",
            "1000",
        ],
    );
}

async fn setup() -> TestContext {
    ensure_validator_binary_built();

    let token_program = read_program_pubkey("sbf_surf_token");
    let registry_program = read_program_pubkey("sbf_surf_name_registry");
    let signals_program = read_program_pubkey("sbf_surf_signals");
    let signer_pubkey = load_test_keypair().pubkey();
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let config_path = temp_dir.path().join("config.json");

    for _ in 0..STARTUP_ATTEMPTS {
        let port = free_port();
        let rpc_url = format!("http://{HOST}:{port}");
        let mut validator =
            ValidatorGuard::start(port, &token_program, &registry_program, &signals_program);

        if validator.wait_ready(&rpc_url).await {
            std::fs::write(
                &config_path,
                serde_json::json!({
                    "rpc_url": rpc_url,
                    "token_program": token_program.to_string(),
                    "registry_program": registry_program.to_string(),
                    "signals_program": signals_program.to_string(),
                    "keypair": test_keypair_path().display().to_string(),
                })
                .to_string(),
            )
            .expect("failed to write temp config");

            return TestContext {
                _validator: validator,
                _temp_dir: temp_dir,
                rpc_url,
                config_path,
                signer_pubkey,
            };
        }
    }

    panic!("failed to start rpc-test-validator on a free port");
}

fn ensure_validator_binary_built() {
    BUILD_VALIDATOR.call_once(|| {
        let status = Command::new("cargo")
            .args(["build", "-p", "rpc-test-validator"])
            .current_dir(workspace_root())
            .status()
            .expect("failed to build rpc-test-validator");
        assert!(status.success(), "building rpc-test-validator failed");
    });
}

fn run_cli_success(config_path: &Path, args: &[&str]) -> String {
    let mut command = Command::new(env!("CARGO_BIN_EXE_surf-cli"));
    command.arg("--config").arg(config_path);
    command.args(args);

    let output = command.output().expect("failed to run surf-cli");
    assert_success(&output, args);

    String::from_utf8(output.stdout).expect("stdout should be utf-8")
}

fn run_cli_json_success(config_path: &Path, args: &[&str]) -> Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_surf-cli"));
    command.arg("--config").arg(config_path).arg("--json");
    command.args(args);

    let output = command.output().expect("failed to run surf-cli");
    assert_success(&output, args);

    serde_json::from_slice(&output.stdout).expect("stdout should be valid json")
}

fn run_cli_failure(config_path: &Path, args: &[&str]) -> String {
    let mut command = Command::new(env!("CARGO_BIN_EXE_surf-cli"));
    command.arg("--config").arg(config_path);
    command.args(args);

    let output = command.output().expect("failed to run surf-cli");
    assert!(
        !output.status.success(),
        "command {:?} unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stderr).expect("stderr should be utf-8")
}

fn assert_success(output: &Output, args: &[&str]) {
    assert!(
        output.status.success(),
        "command {:?} failed\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("surf-cli should live under workspace root")
        .to_path_buf()
}

fn validator_binary_path() -> PathBuf {
    let path = workspace_root().join("target/debug/rpc-test-validator");
    assert!(
        path.exists(),
        "missing rpc-test-validator binary at {}; run `cargo build -p rpc-test-validator`",
        path.display()
    );
    path
}

fn deploy_dir() -> PathBuf {
    let path = workspace_root().join("target/deploy");
    assert!(
        path.join("sbf_surf_token.so").exists()
            && path.join("sbf_surf_name_registry.so").exists()
            && path.join("sbf_surf_signals.so").exists(),
        "missing deploy artifacts in {}; build them first",
        path.display()
    );
    path
}

fn read_program_pubkey(name: &str) -> Pubkey {
    let path = deploy_dir().join(format!("{name}-keypair.json"));
    Keypair::read_from_file(&path)
        .unwrap_or_else(|err| panic!("failed to read keypair {}: {err}", path.display()))
        .pubkey()
}

fn test_keypair_path() -> PathBuf {
    PathBuf::from(std::env::var("HOME").expect("HOME should be set")).join(".config/solana/id.json")
}

fn load_test_keypair() -> Keypair {
    Keypair::read_from_file(test_keypair_path()).expect("failed to load test keypair")
}

fn free_port() -> u16 {
    TcpListener::bind((HOST, 0))
        .expect("failed to bind ephemeral port")
        .local_addr()
        .expect("failed to read local addr")
        .port()
}

async fn request_airdrop(rpc_url: &str, pubkey: &Pubkey, lamports: u64) -> Result<(), String> {
    let client = Client::new();
    let response = client
        .post(rpc_url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "requestAirdrop",
            "params": [pubkey.to_string(), lamports]
        }))
        .send()
        .await
        .map_err(|err| err.to_string())?;

    let value: Value = response.json().await.map_err(|err| err.to_string())?;
    if let Some(error) = value.get("error") {
        return Err(error.to_string());
    }

    Ok(())
}

async fn wait_for_balance(
    rpc_url: &str,
    pubkey: &Pubkey,
    minimum_balance: u64,
) -> Result<(), String> {
    let client = Client::new();

    for _ in 0..40 {
        let response = client
            .post(rpc_url)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "getBalance",
                "params": [pubkey.to_string()]
            }))
            .send()
            .await
            .map_err(|err| err.to_string())?;

        let value: Value = response.json().await.map_err(|err| err.to_string())?;
        let balance = value
            .get("result")
            .and_then(|result| result.get("value"))
            .and_then(Value::as_u64)
            .unwrap_or_default();

        if balance >= minimum_balance {
            return Ok(());
        }

        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    Err(format!("timed out waiting for balance on {pubkey}"))
}
