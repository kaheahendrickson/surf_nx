use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use rstest::{fixture, rstest};
use solana_keypair::Keypair;
use solana_native_token::LAMPORTS_PER_SOL;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use surf_client::backend::{Backend, TestBackend};
use surf_client::client::{AuthorityClient, Surf};
use surf_client::query::QueryClient;
use surf_client::signer::LocalKeypairSigner;

use super::MolluskBackend;

static SBF_BUILD: OnceLock<()> = OnceLock::new();

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn deploy_artifact(name: &str) -> PathBuf {
    workspace_root().join("target/deploy").join(name)
}

fn ensure_sbf_programs_built() {
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
                .unwrap_or_else(|err| panic!("failed to run cargo build-sbf for {manifest}: {err}"));

            assert!(status.success(), "cargo build-sbf failed for {manifest}");
        }
    });
}

fn load_token_program() -> Vec<u8> {
    let path = deploy_artifact("sbf_surf_token.so");
    if !path.exists() {
        ensure_sbf_programs_built();
    }

    std::fs::read(path)
        .expect("failed to read sbf_surf_token.so")
}

fn load_registry_program() -> Vec<u8> {
    let path = deploy_artifact("sbf_surf_name_registry.so");
    if !path.exists() {
        ensure_sbf_programs_built();
    }

    std::fs::read(path)
        .expect("failed to read sbf_surf_name_registry.so")
}

fn try_load_signals_program() -> Option<Vec<u8>> {
    let path = deploy_artifact("sbf_surf_signals.so");
    if !path.exists() {
        ensure_sbf_programs_built();
    }

    std::fs::read(path).ok()
}

#[fixture]
fn backend() -> MolluskBackend {
    MolluskBackend::new()
}

#[fixture]
async fn backend_with_programs() -> (MolluskBackend, Pubkey, Pubkey) {
    let backend = MolluskBackend::new();
    let token_program = Pubkey::new_unique();
    let registry_program = Pubkey::new_unique();

    backend
        .add_program(&token_program, &load_token_program())
        .await
        .unwrap();
    backend
        .add_program(&registry_program, &load_registry_program())
        .await
        .unwrap();

    (backend, token_program, registry_program)
}

#[fixture]
async fn funded_backend() -> (MolluskBackend, Pubkey, Pubkey, Keypair, Keypair) {
    let backend = MolluskBackend::new();
    let token_program = Pubkey::new_unique();
    let registry_program = Pubkey::new_unique();
    let authority = Keypair::new();
    let user = Keypair::new();

    backend
        .add_program(&token_program, &load_token_program())
        .await
        .unwrap();
    backend
        .add_program(&registry_program, &load_registry_program())
        .await
        .unwrap();
    backend
        .airdrop(&authority.pubkey(), 10 * LAMPORTS_PER_SOL)
        .await
        .unwrap();
    backend
        .airdrop(&user.pubkey(), 5 * LAMPORTS_PER_SOL)
        .await
        .unwrap();

    (backend, token_program, registry_program, authority, user)
}

#[fixture]
async fn funded_backend_with_signals() -> (MolluskBackend, Pubkey, Pubkey, Pubkey, Keypair, Keypair)
{
    let signals_bytes = try_load_signals_program().expect("failed to read sbf_surf_signals.so");
    let backend = MolluskBackend::new();
    let token_program = Pubkey::new_unique();
    let registry_program = Pubkey::new_unique();
    let signals_program = Pubkey::new_unique();
    let authority = Keypair::new();
    let user = Keypair::new();

    backend
        .add_program(&token_program, &load_token_program())
        .await
        .unwrap();
    backend
        .add_program(&registry_program, &load_registry_program())
        .await
        .unwrap();
    backend
        .add_program(&signals_program, &signals_bytes)
        .await
        .unwrap();
    backend
        .airdrop(&authority.pubkey(), 10 * LAMPORTS_PER_SOL)
        .await
        .unwrap();
    backend
        .airdrop(&user.pubkey(), 5 * LAMPORTS_PER_SOL)
        .await
        .unwrap();

    (
        backend,
        token_program,
        registry_program,
        signals_program,
        authority,
        user,
    )
}

