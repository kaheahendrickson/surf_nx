use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;

use crate::config::{registry_program_id, signals_program_id, token_program_id};
use crate::error::TestWebServicesError;
use crate::programs::{all_program_paths, ensure_sbf_programs_built};

const VALIDATOR_BIN: &str = "test-rpc-validator";
const INITIAL_PORT: u16 = 43_000;
const MAX_PORT_ATTEMPTS: u16 = 100;
const READY_TIMEOUT: Duration = Duration::from_secs(30);
const READY_POLL_INTERVAL: Duration = Duration::from_millis(500);

static NEXT_PORT: AtomicU16 = AtomicU16::new(INITIAL_PORT);

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn find_validator_binary() -> PathBuf {
    let workspace = workspace_root();
    
    let debug_path = workspace.join("target/debug").join(VALIDATOR_BIN);
    if debug_path.exists() {
        return debug_path;
    }
    
    let release_path = workspace.join("target/release").join(VALIDATOR_BIN);
    if release_path.exists() {
        return release_path;
    }
    
    PathBuf::from(VALIDATOR_BIN)
}

#[derive(Debug)]
pub struct RpcValidatorGuard {
    child: Child,
    port: u16,
    url: String,
}

impl RpcValidatorGuard {
    pub async fn start(port: Option<u16>) -> Result<Self, TestWebServicesError> {
        ensure_sbf_programs_built()?;
        
        let (token_path, registry_path, signals_path) = all_program_paths();
        
        let port = port.unwrap_or_else(|| next_port());
        
        for _ in 0..MAX_PORT_ATTEMPTS {
            let mut candidate = Self::spawn(port, &token_path, &registry_path, &signals_path)?;
            match candidate.wait_ready().await {
                Ok(()) => return Ok(candidate),
                Err(TestWebServicesError::ExitedEarly { .. }) => {
                    continue;
                }
                Err(err) => return Err(err),
            }
        }

        Err(TestWebServicesError::ReadyTimeout {
            service: "test-rpc-validator".to_string(),
            timeout: READY_TIMEOUT,
            last_error: format!("exhausted {MAX_PORT_ATTEMPTS} startup attempts"),
            log_path: PathBuf::from("<none>"),
        })
    }

    fn spawn(
        port: u16,
        token_path: &Path,
        registry_path: &Path,
        signals_path: &Path,
    ) -> Result<Self, TestWebServicesError> {
        let token_id = token_program_id();
        let registry_id = registry_program_id();
        let signals_id = signals_program_id();
        
        let validator_bin = find_validator_binary();

        let child = Command::new(validator_bin)
            .args([
                "--port", &port.to_string(),
                "--program", &format!("{}={}", token_id, token_path.display()),
                "--program", &format!("{}={}", registry_id, registry_path.display()),
                "--program", &format!("{}={}", signals_id, signals_path.display()),
            ])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|source| TestWebServicesError::Spawn {
                service: "test-rpc-validator".to_string(),
                source,
            })?;

        let url = format!("http://127.0.0.1:{}", port);

        Ok(Self {
            child,
            port,
            url,
        })
    }

    async fn wait_ready(&mut self) -> Result<(), TestWebServicesError> {
        let started_at = tokio::time::Instant::now();
        let mut last_error = String::from("connection not attempted");

        let client = reqwest::Client::new();

        loop {
            if let Ok(Some(status)) = self.child.try_wait() {
                return Err(TestWebServicesError::ExitedEarly {
                    service: "test-rpc-validator".to_string(),
                    status,
                    log_path: PathBuf::from("<stdout>"),
                });
            }

            if started_at.elapsed() >= READY_TIMEOUT {
                return Err(TestWebServicesError::ReadyTimeout {
                    service: "test-rpc-validator".to_string(),
                    timeout: READY_TIMEOUT,
                    last_error,
                    log_path: PathBuf::from("<stdout>"),
                });
            }

            let body = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "getLatestBlockhash",
                "params": []
            });

            match client
                .post(&self.url)
                .header("Content-Type", "application/json")
                .body(serde_json::to_string(&body).unwrap())
                .timeout(Duration::from_secs(2))
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    return Ok(());
                }
                Ok(resp) => {
                    last_error = format!("HTTP {}", resp.status());
                }
                Err(err) => {
                    last_error = err.to_string();
                }
            }

            tokio::time::sleep(READY_POLL_INTERVAL).await;
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for RpcValidatorGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn next_port() -> u16 {
    NEXT_PORT.fetch_add(1, Ordering::Relaxed)
}