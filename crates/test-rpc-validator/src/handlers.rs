use std::sync::Arc;

use axum::{extract::State, Json};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde_json::Value;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use solana_transaction::Transaction;
use surf_client::backend::Backend;
use surf_client::backend::TestBackend;

use crate::error::RpcError;
use crate::rpc_types::*;
use crate::server::AppState;

pub async fn handle_rpc(
    State(state): State<Arc<AppState>>,
    Json(request): Json<Value>,
) -> Json<Value> {
    let rpc_request: JsonRpcRequest = match serde_json::from_value(request) {
        Ok(req) => req,
        Err(e) => {
            return Json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": null,
                "error": JsonRpcError::invalid_request(format!("Invalid JSON-RPC request: {}", e))
            }));
        }
    };

    let id = rpc_request.id;

    let result = match rpc_request.method.as_str() {
        "getAccountInfo" => handle_get_account_info(&state, &rpc_request.params).await,
        "getBalance" => handle_get_balance(&state, &rpc_request.params).await,
        "getLatestBlockhash" => handle_get_latest_blockhash(&state).await,
        "getMinimumBalanceForRentExemption" => {
            handle_get_minimum_balance_for_rent_exemption(&state, &rpc_request.params).await
        }
        "sendTransaction" => handle_send_transaction(&state, &rpc_request.params).await,
        "getProgramAccounts" => handle_get_program_accounts(&state, &rpc_request.params).await,
        "getSignaturesForAddress" => {
            handle_get_signatures_for_address(&state, &rpc_request.params).await
        }
        "getTransaction" => handle_get_transaction(&state, &rpc_request.params).await,
        "requestAirdrop" => handle_request_airdrop(&state, &rpc_request.params).await,
        "getHealth" => Ok(serde_json::json!("ok")),
        "getSignatureStatuses" => handle_get_signature_statuses(&state, &rpc_request.params).await,
        "isBlockhashValid" => Ok(serde_json::json!({ "context": { "slot": 1 }, "value": true })),
        _ => {
            eprintln!("Unknown method: {}", rpc_request.method);
            Err(RpcError::Backend(format!(
                "Method not found: {}",
                rpc_request.method
            )))
        }
    };

    let response = match result {
        Ok(value) => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(value),
            error: None,
        },
        Err(err) => {
            let json_error = match &err {
                RpcError::Backend(msg) if msg.starts_with("Method not found") => {
                    JsonRpcError::method_not_found()
                }
                RpcError::MissingParam(_) => JsonRpcError::invalid_params(err.to_string()),
                _ => JsonRpcError::internal_error(err.to_string()),
            };
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(json_error),
            }
        }
    };

    Json(serde_json::to_value(response).unwrap())
}

async fn handle_get_account_info(state: &AppState, params: &[Value]) -> Result<Value, RpcError> {
    let pubkey_str = params
        .get(0)
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::MissingParam("pubkey".to_string()))?;

    let pubkey: Pubkey = pubkey_str
        .parse()
        .map_err(|_| RpcError::InvalidPubkey(pubkey_str.to_string()))?;

    let account = state
        .backend
        .get_account(&pubkey)
        .await
        .map_err(RpcError::from)?;

    let result = account.map(|acc| {
        let data = BASE64.encode(&acc.data);
        AccountInfoResult {
            lamports: acc.lamports,
            data: vec![data, "base64".to_string()],
            owner: acc.owner.to_string(),
            executable: acc.executable,
            rent_epoch: acc.rent_epoch,
        }
    });

    Ok(serde_json::to_value(RpcContextResult::new(result)).unwrap())
}

async fn handle_get_balance(state: &AppState, params: &[Value]) -> Result<Value, RpcError> {
    let pubkey_str = params
        .get(0)
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::MissingParam("pubkey".to_string()))?;

    let pubkey: Pubkey = pubkey_str
        .parse()
        .map_err(|_| RpcError::InvalidPubkey(pubkey_str.to_string()))?;

    let balance = state
        .backend
        .get_balance(&pubkey)
        .await
        .map_err(RpcError::from)?
        .unwrap_or(0);

    Ok(serde_json::to_value(RpcContextResult::new(balance)).unwrap())
}

async fn handle_get_latest_blockhash(state: &AppState) -> Result<Value, RpcError> {
    let hash = state
        .backend
        .get_latest_blockhash()
        .await
        .map_err(RpcError::from)?;

    Ok(serde_json::json!({
        "context": {
            "slot": 1,
            "apiVersion": "1.0.0"
        },
        "value": {
            "blockhash": hash.to_string(),
            "lastValidBlockHeight": 1000000u64
        }
    }))
}

async fn handle_get_minimum_balance_for_rent_exemption(
    state: &AppState,
    params: &[Value],
) -> Result<Value, RpcError> {
    let size = params
        .get(0)
        .and_then(|v| v.as_u64())
        .ok_or_else(|| RpcError::MissingParam("dataLength".to_string()))?;

    let rent = state
        .backend
        .minimum_balance_for_rent_exemption(size as usize)
        .await
        .map_err(RpcError::from)?;

    Ok(serde_json::json!(rent))
}

async fn handle_send_transaction(state: &AppState, params: &[Value]) -> Result<Value, RpcError> {
    let tx_base64 = params
        .get(0)
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::MissingParam("transaction".to_string()))?;

    let tx_bytes = BASE64
        .decode(tx_base64)
        .map_err(|e| RpcError::InvalidTransaction(format!("Invalid base64: {}", e)))?;

    let tx: Transaction = bincode::deserialize(&tx_bytes)
        .map_err(|e| RpcError::InvalidTransaction(format!("Invalid transaction: {}", e)))?;

    let signature = state
        .backend
        .send_and_confirm(&tx)
        .await
        .map_err(RpcError::from)?;

    Ok(serde_json::json!(signature.to_string()))
}