#[rstest]
#[tokio::test]
async fn test_airdrop_and_balance(backend: MolluskBackend) {
    let pubkey = Pubkey::new_unique();

    backend.airdrop(&pubkey, 1_000_000).await.unwrap();
    let balance = backend.get_balance(&pubkey).await.unwrap();
    assert_eq!(balance, Some(1_000_000));
}

#[rstest]
#[tokio::test]
async fn test_get_latest_blockhash(backend: MolluskBackend) {
    let blockhash = backend.get_latest_blockhash().await.unwrap();
    assert!(!blockhash.to_bytes().iter().all(|&b| b == 0));
}

#[rstest]
#[tokio::test]
async fn test_minimum_balance_for_rent_exemption(backend: MolluskBackend) {
    let minimum = backend
        .minimum_balance_for_rent_exemption(100)
        .await
        .unwrap();
    assert!(minimum > 0);
}

#[rstest]
#[tokio::test]
async fn test_account_persistence(backend: MolluskBackend) {
    let pubkey = Pubkey::new_unique();

    backend.airdrop(&pubkey, 500_000).await.unwrap();
    let account = backend.get_account(&pubkey).await.unwrap();
    assert!(account.is_some());
    assert_eq!(account.unwrap().lamports, 500_000);
}

#[rstest]
#[tokio::test]
async fn test_multiple_airdrops(backend: MolluskBackend) {
    let pubkey1 = Pubkey::new_unique();
    let pubkey2 = Pubkey::new_unique();

    backend.airdrop(&pubkey1, LAMPORTS_PER_SOL).await.unwrap();
    backend
        .airdrop(&pubkey2, 2 * LAMPORTS_PER_SOL)
        .await
        .unwrap();

    assert_eq!(
        backend.get_balance(&pubkey1).await.unwrap(),
        Some(LAMPORTS_PER_SOL)
    );
    assert_eq!(
        backend.get_balance(&pubkey2).await.unwrap(),
        Some(2 * LAMPORTS_PER_SOL)
    );
}

#[rstest]
#[tokio::test]
async fn test_get_nonexistent_account(backend: MolluskBackend) {
    let nonexistent = Pubkey::new_unique();

    assert!(backend.get_account(&nonexistent).await.unwrap().is_none());
    assert!(backend.get_balance(&nonexistent).await.unwrap().is_none());
}

#[rstest]
#[tokio::test]
async fn test_mollusk_backend_connection(backend: MolluskBackend) {
    let blockhash = backend.get_latest_blockhash().await.unwrap();
    assert!(!blockhash.to_bytes().iter().all(|&b| b == 0));
}

#[rstest]
#[tokio::test]
async fn test_mollusk_backend_balance(backend: MolluskBackend) {
    let keypair = Keypair::new();
    let pubkey = keypair.pubkey();

    backend.airdrop(&pubkey, LAMPORTS_PER_SOL).await.unwrap();
    assert_eq!(
        backend.get_balance(&pubkey).await.unwrap(),
        Some(LAMPORTS_PER_SOL)
    );
}

#[rstest]
#[tokio::test]
async fn test_mollusk_backend_account(backend: MolluskBackend) {
    let keypair = Keypair::new();
    let pubkey = keypair.pubkey();

    backend.airdrop(&pubkey, LAMPORTS_PER_SOL).await.unwrap();

    let account = backend.get_account(&pubkey).await.unwrap();
    assert!(account.is_some());
    assert_eq!(account.unwrap().lamports, LAMPORTS_PER_SOL);
}

#[rstest]
#[tokio::test]
async fn test_mollusk_backend_account_not_found(backend: MolluskBackend) {
    let pubkey = Pubkey::new_unique();
    assert!(backend.get_account(&pubkey).await.unwrap().is_none());
}

