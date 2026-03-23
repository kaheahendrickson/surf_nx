use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde_json::Value;
use solana_account::Account;
use solana_hash::Hash;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use solana_transaction::Transaction;
use std::time::Duration;
use surf_client::backend::{
    AccountInfo, InstructionInfo, ParsedTransaction, ProgramAccountsFilter, SignatureInfo,
    SignaturesForAddressOptions, TransactionMessage,
};
use surf_client::error::Error;
use surf_client::rpc_types::*;
use surf_client_http_config::HttpBackendConfig;

#[cfg(target_arch = "wasm32")]
use surf_client::backend::WasmBackend;
#[cfg(not(target_arch = "wasm32"))]
use surf_client::Backend;

pub struct HttpBackend {
    client: Client,
    url: String,
}

impl HttpBackend {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(url: &str) -> Self {
        Self {
            client: Client::new(),
            url: url.to_string(),
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn new(url: &str) -> Self {
        Self {
            client: Client::new(),
            url: url.to_string(),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_config(config: HttpBackendConfig) -> Self {
        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .expect("Failed to create HTTP client");
        Self {
            client,
            url: config.url,
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn from_config(config: HttpBackendConfig) -> Self {
        Self {
            client: Client::new(),
            url: config.url,
        }
    }

    async fn rpc_call<R: DeserializeOwned + Default>(
        &self,
        method: &'static str,
        params: Vec<Value>,
    ) -> Result<R, Error> {
        let request = JsonRpcRequest::new(1, method, params);
        let response = self
            .client
            .post(&self.url)
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::Backend(format!("HTTP request failed: {}", e)))?;

        let rpc_response: JsonRpcResponse<R> = response
            .json()
            .await
            .map_err(|e| Error::Backend(format!("Failed to parse response: {}", e)))?;

        if let Some(err) = rpc_response.error {
            return Err(Error::Backend(format!(
                "RPC error {}: {}",
                err.code, err.message
            )));
        }

        rpc_response
            .result
            .ok_or_else(|| Error::Backend("No result in response".to_string()))
    }

    async fn rpc_call_allow_null<R: DeserializeOwned>(
        &self,
        method: &'static str,
        params: Vec<Value>,
    ) -> Result<Option<R>, Error> {
        let request = JsonRpcRequest::new(1, method, params);
        let response = self
            .client
            .post(&self.url)
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::Backend(format!("HTTP request failed: {}", e)))?;

        let rpc_response: Value = response
            .json()
            .await
            .map_err(|e| Error::Backend(format!("Failed to parse response: {}", e)))?;

        if let Some(err) = rpc_response.get("error") {
            let code = err.get("code").and_then(Value::as_i64).unwrap_or(-32603);
            let message = err
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("Unknown RPC error");
            return Err(Error::Backend(format!("RPC error {}: {}", code, message)));
        }

        match rpc_response.get("result") {
            Some(Value::Null) | None => Ok(None),
            Some(result) => serde_json::from_value(result.clone())
                .map(Some)
                .map_err(|e| Error::Backend(format!("Failed to parse response: {}", e))),
        }
    }

    async fn transaction_exists(&self, signature: &Signature) -> Result<bool, Error> {
        let request = JsonRpcRequest::new(
            1,
            METHOD_GET_TRANSACTION,
            vec![
                Value::String(signature.to_string()),
                serde_json::json!({"encoding": "json", "commitment": "confirmed"}),
            ],
        );
        let response = self
            .client
            .post(&self.url)
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::Backend(format!("HTTP request failed: {}", e)))?;

        let rpc_response: Value = response
            .json()
            .await
            .map_err(|e| Error::Backend(format!("Failed to parse response: {}", e)))?;

        if let Some(err) = rpc_response.get("error") {
            let code = err.get("code").and_then(Value::as_i64).unwrap_or(-32603);
            let message = err
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("Unknown RPC error");
            return Err(Error::Backend(format!("RPC error {}: {}", code, message)));
        }

        Ok(rpc_response
            .get("result")
            .is_some_and(|result| !result.is_null()))
    }

    async fn get_account_raw(&self, pubkey: &Pubkey) -> Result<Option<Account>, Error> {
        let result: RpcContextResult<AccountInfoResult> = self
            .rpc_call(
                METHOD_GET_ACCOUNT_INFO,
                vec![
                    Value::String(pubkey.to_string()),
                    serde_json::json!({"encoding": "base64", "commitment": "confirmed"}),
                ],
            )
            .await?;

        match result.value {
            Some(acc) => {
                let data = acc
                    .data
                    .first()
                    .map(|s| {
                        BASE64
                            .decode(s)
                            .map_err(|e| Error::Backend(format!("Invalid base64 data: {}", e)))
                    })
                    .transpose()?
                    .unwrap_or_default();

                let owner = acc
                    .owner
                    .parse()
                    .map_err(|_| Error::Backend("Invalid owner pubkey".to_string()))?;

                Ok(Some(Account {
                    lamports: acc.lamports,
                    data,
                    owner,
                    executable: acc.executable,
                    rent_epoch: acc.rent_epoch,
                }))
            }
            None => Ok(None),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn sleep(duration: Duration) {
        tokio::time::sleep(duration).await;
    }

    #[cfg(target_arch = "wasm32")]
    async fn sleep(duration: Duration) {
        let millis = duration.as_millis() as u32;
        gloo::timers::future::TimeoutFuture::new(millis).await;
    }

    async fn send_and_confirm_impl(&self, tx: &Transaction) -> Result<Signature, Error> {
        let tx_bytes = bincode::serialize(&tx)
            .map_err(|e| Error::Backend(format!("Failed to serialize transaction: {}", e)))?;
        let tx_base64 = BASE64.encode(&tx_bytes);

        let sig_str: String = self
            .rpc_call(
                METHOD_SEND_TRANSACTION,
                vec![
                    Value::String(tx_base64),
                    serde_json::json!({
                        "encoding": "base64",
                        "preflightCommitment": "confirmed"
                    }),
                ],
            )
            .await?;

        let signature: Signature = sig_str
            .parse()
            .map_err(|_| Error::Backend("Invalid signature".to_string()))?;

        for _ in 0..30 {
            if self.transaction_exists(&signature).await? {
                return Ok(signature);
            }

            Self::sleep(Duration::from_millis(200)).await;
        }

        Err(Error::Backend(
            "Transaction was submitted but not confirmed in time".to_string(),
        ))
    }

    async fn get_balance_impl(&self, pubkey: &Pubkey) -> Result<Option<u64>, Error> {
        let result: RpcContextResult<u64> = self
            .rpc_call(
                METHOD_GET_BALANCE,
                vec![
                    Value::String(pubkey.to_string()),
                    serde_json::json!({"commitment": "confirmed"}),
                ],
            )
            .await?;

        Ok(result.value)
    }

    async fn get_latest_blockhash_impl(&self) -> Result<Hash, Error> {
        let result: RpcContextResult<BlockhashResult> = self
            .rpc_call(
                METHOD_GET_LATEST_BLOCKHASH,
                vec![serde_json::json!({"commitment": "confirmed"})],
            )
            .await?;

        result
            .value
            .ok_or_else(|| Error::Backend("No blockhash in response".to_string()))?
            .blockhash
            .parse()
            .map_err(|_| Error::Backend("Invalid blockhash".to_string()))
    }

    async fn minimum_balance_for_rent_exemption_impl(&self, size: usize) -> Result<u64, Error> {
        let result: u64 = self
            .rpc_call(
                METHOD_GET_MINIMUM_BALANCE_FOR_RENT_EXEMPTION,
                vec![
                    Value::Number(size.into()),
                    serde_json::json!({"commitment": "confirmed"}),
                ],
            )
            .await?;

        Ok(result)
    }

    async fn get_program_accounts_impl(
        &self,
        program_id: &Pubkey,
        filters: Option<ProgramAccountsFilter>,
    ) -> Result<Vec<AccountInfo>, Error> {
        let mut config = serde_json::json!({"encoding": "base64", "commitment": "confirmed"});

        if let Some(f) = &filters {
            if let Some(size) = f.data_size {
                config["filters"] = serde_json::json!([{"dataSize": size}]);
            }
        }

        let results: Vec<ProgramAccountResult> = self
            .rpc_call(
                METHOD_GET_PROGRAM_ACCOUNTS,
                vec![Value::String(program_id.to_string()), config],
            )
            .await?;

        results
            .into_iter()
            .map(|r| {
                let pubkey = r
                    .pubkey
                    .parse()
                    .map_err(|_| Error::Backend("Invalid pubkey in program account".to_string()))?;

                let data = r
                    .account
                    .data
                    .first()
                    .map(|s| {
                        BASE64
                            .decode(s)
                            .map_err(|e| Error::Backend(format!("Invalid base64 data: {}", e)))
                    })
                    .transpose()?
                    .unwrap_or_default();

                let owner = r.account.owner.parse().map_err(|_| {
                    Error::Backend("Invalid owner pubkey in program account".to_string())
                })?;

                Ok(AccountInfo {
                    pubkey,
                    account: Account {
                        lamports: r.account.lamports,
                        data,
                        owner,
                        executable: r.account.executable,
                        rent_epoch: r.account.rent_epoch,
                    },
                })
            })
            .collect()
    }

    async fn get_signatures_for_address_impl(
        &self,
        address: &Pubkey,
        options: Option<SignaturesForAddressOptions>,
    ) -> Result<Vec<SignatureInfo>, Error> {
        let mut config = serde_json::json!({"commitment": "confirmed"});

        if let Some(opts) = options {
            if let Some(before) = opts.before {
                config["before"] = Value::String(before.to_string());
            }
            if let Some(until) = opts.until {
                config["until"] = Value::String(format!("{}", until));
            }
            if let Some(limit) = opts.limit {
                config["limit"] = Value::Number(limit.into());
            }
        }

        let results: Vec<SignatureInfoResult> = self
            .rpc_call(
                METHOD_GET_SIGNATURES_FOR_ADDRESS,
                vec![Value::String(address.to_string()), config],
            )
            .await?;

        results
            .into_iter()
            .map(|r| {
                r.signature
                    .parse()
                    .map(|sig| SignatureInfo {
                        signature: sig,
                        slot: r.slot,
                        block_time: r.block_time,
                    })
                    .map_err(|_| Error::Backend("Invalid signature in signature info".to_string()))
            })
            .collect()
    }

    async fn get_transaction_impl(
        &self,
        signature: &Signature,
    ) -> Result<Option<ParsedTransaction>, Error> {
        let result: Option<TransactionMetaInfo> = self
            .rpc_call_allow_null(
                METHOD_GET_TRANSACTION,
                vec![
                    Value::String(signature.to_string()),
                    serde_json::json!({"encoding": "json", "commitment": "confirmed"}),
                ],
            )
            .await?;

        match result {
            Some(tx) => {
                let signatures: Result<Vec<_>, _> = tx
                    .transaction
                    .signatures
                    .into_iter()
                    .map(|s| {
                        s.parse().map_err(|_| {
                            Error::Backend("Invalid signature in transaction".to_string())
                        })
                    })
                    .collect();
                let signatures = signatures?;

                let account_keys: Result<Vec<_>, _> = tx
                    .transaction
                    .message
                    .account_keys
                    .0
                    .into_iter()
                    .map(|s| {
                        s.parse().map_err(|_| {
                            Error::Backend("Invalid pubkey in transaction".to_string())
                        })
                    })
                    .collect();
                let account_keys = account_keys?;

                let instructions: Result<Vec<_>, _> = tx
                    .transaction
                    .message
                    .instructions
                    .into_iter()
                    .map(|i| {
                        BASE64
                            .decode(&i.data)
                            .map(|data| InstructionInfo {
                                program_id_index: i.program_id_index,
                                accounts: i.accounts,
                                data,
                            })
                            .map_err(|_| {
                                Error::Backend("Failed to decode instruction data".to_string())
                            })
                    })
                    .collect();
                let instructions = instructions?;

                Ok(Some(ParsedTransaction {
                    slot: tx.slot,
                    block_time: tx.block_time,
                    signatures,
                    message: TransactionMessage {
                        account_keys,
                        instructions,
                    },
                }))
            }
            None => Ok(None),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Backend for HttpBackend {
    async fn get_account(&self, pubkey: &Pubkey) -> Result<Option<Account>, Error> {
        self.get_account_raw(pubkey).await
    }

    async fn get_balance(&self, pubkey: &Pubkey) -> Result<Option<u64>, Error> {
        self.get_balance_impl(pubkey).await
    }

    async fn get_latest_blockhash(&self) -> Result<Hash, Error> {
        self.get_latest_blockhash_impl().await
    }

    async fn minimum_balance_for_rent_exemption(&self, size: usize) -> Result<u64, Error> {
        self.minimum_balance_for_rent_exemption_impl(size).await
    }

    async fn send_and_confirm(&self, tx: &Transaction) -> Result<Signature, Error> {
        self.send_and_confirm_impl(tx).await
    }

    async fn get_program_accounts(
        &self,
        program_id: &Pubkey,
        filters: Option<ProgramAccountsFilter>,
    ) -> Result<Vec<AccountInfo>, Error> {
        self.get_program_accounts_impl(program_id, filters).await
    }

    async fn get_signatures_for_address(
        &self,
        address: &Pubkey,
        options: Option<SignaturesForAddressOptions>,
    ) -> Result<Vec<SignatureInfo>, Error> {
        self.get_signatures_for_address_impl(address, options).await
    }

    async fn get_transaction(
        &self,
        signature: &Signature,
    ) -> Result<Option<ParsedTransaction>, Error> {
        self.get_transaction_impl(signature).await
    }
}

#[cfg(target_arch = "wasm32")]
impl WasmBackend for HttpBackend {
    async fn get_account(&self, pubkey: &Pubkey) -> Result<Option<Account>, Error> {
        self.get_account_raw(pubkey).await
    }

    async fn get_balance(&self, pubkey: &Pubkey) -> Result<Option<u64>, Error> {
        self.get_balance_impl(pubkey).await
    }

    async fn get_latest_blockhash(&self) -> Result<Hash, Error> {
        self.get_latest_blockhash_impl().await
    }

    async fn minimum_balance_for_rent_exemption(&self, size: usize) -> Result<u64, Error> {
        self.minimum_balance_for_rent_exemption_impl(size).await
    }

    async fn send_and_confirm(&self, tx: &Transaction) -> Result<Signature, Error> {
        self.send_and_confirm_impl(tx).await
    }

    async fn get_program_accounts(
        &self,
        program_id: &Pubkey,
        filters: Option<ProgramAccountsFilter>,
    ) -> Result<Vec<AccountInfo>, Error> {
        self.get_program_accounts_impl(program_id, filters).await
    }

    async fn get_signatures_for_address(
        &self,
        address: &Pubkey,
        options: Option<SignaturesForAddressOptions>,
    ) -> Result<Vec<SignatureInfo>, Error> {
        self.get_signatures_for_address_impl(address, options).await
    }

    async fn get_transaction(
        &self,
        signature: &Signature,
    ) -> Result<Option<ParsedTransaction>, Error> {
        self.get_transaction_impl(signature).await
    }
}

pub use surf_client::rpc_types;

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;

    #[fixture]
    fn test_pubkey() -> Pubkey {
        std::env::var("SOLANA_KEYPAIR_PUBKEY")
            .expect("SOLANA_KEYPAIR_PUBKEY env var must be set")
            .parse()
            .expect("Invalid pubkey")
    }

    #[fixture]
    fn http_backend() -> HttpBackend {
        HttpBackend::new("http://localhost:8899")
    }

    async fn assert_get_balance_works(backend: &HttpBackend, pubkey: &Pubkey) {
        let balance = backend.get_balance(pubkey).await;
        assert!(balance.is_ok(), "get_balance failed: {:?}", balance.err());
        let balance = balance.unwrap();
        assert!(balance.is_some(), "No balance returned");
    }

    #[test]
    fn test_http_backend_new() {
        let backend = HttpBackend::new("http://localhost:8899");
        assert_eq!(backend.url, "http://localhost:8899");
    }

    #[rstest]
    fn test_http_backend_from_config() {
        let config = HttpBackendConfig::new("http://test.validator:9000")
            .with_timeout(std::time::Duration::from_secs(45));
        let backend = HttpBackend::from_config(config);
        assert_eq!(backend.url, "http://test.validator:9000");
    }

    #[rstest]
    #[tokio::test]
    #[ignore = "requires running validator at localhost:8899"]
    async fn test_get_balance_native(test_pubkey: Pubkey, http_backend: HttpBackend) {
        assert_get_balance_works(&http_backend, &test_pubkey).await;
    }
}

#[cfg(all(target_arch = "wasm32", test))]
async fn assert_get_balance_works(backend: &HttpBackend, pubkey: &Pubkey) {
    let balance = backend.get_balance(pubkey).await;
    assert!(balance.is_ok(), "get_balance failed: {:?}", balance.err());
    let balance = balance.unwrap();
    assert!(balance.is_some(), "No balance returned");
}

#[cfg(target_arch = "wasm32")]
#[cfg(test)]
mod wasm_tests {
    use super::*;

    const TEST_PUBKEY: &str = env!("SOLANA_KEYPAIR_PUBKEY");

    #[wasm_bindgen_test::wasm_bindgen_test]
    async fn test_get_balance_wasm() {
        let pubkey: Pubkey = TEST_PUBKEY.parse().expect("Invalid pubkey");
        let backend = HttpBackend::new("http://localhost:8899");
        assert_get_balance_works(&backend, &pubkey).await;
    }
}

#[cfg(all(target_arch = "wasm32", feature = "browser-tests"))]
#[cfg(test)]
mod browser_tests {
    use super::*;
    // use surf_client::backend::WasmBackend;

    const TEST_PUBKEY: &str = env!("SOLANA_KEYPAIR_PUBKEY");

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test::wasm_bindgen_test]
    async fn test_get_balance_browser() {
        let pubkey: Pubkey = TEST_PUBKEY.parse().expect("Invalid pubkey");
        let backend = HttpBackend::new("http://localhost:8899");
        assert_get_balance_works(&backend, &pubkey).await;
    }
}
