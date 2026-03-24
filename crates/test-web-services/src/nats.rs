use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;

use crate::error::TestWebServicesError;

const NATS_SERVER_BIN: &str = "/usr/local/bin/nats-server";
const INITIAL_PORT: u16 = 42_000;
const MAX_PORT_ATTEMPTS: u16 = 100;
const READY_TIMEOUT: Duration = Duration::from_secs(15);
const READY_POLL_INTERVAL: Duration = Duration::from_millis(250);

static NEXT_PORT: AtomicU16 = AtomicU16::new(INITIAL_PORT);

#[derive(Debug)]
pub struct NatsServerGuard {
    child: Child,
    port: u16,
    url: String,
}

impl NatsServerGuard {
    pub async fn start(port: Option<u16>) -> Result<Self, TestWebServicesError> {
        let port = port.unwrap_or_else(|| next_port());
        
        for _ in 0..MAX_PORT_ATTEMPTS {
            let mut candidate = Self::spawn(port)?;
            match candidate.wait_ready().await {
                Ok(()) => return Ok(candidate),
                Err(TestWebServicesError::ExitedEarly { .. }) => {
                    continue;
                }
                Err(err) => return Err(err),
            }
        }

        Err(TestWebServicesError::ReadyTimeout {
            service: "nats-server".to_string(),
            timeout: READY_TIMEOUT,
            last_error: format!("exhausted {MAX_PORT_ATTEMPTS} startup attempts"),
            log_path: PathBuf::from("<none>"),
        })
    }

    fn spawn(port: u16) -> Result<Self, TestWebServicesError> {
        let child = Command::new(NATS_SERVER_BIN)
            .args(["-js", "-a", "127.0.0.1", "-p", &port.to_string()])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|source| TestWebServicesError::Spawn {
                service: "nats-server".to_string(),
                source,
            })?;

        let url = format!("nats://127.0.0.1:{}", port);

        Ok(Self {
            child,
            port,
            url,
        })
    }

    async fn wait_ready(&mut self) -> Result<(), TestWebServicesError> {
        let started_at = tokio::time::Instant::now();
        let mut last_error = String::from("connection not attempted");

        loop {
            if let Ok(Some(status)) = self.child.try_wait() {
                return Err(TestWebServicesError::ExitedEarly {
                    service: "nats-server".to_string(),
                    status,
                    log_path: PathBuf::from("<stdout>"),
                });
            }

            if started_at.elapsed() >= READY_TIMEOUT {
                return Err(TestWebServicesError::ReadyTimeout {
                    service: "nats-server".to_string(),
                    timeout: READY_TIMEOUT,
                    last_error,
                    log_path: PathBuf::from("<stdout>"),
                });
            }

            match async_nats::connect(self.url.as_str()).await {
                Ok(client) => {
                    let _ = async_nats::jetstream::new(client);
                    return Ok(());
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

impl Drop for NatsServerGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn next_port() -> u16 {
    NEXT_PORT.fetch_add(1, Ordering::Relaxed)
}