#[rstest]
#[tokio::test]
async fn test_mollusk_backend_rent_exemption(backend: MolluskBackend) {
    let rent_0 = backend.minimum_balance_for_rent_exemption(0).await.unwrap();
    let rent_100 = backend
        .minimum_balance_for_rent_exemption(100)
        .await
        .unwrap();
    let rent_1000 = backend
        .minimum_balance_for_rent_exemption(1000)
        .await
        .unwrap();

    assert!(rent_0 > 0);
    assert!(rent_100 >= rent_0);
    assert!(rent_1000 >= rent_100);
}

#[rstest]
#[tokio::test]
async fn test_sbf_surf_token_program_loads(backend: MolluskBackend) {
    let token_program_id = Pubkey::new_unique();
    backend
        .add_program(&token_program_id, &load_token_program())
        .await
        .unwrap();
}

#[rstest]
#[tokio::test]
async fn test_sbf_surf_name_registry_program_loads(backend: MolluskBackend) {
    let registry_program_id = Pubkey::new_unique();
    backend
        .add_program(&registry_program_id, &load_registry_program())
        .await
        .unwrap();
}

#[rstest]
#[tokio::test]
async fn test_sbf_surf_signals_program_loads(backend: MolluskBackend) {
    let Some(signals_bytes) = try_load_signals_program() else {
        return;
    };

    let signals_program_id = Pubkey::new_unique();
    backend
        .add_program(&signals_program_id, &signals_bytes)
        .await
        .unwrap();
}

#[rstest]
#[tokio::test]
async fn test_programs_load_correctly(
    #[future] backend_with_programs: (MolluskBackend, Pubkey, Pubkey),
) {
    let (backend, token_program, registry_program) = backend_with_programs.await;

    let token_account = backend.get_account(&token_program).await.unwrap();
    assert!(token_account.is_some());
    assert!(token_account.unwrap().executable);

    let registry_account = backend.get_account(&registry_program).await.unwrap();
    assert!(registry_account.is_some());
    assert!(registry_account.unwrap().executable);
}

#[rstest]
#[tokio::test]
async fn test_token_client_creation(
    #[future] backend_with_programs: (MolluskBackend, Pubkey, Pubkey),
) {
    let (backend, token_program, registry_program) = backend_with_programs.await;

    let authority_keypair = Keypair::new();
    let authority_signer = LocalKeypairSigner::new(authority_keypair);

    backend
        .airdrop(&authority_signer.pubkey(), 10 * LAMPORTS_PER_SOL)
        .await
        .unwrap();

    let surf = Surf::new(backend, token_program, registry_program);
    let authority_client = surf.authority(authority_signer);

    assert_eq!(authority_client.token_program(), &token_program);
    assert_eq!(authority_client.registry_program(), &registry_program);
    assert_eq!(authority_client.token().program_id(), &token_program);
    assert_eq!(authority_client.names().program_id(), &registry_program);
}

#[rstest]
#[tokio::test]
async fn test_token_client_user_client(backend: MolluskBackend) {
    let token_program_id = Pubkey::new_unique();
    let registry_program_id = Pubkey::new_unique();

    let user_keypair = Keypair::new();
    let user_signer = LocalKeypairSigner::new(user_keypair);

    backend
        .airdrop(&user_signer.pubkey(), LAMPORTS_PER_SOL)
        .await
        .unwrap();

    let surf = Surf::new(backend, token_program_id, registry_program_id);
    let user_client = surf.user(user_signer);

    assert_eq!(user_client.token_program(), &token_program_id);
}

#[rstest]
#[tokio::test]
async fn test_names_client_lookup_nonexistent(backend: MolluskBackend) {
    let token_program_id = Pubkey::new_unique();
    let registry_program_id = Pubkey::new_unique();

    let user_keypair = Keypair::new();
    let user_signer = LocalKeypairSigner::new(user_keypair);

    backend
        .airdrop(&user_signer.pubkey(), LAMPORTS_PER_SOL)
        .await
        .unwrap();

    let surf = Surf::new(backend, token_program_id, registry_program_id);
    let user_client = surf.user(user_signer);

    let result = user_client.names().lookup("nonexistent").await.unwrap();
    assert!(result.is_none());
}

