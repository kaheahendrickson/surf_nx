use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

pub const METHOD_GET_ACCOUNT_INFO: &str = "getAccountInfo";
pub const METHOD_GET_BALANCE: &str = "getBalance";
pub const METHOD_GET_LATEST_BLOCKHASH: &str = "getLatestBlockhash";
pub const METHOD_GET_MINIMUM_BALANCE_FOR_RENT_EXEMPTION: &str = "getMinimumBalanceForRentExemption";
pub const METHOD_SEND_TRANSACTION: &str = "sendTransaction";
pub const METHOD_GET_PROGRAM_ACCOUNTS: &str = "getProgramAccounts";
pub const METHOD_GET_SIGNATURES_FOR_ADDRESS: &str = "getSignaturesForAddress";
pub const METHOD_GET_TRANSACTION: &str = "getTransaction";

#[derive(Serialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: &'static str,
    pub id: u64,
    pub method: &'static str,
    pub params: Vec<Value>,
}

impl JsonRpcRequest {
    pub fn new(id: u64, method: &'static str, params: Vec<Value>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            method,
            params,
        }
    }
}

#[derive(Deserialize)]
pub struct JsonRpcResponse<T> {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(default)]
    pub result: Option<T>,
    #[serde(default)]
    pub error: Option<JsonRpcError>,
}

#[derive(Deserialize, Debug)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcContextResult<T> {
    pub value: Option<T>,
}

impl<T> Default for RpcContextResult<T> {
    fn default() -> Self {
        Self { value: None }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfoResult {
    pub lamports: u64,
    pub data: Vec<String>,
    pub owner: String,
    pub executable: bool,
    pub rent_epoch: u64,
    #[serde(default)]
    pub space: u64,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BlockhashResult {
    pub blockhash: String,
    pub last_valid_block_height: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureInfoResult {
    pub signature: String,
    pub slot: u64,
    pub block_time: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionMetaInfo {
    pub slot: u64,
    pub block_time: Option<i64>,
    pub transaction: ParsedTransactionResult,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedTransactionResult {
    pub signatures: Vec<String>,
    pub message: ParsedMessageResult,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedMessageResult {
    pub account_keys: PubkeyStrings,
    pub instructions: Vec<ParsedInstructionResult>,
}

#[derive(Deserialize)]
pub struct PubkeyStrings(pub Vec<String>);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedInstructionResult {
    pub program_id_index: u8,
    pub accounts: Vec<u8>,
    pub data: String,
}

#[derive(Deserialize)]
pub struct ProgramAccountResult {
    pub pubkey: String,
    pub account: AccountInfoResult,
}
