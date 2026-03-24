use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use crate::config::{registry_program_id, signals_program_id, token_program_id, tracked_address};
use crate::error::TestWebServicesError;

const EVENTS_SERVER_BIN: &str = "surf-events-server";

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn find_events_binary() -> PathBuf {
    let workspace = workspace_root();
    
    let debug_path = workspace.join("target/debug").join(EVENTS_SERVER_BIN);
    if debug_path.exists() {
        return debug_path;
    }
    
    let release_path = workspace.join("target/release").join(EVENTS_SERVER_BIN);
    if release_path.exists() {
        return release_path;
    }
    
    PathBuf::from(EVENTS_SERVER_BIN)
}

#[derive(Debug)]
pub struct EventsServerGuard {
    child: Child,
}

impl EventsServerGuard {
    pub async fn start(
        nats_url: &str,
        rpc_url: &str,
        checkpoint_path: &Path,
    ) -> Result<Self, TestWebServicesError> {
        let token_id = token_program_id();
        let registry_id = registry_program_id();
        let signals_id = signals_program_id();
        let tracked = tracked_address();
        
        let events_bin = find_events_binary();

        let child = Command::new(events_bin)
            .env("SURF_TEST_VALIDATOR_URL", rpc_url)
            .env("SURF_NATS_URL", nats_url)
            .env("SURF_EVENTS_STREAM", "surf-events")
            .env("SURF_EVENTS_TRACKED_ADDRESS", tracked.to_string())
            .env("SURF_TOKEN_PROGRAM", token_id.to_string())
            .env("SURF_REGISTRY_PROGRAM", registry_id.to_string())
            .env("SURF_SIGNALS_PROGRAM", signals_id.to_string())
            .env("SURF_EVENTS_CHECKPOINT_PATH", checkpoint_path.to_string_lossy().to_string())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|source| TestWebServicesError::Spawn {
                service: "surf-events-server".to_string(),
                source,
            })?;

        let guard = Self { child };

        guard.wait_ready().await
    }

    async fn wait_ready(mut self) -> Result<Self, TestWebServicesError> {
        tokio::time::sleep(Duration::from_millis(500)).await;

        if let Ok(Some(status)) = self.child.try_wait() {
            return Err(TestWebServicesError::ExitedEarly {
                service: "surf-events-server".to_string(),
                status,
                log_path: PathBuf::from("<stdout>"),
            });
        }

        Ok(self)
    }
}

impl Drop for EventsServerGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}