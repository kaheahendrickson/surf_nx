#![cfg(target_arch = "wasm32")]

use gloo::net::http::Request;
use serde_json::json;
use solana_keypair::Keypair;
use solana_message::Message;
use solana_native_token::LAMPORTS_PER_SOL;
use solana_pubkey::Pubkey;
use solana_signature::{Signature, SIGNATURE_BYTES};
use solana_signer::Signer;
use solana_system_interface::instruction::transfer;
use solana_transaction::Transaction;
use surf_client::backend::{ProgramAccountsFilter, SignaturesForAddressOptions, WasmBackend};
use wasm_bindgen::prelude::*;

use crate::backend::BrowserBackend;
use crate::HttpBackendConfig;

fn backend(url: &str) -> BrowserBackend {
    BrowserBackend::from_config(HttpBackendConfig::new(url))
}

fn js_error(message: impl Into<String>) -> JsValue {
    JsValue::from_str(&message.into())
}

fn ensure(condition: bool, message: impl Into<String>) -> Result<(), JsValue> {
    if condition {
        Ok(())
    } else {
        Err(js_error(message))
    }
}

async fn airdrop(url: &str, pubkey: &Pubkey, lamports: u64) -> Result<(), JsValue> {
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "requestAirdrop",
        "params": [pubkey.to_string(), lamports]
    });

    let response = Request::post(url)
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .map_err(|err| js_error(format!("Failed to build airdrop request: {err}")))?
        .send()
        .await
        .map_err(|err| js_error(format!("Airdrop request failed: {err}")))?;

    ensure(
        response.ok(),
        format!("Airdrop failed with status {}", response.status()),
    )
}

#[wasm_bindgen]
pub async fn run_surf_client_backend_browser_integration_tests(
    validator_url: String,
) -> Result<(), JsValue> {
    let backend = backend(&validator_url);

    let blockhash = backend
        .get_latest_blockhash()
        .await
        .map_err(|err| js_error(format!("get_latest_blockhash failed: {err}")))?;
    ensure(
        blockhash != solana_hash::Hash::default(),
        "get_latest_blockhash returned default hash",
    )?;

    let missing_account = Pubkey::new_unique();
    let account = backend
        .get_account(&missing_account)
        .await
        .map_err(|err| js_error(format!("get_account failed: {err}")))?;
    ensure(account.is_none(), "expected missing account to return None")?;

    let balance = backend
        .get_balance(&missing_account)
        .await
        .map_err(|err| js_error(format!("get_balance failed: {err}")))?;
    ensure(balance.is_none(), "expected missing balance to return None")?;

    let funded = Keypair::new();
    airdrop(&validator_url, &funded.pubkey(), LAMPORTS_PER_SOL).await?;
    let funded_balance = backend
        .get_balance(&funded.pubkey())
        .await
        .map_err(|err| js_error(format!("get_balance funded failed: {err}")))?;
    ensure(
        funded_balance.is_some_and(|amount| amount >= LAMPORTS_PER_SOL),
        "expected funded account balance after airdrop",
    )?;

    let minimum = backend
        .minimum_balance_for_rent_exemption(100)
        .await
        .map_err(|err| js_error(format!("minimum_balance_for_rent_exemption failed: {err}")))?;
    ensure(minimum > 0, "expected rent exemption to be positive")?;

    let system_program = solana_system_interface::program::id();
    backend
        .get_program_accounts(&system_program, None)
        .await
        .map_err(|err| js_error(format!("get_program_accounts failed: {err}")))?;

    let filtered_accounts = backend
        .get_program_accounts(
            &system_program,
            Some(ProgramAccountsFilter { data_size: Some(0) }),
        )
        .await
        .map_err(|err| js_error(format!("get_program_accounts with filter failed: {err}")))?;
    ensure(
        filtered_accounts
            .iter()
            .all(|account| account.account.data.is_empty()),
        "expected filtered program accounts to have zero-length data",
    )?;

    let payer = Keypair::new();
    let recipient = Pubkey::new_unique();
    airdrop(&validator_url, &payer.pubkey(), 2 * LAMPORTS_PER_SOL).await?;

    let tx_blockhash = backend
        .get_latest_blockhash()
        .await
        .map_err(|err| js_error(format!("get_latest_blockhash for tx failed: {err}")))?;
    let instruction = transfer(&payer.pubkey(), &recipient, LAMPORTS_PER_SOL);
    let message = Message::new_with_blockhash(&[instruction], None, &tx_blockhash);
    let mut tx = Transaction::new_unsigned(message);
    tx.sign(&[&payer], tx_blockhash);

    let signature = backend
        .send_and_confirm(&tx)
        .await
        .map_err(|err| js_error(format!("send_and_confirm failed: {err}")))?;
    ensure(
        signature != Signature::default(),
        "expected non-default transaction signature",
    )?;

    let recipient_balance = backend
        .get_balance(&recipient)
        .await
        .map_err(|err| js_error(format!("recipient balance lookup failed: {err}")))?;
    ensure(
        recipient_balance == Some(LAMPORTS_PER_SOL),
        format!(
            "expected recipient balance {}, got {:?}",
            LAMPORTS_PER_SOL, recipient_balance
        ),
    )?;

    let signatures = backend
        .get_signatures_for_address(
            &payer.pubkey(),
            Some(SignaturesForAddressOptions {
                limit: Some(1),
                ..Default::default()
            }),
        )
        .await
        .map_err(|err| js_error(format!("get_signatures_for_address failed: {err}")))?;
    ensure(
        signatures.len() <= 1,
        format!("expected at most one signature, got {}", signatures.len()),
    )?;

    let missing_signature = Signature::from([0u8; SIGNATURE_BYTES]);
    let missing_tx = backend
        .get_transaction(&missing_signature)
        .await
        .map_err(|err| js_error(format!("get_transaction missing failed: {err}")))?;
    ensure(
        missing_tx.is_none(),
        "expected missing transaction to return None",
    )?;

    Ok(())
}
