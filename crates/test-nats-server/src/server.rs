use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;

use tempfile::TempDir;

use crate::error::NatsServerTestError;

const HOST: &str = "127.0.0.1";
const NATS_SERVER_BIN: &str = "/usr/local/bin/nats-server";
const INITIAL_PORT: u16 = 41_000;
const MAX_PORT_ATTEMPTS: u16 = 100;
const READY_TIMEOUT: Duration = Duration::from_secs(15);
const READY_POLL_INTERVAL: Duration = Duration::from_millis(250);

static NEXT_PORT: AtomicU16 = AtomicU16::new(INITIAL_PORT);

#[derive(Debug)]
pub struct NatsServerGuard {
    child: Child,
    _temp_dir: TempDir,
    log_path: PathBuf,
    port: u16,
    url: String,
}

impl NatsServerGuard {
    pub async fn start() -> Result<Self, NatsServerTestError> {
        for _ in 0..MAX_PORT_ATTEMPTS {
            let port = next_port();
            let mut candidate = Self::spawn(port)?;
            match candidate.wait_ready().await {
                Ok(()) => return Ok(candidate),
                Err(NatsServerTestError::ExitedEarly { .. }) => continue,
                Err(err) => return Err(err),
            }
        }

        Err(NatsServerTestError::ReadyTimeout {
            timeout: READY_TIMEOUT,
            last_error: format!("exhausted {MAX_PORT_ATTEMPTS} startup attempts"),
            log_path: PathBuf::from("<none>"),
        })
    }

    fn spawn(port: u16) -> Result<Self, NatsServerTestError> {
        let temp_dir = tempfile::tempdir()?;
        let log_path = temp_dir.path().join("nats-server.log");
        let log_file = File::create(&log_path)?;
        let stderr_file = log_file.try_clone()?;
        let child = Command::new(NATS_SERVER_BIN)
            .args(["-js", "-a", HOST, "-p", &port.to_string()])
            .stdout(Stdio::from(log_file))
            .stderr(Stdio::from(stderr_file))
            .spawn()
            .map_err(|source| NatsServerTestError::Spawn { source })?;

        Ok(Self {
            child,
            _temp_dir: temp_dir,
            log_path,
            port,
            url: format!("nats://{HOST}:{port}"),
        })
    }

    async fn wait_ready(&mut self) -> Result<(), NatsServerTestError> {
        let started_at = tokio::time::Instant::now();
        let mut last_error = String::from("connection not attempted");

        loop {
            if let Ok(Some(status)) = self.child.try_wait() {
                return Err(NatsServerTestError::ExitedEarly {
                    status,
                    log_path: self.log_path.clone(),
                });
            }

            if started_at.elapsed() >= READY_TIMEOUT {
                return Err(NatsServerTestError::ReadyTimeout {
                    timeout: READY_TIMEOUT,
                    last_error,
                    log_path: self.log_path.clone(),
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

    pub fn log_path(&self) -> &Path {
        &self.log_path
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