#[rstest]
#[tokio::test]
async fn test_query_client_config_nonexistent(backend: MolluskBackend) {
    let token_program_id = Pubkey::new_unique();
    let registry_program_id = Pubkey::new_unique();

    let query = QueryClient::new(backend, token_program_id, registry_program_id);

    assert!(query.token_config().await.is_err());
    assert!(query.registry_config().await.is_err());
    assert!(query.signals_config().await.is_err());
}

#[rstest]
#[tokio::test]
async fn test_signals_initialize_follow_and_unfollow(
    #[future] funded_backend_with_signals: (
        MolluskBackend,
        Pubkey,
        Pubkey,
        Pubkey,
        Keypair,
        Keypair,
    ),
) {
    if try_load_signals_program().is_none() {
        return;
    }

    let (backend, token_program, registry_program, signals_program, authority, user) =
        funded_backend_with_signals.await;
    let user_pubkey = user.pubkey();
    let target = Keypair::new();

    backend
        .airdrop(&target.pubkey(), LAMPORTS_PER_SOL)
        .await
        .unwrap();

    let authority_client = Surf::new(backend.clone(), token_program, registry_program)
        .with_signals_program(signals_program)
        .authority(LocalKeypairSigner::new(authority));
    let user_client = Surf::new(backend.clone(), token_program, registry_program)
        .with_signals_program(signals_program)
        .user(LocalKeypairSigner::new(user));
    let query = QueryClient::new(backend, token_program, registry_program)
        .with_signals_program(signals_program);

    authority_client
        .token()
        .initialize(1_000_000, 9)
        .await
        .unwrap();
    authority_client
        .signals()
        .initialize(1, &token_program)
        .await
        .unwrap();
    authority_client
        .token()
        .mint(&user_pubkey, 10)
        .await
        .unwrap();

    user_client
        .signals()
        .follow(&target.pubkey())
        .await
        .unwrap();
    user_client
        .signals()
        .unfollow(&target.pubkey())
        .await
        .unwrap();

    let config = query.signals_config().await.unwrap();
    assert_eq!(config.token_program, token_program);
    assert_eq!(config.min_balance, 1);
}

#[rstest]
#[tokio::test]
async fn test_signals_reject_self_follow(
    #[future] funded_backend_with_signals: (
        MolluskBackend,
        Pubkey,
        Pubkey,
        Pubkey,
        Keypair,
        Keypair,
    ),
) {
    if try_load_signals_program().is_none() {
        return;
    }

    let (backend, token_program, registry_program, signals_program, authority, user) =
        funded_backend_with_signals.await;
    let user_pubkey = user.pubkey();

    let authority_client = Surf::new(backend.clone(), token_program, registry_program)
        .with_signals_program(signals_program)
        .authority(LocalKeypairSigner::new(authority));
    let user_client = Surf::new(backend, token_program, registry_program)
        .with_signals_program(signals_program)
        .user(LocalKeypairSigner::new(user));

    authority_client
        .token()
        .initialize(1_000_000, 9)
        .await
        .unwrap();
    authority_client
        .signals()
        .initialize(1, &token_program)
        .await
        .unwrap();
    authority_client
        .token()
        .mint(&user_pubkey, 10)
        .await
        .unwrap();

    assert!(user_client.signals().follow(&user_pubkey).await.is_err());
}

#[rstest]
#[tokio::test]
async fn test_multiple_clients_same_backend_state(
    #[future] backend_with_programs: (MolluskBackend, Pubkey, Pubkey),
) {
    let (backend, token_program, registry_program) = backend_with_programs.await;

    let authority_keypair = Keypair::new();
    let authority_pubkey = authority_keypair.pubkey();

    backend
        .airdrop(&authority_pubkey, 10 * LAMPORTS_PER_SOL)
        .await
        .unwrap();

    let authority_signer = LocalKeypairSigner::new(authority_keypair);
    let surf = Surf::new(backend, token_program, registry_program);

    let authority_client = surf.authority(authority_signer);
    assert_eq!(authority_client.signer_pubkey(), authority_pubkey);
}

