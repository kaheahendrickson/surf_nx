use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use crate::error::TestWebServicesError;

pub const SBF_TOKEN_PROGRAM: &str = "sbf_surf_token";
pub const SBF_REGISTRY_PROGRAM: &str = "sbf_surf_name_registry";
pub const SBF_SIGNALS_PROGRAM: &str = "sbf_surf_signals";

static SBF_BUILD: OnceLock<()> = OnceLock::new();

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn deploy_artifact(name: &str) -> PathBuf {
    workspace_root()
        .join("target/deploy")
        .join(format!("{}.so", name))
}

pub(crate) fn ensure_sbf_programs_built() -> Result<(), TestWebServicesError> {
    SBF_BUILD.get_or_init(|| {
        let manifests = [
            "crates/sbf-surf-token/Cargo.toml",
            "crates/sbf-surf-name-registry/Cargo.toml",
            "crates/sbf-surf-signals/Cargo.toml",
        ];

        for manifest in manifests {
            let status = Command::new("cargo")
                .arg("build-sbf")
                .arg("--manifest-path")
                .arg(manifest)
                .current_dir(workspace_root())
                .status()
                .unwrap_or_else(|err| {
                    panic!("failed to run cargo build-sbf for {manifest}: {err}")
                });

            assert!(status.success(), "cargo build-sbf failed for {manifest}");
        }
    });
    Ok(())
}

pub fn load_program_bytes(name: &str) -> Result<Vec<u8>, TestWebServicesError> {
    let path = deploy_artifact(name);
    if !path.exists() {
        ensure_sbf_programs_built()?;
    }
    std::fs::read(&path).map_err(|e| TestWebServicesError::ProgramRead { path, source: e })
}

pub fn load_token_program() -> Result<Vec<u8>, TestWebServicesError> {
    load_program_bytes(SBF_TOKEN_PROGRAM)
}

pub fn load_registry_program() -> Result<Vec<u8>, TestWebServicesError> {
    load_program_bytes(SBF_REGISTRY_PROGRAM)
}

pub fn load_signals_program() -> Result<Vec<u8>, TestWebServicesError> {
    load_program_bytes(SBF_SIGNALS_PROGRAM)
}

pub fn programs_built() -> bool {
    deploy_artifact(SBF_TOKEN_PROGRAM).exists()
        && deploy_artifact(SBF_REGISTRY_PROGRAM).exists()
        && deploy_artifact(SBF_SIGNALS_PROGRAM).exists()
}

pub fn all_program_paths() -> (PathBuf, PathBuf, PathBuf) {
    (
        deploy_artifact(SBF_TOKEN_PROGRAM),
        deploy_artifact(SBF_REGISTRY_PROGRAM),
        deploy_artifact(SBF_SIGNALS_PROGRAM),
    )
}