async fn handle_get_program_accounts(
    state: &AppState,
    params: &[Value],
) -> Result<Value, RpcError> {
    let pubkey_str = params
        .get(0)
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::MissingParam("programId".to_string()))?;

    let program_id: Pubkey = pubkey_str
        .parse()
        .map_err(|_| RpcError::InvalidPubkey(pubkey_str.to_string()))?;

    let data_size = params
        .get(1)
        .and_then(|config| config.get("filters"))
        .and_then(|filters| filters.as_array())
        .and_then(|filters| filters.iter().find_map(|filter| filter.get("dataSize")))
        .and_then(|size| size.as_u64())
        .map(|size| size as usize);

    let accounts = state
        .backend
        .get_program_accounts(
            &program_id,
            Some(surf_client::backend::ProgramAccountsFilter { data_size }),
        )
        .await
        .map_err(RpcError::from)?;

    let result: Vec<ProgramAccountResult> = accounts
        .into_iter()
        .map(|account| ProgramAccountResult {
            pubkey: account.pubkey.to_string(),
            account: AccountInfoResult {
                lamports: account.account.lamports,
                data: vec![BASE64.encode(&account.account.data), "base64".to_string()],
                owner: account.account.owner.to_string(),
                executable: account.account.executable,
                rent_epoch: account.account.rent_epoch,
            },
        })
        .collect();

    Ok(serde_json::to_value(result).unwrap())
}

async fn handle_get_signatures_for_address(
    state: &AppState,
    params: &[Value],
) -> Result<Value, RpcError> {
    let pubkey_str = params
        .get(0)
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::MissingParam("address".to_string()))?;

    let address: Pubkey = pubkey_str
        .parse()
        .map_err(|_| RpcError::InvalidPubkey(pubkey_str.to_string()))?;

    let options = params
        .get(1)
        .map(|config| surf_client::backend::SignaturesForAddressOptions {
            before: config
                .get("before")
                .and_then(|value| value.as_str())
                .and_then(|value| value.parse::<Signature>().ok()),
            until: config.get("until").and_then(|value| value.as_u64()),
            limit: config
                .get("limit")
                .and_then(|value| value.as_u64())
                .map(|value| value as usize),
        });

    let signatures = state
        .backend
        .get_signatures_for_address(&address, options)
        .await
        .map_err(RpcError::from)?;

    let result: Vec<SignatureInfoResult> = signatures
        .into_iter()
        .map(|info| SignatureInfoResult {
            signature: info.signature.to_string(),
            slot: info.slot,
            block_time: info.block_time,
        })
        .collect();

    Ok(serde_json::to_value(result).unwrap())
}

async fn handle_get_transaction(state: &AppState, params: &[Value]) -> Result<Value, RpcError> {
    let sig_str = params
        .get(0)
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::MissingParam("signature".to_string()))?;

    let signature: Signature = sig_str
        .parse()
        .map_err(|_| RpcError::InvalidTransaction(format!("Invalid signature: {}", sig_str)))?;

    let tx = state
        .backend
        .get_transaction(&signature)
        .await
        .map_err(RpcError::from)?;

    let result = tx.map(|tx| TransactionMetaInfoResult {
        slot: tx.slot,
        block_time: tx.block_time,
        transaction: ParsedTransactionResult {
            signatures: tx
                .signatures
                .into_iter()
                .map(|sig| sig.to_string())
                .collect(),
            message: ParsedMessageResult {
                account_keys: tx
                    .message
                    .account_keys
                    .into_iter()
                    .map(|key| key.to_string())
                    .collect(),
                instructions: tx
                    .message
                    .instructions
                    .into_iter()
                    .map(|ix| ParsedInstructionResult {
                        program_id_index: ix.program_id_index,
                        accounts: ix.accounts,
                        data: BASE64.encode(ix.data),
                    })
                    .collect(),
            },
        },
    });

    Ok(serde_json::to_value(result).unwrap())
}

async fn handle_request_airdrop(state: &AppState, params: &[Value]) -> Result<Value, RpcError> {
    let pubkey_str = params
        .get(0)
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::MissingParam("pubkey".to_string()))?;

    let lamports = params
        .get(1)
        .and_then(|v| v.as_u64())
        .ok_or_else(|| RpcError::MissingParam("lamports".to_string()))?;

    let pubkey: Pubkey = pubkey_str
        .parse()
        .map_err(|_| RpcError::InvalidPubkey(pubkey_str.to_string()))?;

    let signature = state
        .backend
        .airdrop(&pubkey, lamports)
        .await
        .map_err(RpcError::from)?;

    Ok(serde_json::json!(signature.to_string()))
}

async fn handle_get_signature_statuses(
    state: &AppState,
    params: &[Value],
) -> Result<Value, RpcError> {
    let sig_strs = params
        .get(0)
        .and_then(|v| v.as_array())
        .ok_or_else(|| RpcError::MissingParam("signatures".to_string()))?;

    let mut statuses: Vec<Option<Value>> = Vec::new();
    for sig_str in sig_strs {
        let status = if let Some(sig_str) = sig_str.as_str() {
            if let Ok(signature) = sig_str.parse::<Signature>() {
                if let Some(tx) = state.backend.get_transaction(&signature).await.ok().flatten() {
                    Some(serde_json::json!({
                        "slot": tx.slot,
                        "confirmations": None::<u64>,
                        "err": null,
                        "status": { "Ok": null },
                        "confirmationStatus": "finalized"
                    }))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };
        statuses.push(status);
    }

    Ok(serde_json::json!({ "context": { "slot": 1 }, "value": statuses }))
}