#[rstest]
#[tokio::test]
async fn test_surf_client_full_example(
    #[future] funded_backend: (MolluskBackend, Pubkey, Pubkey, Keypair, Keypair),
) {
    let (backend, token_program, registry_program, authority, _user) = funded_backend.await;

    let authority_signer = LocalKeypairSigner::new(authority);
    let surf_authority = Surf::new(backend, token_program, registry_program);
    let authority_client: AuthorityClient<_, _> = surf_authority.authority(authority_signer);

    let _token_ops = authority_client.token();
    let _registry_ops = authority_client.registry();
    assert_eq!(authority_client.token_program(), &token_program);
    assert_eq!(authority_client.registry_program(), &registry_program);
}

#[rstest]
#[tokio::test]
async fn test_token_lifecycle_example(
    #[future] backend_with_programs: (MolluskBackend, Pubkey, Pubkey),
) {
    let (backend, token_program, registry_program) = backend_with_programs.await;

    let authority = Keypair::new();
    let user = Keypair::new();
    let recipient = Keypair::new();

    backend
        .airdrop(&authority.pubkey(), 10 * LAMPORTS_PER_SOL)
        .await
        .unwrap();
    backend
        .airdrop(&user.pubkey(), 5 * LAMPORTS_PER_SOL)
        .await
        .unwrap();
    backend
        .airdrop(&recipient.pubkey(), 1 * LAMPORTS_PER_SOL)
        .await
        .unwrap();

    let authority_signer = LocalKeypairSigner::new(authority);
    let surf = Surf::new(backend, token_program, registry_program);
    let _authority_client: AuthorityClient<_, _> = surf.authority(authority_signer);
}

#[rstest]
#[tokio::test]
async fn test_local_keypair_signer() {
    let keypair = Keypair::new();
    let signer = LocalKeypairSigner::new(keypair);

    let pubkey = signer.pubkey();
    assert_ne!(pubkey, Pubkey::default());

    let message = b"test message";
    let signature = signer.sign(message).unwrap();
    assert!(!signature.as_ref().iter().all(|&b| b == 0));
}

#[test]
fn test_name_validation_too_short() {
    let name = "ab";
    assert!(name.len() < 3);
}

#[test]
fn test_name_validation_too_long() {
    let name = "a".repeat(33);
    assert!(name.len() > 32);
}

#[test]
fn test_name_validation_valid() {
    let name = "alice";
    assert!(name.len() >= 3 && name.len() <= 32);
    assert!(name.chars().all(|c| c.is_ascii_lowercase()));
}

#[test]
fn test_name_validation_uppercase_normalized() {
    let name = "ALICE";
    let normalized: String = name.chars().map(|c| c.to_ascii_lowercase()).collect();
    assert_eq!(normalized, "alice");
}

#[test]
fn test_invalid_name_characters_rejected() {
    let name = "alice123";
    let has_invalid = name.chars().any(|c| !c.is_ascii_lowercase());
    assert!(
        has_invalid,
        "Name with digits should have invalid characters"
    );
}

#[test]
fn test_token_initialize_packing() {
    let data = surf_protocol::pack_token_initialize(1_000_000, 9);
    assert_eq!(data[0], 0);
    assert_eq!(
        u64::from_le_bytes(data[1..9].try_into().unwrap()),
        1_000_000
    );
    assert_eq!(data[9], 9);
}

#[test]
fn test_token_transfer_packing() {
    let data = surf_protocol::pack_transfer(500_000);
    assert_eq!(data[0], 1);
    assert_eq!(u64::from_le_bytes(data[1..9].try_into().unwrap()), 500_000);
}

#[test]
fn test_token_burn_packing() {
    let data = surf_protocol::pack_burn(100_000);
    assert_eq!(data[0], 2);
    assert_eq!(u64::from_le_bytes(data[1..9].try_into().unwrap()), 100_000);
}

#[test]
fn test_token_mint_packing() {
    let data = surf_protocol::pack_mint(250_000);
    assert_eq!(data[0], 3);
    assert_eq!(u64::from_le_bytes(data[1..9].try_into().unwrap()), 250_000);
}

#[test]
fn test_registry_initialize_packing() {
    let token_program = Pubkey::new_unique();
    let data = surf_protocol::pack_registry_initialize(100_000, &token_program);
    assert_eq!(data[0], 0);
    assert_eq!(u64::from_le_bytes(data[1..9].try_into().unwrap()), 100_000);
    assert_eq!(&data[9..41], token_program.as_ref());
}

#[test]
fn test_register_packing() {
    let name = "alice";
    let mut name_array = [0u8; 32];
    name_array[..name.len()].copy_from_slice(name.as_bytes());
    let data = surf_protocol::pack_register(&name_array, name.len() as u8);
    assert_eq!(data[0], 1);
    assert_eq!(&data[1..6], b"alice");
    assert_eq!(data[33], 5);
}

#[test]
fn test_signals_initialize_packing() {
    let token_program = Pubkey::new_unique();
    let data = surf_protocol::pack_signals_initialize(&token_program, 1);
    assert_eq!(data[0], 0);
    assert_eq!(&data[1..33], token_program.as_ref());
    assert_eq!(u64::from_le_bytes(data[33..41].try_into().unwrap()), 1);
}

#[test]
fn test_signal_packing() {
    let target = Pubkey::new_unique();
    let data = surf_protocol::pack_signal(surf_protocol::SignalKind::Follow, &target);
    assert_eq!(data[0], 1);
    assert_eq!(data[1], surf_protocol::SignalKind::Follow as u8);
    assert_eq!(&data[2..34], target.as_ref());
}

#[test]
fn test_pda_derivation() {
    let program_id = Pubkey::new_unique();
    let (pda1, bump1) = surf_protocol::derive_token_config_pda(&program_id);
    let (pda2, bump2) = surf_protocol::derive_token_config_pda(&program_id);

    assert_eq!(pda1, pda2);
    assert_eq!(bump1, bump2);
    assert_ne!(pda1, program_id);
}

#[test]
fn test_token_balance_pda_derivation() {
    let program_id = Pubkey::new_unique();
    let owner = Pubkey::new_unique();

    let (pda1, bump1) = surf_protocol::derive_token_balance_pda(&owner, &program_id);
    let (pda2, bump2) = surf_protocol::derive_token_balance_pda(&owner, &program_id);

    assert_eq!(pda1, pda2);
    assert_eq!(bump1, bump2);
    assert_ne!(pda1, owner);
}

#[test]
fn test_registry_config_pda_derivation() {
    let program_id = Pubkey::new_unique();

    let (pda1, bump1) = surf_protocol::derive_registry_config_pda(&program_id);
    let (pda2, bump2) = surf_protocol::derive_registry_config_pda(&program_id);

    assert_eq!(pda1, pda2);
    assert_eq!(bump1, bump2);
    assert_ne!(pda1, program_id);
}

#[test]
fn test_name_record_pda_derivation() {
    let program_id = Pubkey::new_unique();
    let name = b"alice";

    let (pda1, bump1) = surf_protocol::derive_name_record_pda(name, &program_id);
    let (pda2, bump2) = surf_protocol::derive_name_record_pda(name, &program_id);

    assert_eq!(pda1, pda2);
    assert_eq!(bump1, bump2);
    assert_ne!(pda1, program_id);
}

#[test]
fn test_signals_config_pda_derivation() {
    let program_id = Pubkey::new_unique();

    let (pda1, bump1) = surf_protocol::derive_signals_config_pda(&program_id);
    let (pda2, bump2) = surf_protocol::derive_signals_config_pda(&program_id);

    assert_eq!(pda1, pda2);
    assert_eq!(bump1, bump2);
    assert_ne!(pda1, program_id);
}
