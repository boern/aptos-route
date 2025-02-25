#![allow(unused)]
use crate::aptos_client::constants::{FORWARD_KEY, HEADER_SIZE_LIMIT, IDEMPOTENCY_KEY};
use crate::aptos_client::error::RestError;
use crate::aptos_client::request::{self, build_rest_req};
use crate::aptos_client::utils::get_http_request_cost;
use crate::config::{mutate_config, read_config, AptosPortAction};
use crate::constants::{
    BURN_FUNC, COIN_MODULE, COIN_PKG_ID, DEFAULT_GAS_BUDGET, MINT_FUNC, MINT_WITH_TICKET_FUNC,
    SUI_COIN, UPDATE_DESC_FUNC, UPDATE_ICON_FUNC, UPDATE_NAME_FUNC, UPDATE_SYMBOL_FUNC,
};
use crate::ic_log::{DEBUG, ERROR};

use crate::state::{mutate_state, read_state, AptosToken, UpdateType};

use aptos_api_types::transaction::Transaction;
use aptos_types::transaction::SignedTransaction;
use candid::CandidType;
use futures::{stream, StreamExt};
use futures_core::Stream;
use ic_canister_log::log;
use ic_cdk::api;
use ic_cdk::api::management_canister::http_request::{
    http_request, CanisterHttpRequestArgument, HttpHeader, HttpMethod, HttpResponse,
    TransformContext,
};
use ic_cdk::api::print;

use move_core_types::account_address::AccountAddress;

use serde::Deserialize;
use serde::Serialize;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::future;
use std::str::FromStr;
use std::sync::Arc;

use crate::ck_eddsa::{hash_with_sha256, KeyType};
use serde_bytes::ByteBuf;

use super::aptos_providers::Provider;
use super::request::RestReq;
use super::response::Response;
use super::types::{parse_error, parse_state, AptosResult};
use super::{Account, State};
pub use aptos_api_types::PendingTransaction;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct JsonRpcResponse<T> {
    pub jsonrpc: String,
    pub result: Option<T>,
    pub error: Option<JsonRpcError>,
    pub id: u64,
}

#[derive(Debug, thiserror::Error, Deserialize, CandidType)]
pub enum RpcError {
    #[error("http outcall error: {0}")]
    HttpCallError(String),
    #[error("RPC response error {code}: {message} {data:?}")]
    RpcResponseError {
        code: i64,
        message: String,
        data: Option<String>,
    },
    #[error("parse error: expected {0}")]
    ParseError(String),
    #[error("{0}")]
    Text(String),
}

impl From<JsonRpcError> for RpcError {
    fn from(e: JsonRpcError) -> Self {
        Self::RpcResponseError {
            code: e.code,
            message: e.message,
            data: None,
        }
    }
}

impl From<serde_json::Error> for RpcError {
    fn from(e: serde_json::Error) -> Self {
        let error_string = e.to_string();
        Self::ParseError(error_string)
    }
}

pub type RpcResult<T> = Result<T, RpcError>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RestClient {
    pub provider: Provider,
    pub nodes_in_subnet: Option<u32>,
}

impl RestClient {
    pub fn new(provider: Provider, nodes_in_subnet: Option<u32>) -> Self {
        Self {
            provider,
            nodes_in_subnet,
        }
    }

    pub fn with_nodes_in_subnet(mut self, nodes_in_subnet: u32) -> Self {
        self.nodes_in_subnet = Some(nodes_in_subnet);
        self
    }

    /// Asynchronously sends an HTTP POST request to the specified URL with the given payload and
    /// maximum response bytes, and returns the response as a string.
    /// This function calculates the required cycles for the HTTP request and logs the request
    /// details and response status. It uses a transformation named "cleanup_response" for the
    /// response body.
    ///
    /// # Arguments
    ///
    /// * `payload` - A string slice that holds the JSON payload to be sent in the HTTP request.
    /// * `max_response_bytes` - A u64 value representing the maximum number of bytes for the response.
    ///
    /// # Returns
    ///
    /// * `RpcResult<String>` - A result type that contains the response body as a string if the request
    /// is successful, or an `RpcError` if the request fails.
    ///
    /// # Errors
    ///
    /// This function returns an `RpcError` in the following cases:
    /// * If the response body cannot be parsed as a UTF-8 string, a `ParseError` is returned.
    /// * If the HTTP request fails, an `RpcRequestError` is returned with the error details.
    ///
    pub async fn call(
        &self,
        mut req: RestReq,
        max_response_bytes: u64,
        transform: Option<TransformContext>,
        forward: Option<String>,
    ) -> AptosResult<HttpResponse> {
        // let transform = transform.unwrap_or(TransformContext::from_name(
        //     "cleanup_response".to_owned(),
        //     vec![],
        // ));

        // add forward address
        if let Some(forward) = forward.to_owned() {
            let mut headers = req.headers;
            headers.push(HttpHeader {
                name: FORWARD_KEY.to_string(),
                value: forward,
            });
            log!(DEBUG, "[rpc_client::call] headers: {:?} ", headers);
            // update req headers
            req.headers = headers;
        }

        let request = CanisterHttpRequestArgument {
            url: req.url.to_string(),
            max_response_bytes: Some(max_response_bytes + HEADER_SIZE_LIMIT),
            // max_response_bytes: None,
            method: req.method,
            headers: req.headers,
            body: req.body,
            // transform: Some(transform),
            transform: None,
        };

        let url = req.url.to_string();
        log!(DEBUG, "Calling url: {url} ");
        let cycles = get_http_request_cost(
            request.body.as_ref().map_or(0, |b| b.len() as u64),
            request.max_response_bytes.unwrap_or(2 * 1024 * 1024), // default 2Mb
        );

        let start = api::time();
        match http_request(request, cycles).await {
            Ok((response,)) => {
                let end = api::time();
                let elapsed = (end - start) / 1_000_000_000;

                log!(
                    DEBUG,
                    "Got response (with {} bytes): {} from url: {} with status: {} the time elapsed: {}",
                    response.body.len(),
                    String::from_utf8_lossy(&response.body),
                    url,
                    response.status,
                    elapsed
                );

                // match String::from_utf8(response.body) {
                //     Ok(body) => Ok(body),
                //     Err(error) => Err(RpcError::ParseError(error.to_string())),
                // }
                Ok(response)
            }
            Err((r, m)) => {
                let end = api::time();
                let elapsed = (end - start) / 1_000_000_000;
                log!(
                    ERROR,
                    "Got response  error : {:?},{} from url: {} ,the time elapsed: {}",
                    r,
                    m,
                    url,
                    elapsed
                );
                Err(RestError::HttpCallError(format!("({r:?}) {m:?}")))
            }
        }
    }

    pub async fn get_account(
        &self,
        address: String,
        ledger_version: Option<u64>,
        forward: Option<String>,
    ) -> AptosResult<Account> {
        let mut req = build_rest_req(request::AtosRequest::GetAccount { address });
        log!(DEBUG, "[rpc_client::get_account] request: {:?} ", req);

        let response = self.call(req, 1000, None, forward).await?;
        match self.json::<Account>(response) {
            Ok(response) => Ok(response.into_inner()),
            Err(e) => {
                log!(DEBUG, "[rpc_client::get_account] response error: {:?}", e);
                Err(e.into())
            }
        }
    }

    pub async fn get_account_balance(
        &self,
        address: String,
        asset_type: Option<String>,
        forward: Option<String>,
    ) -> AptosResult<u64> {
        let mut req = build_rest_req(request::AtosRequest::GetAccountBalance {
            address,
            asset_type: asset_type.unwrap_or("0x1::aptos_coin::AptosCoin".to_string()),
        });
        log!(
            DEBUG,
            "[rpc_client::get_account_balance] request: {:?} ",
            req
        );

        let response = self.call(req, 1000, None, forward).await?;
        log!(
            DEBUG,
            "[rpc_client::get_account_balance] response: {:?} ",
            response
        );
        // self.json(response)
        let balance = String::from_utf8(response.body)
            .map_err(|e| RestError::ParseError(e.to_string()))?
            .parse::<u64>()
            .map_err(|e| RestError::ParseError(e.to_string()))?;

        Ok(balance)
    }

    pub async fn transfer_aptos(
        &self,
        txn: &SignedTransaction,
        forward: Option<String>,
    ) -> AptosResult<PendingTransaction> {
        let mut req = build_rest_req(request::AtosRequest::SubmitTransaction {
            txn: txn.to_owned(),
        });
        log!(DEBUG, "[rpc_client::transfer_aptos] request: {:?} ", req);

        let response = self.call(req, 5000, None, forward).await?;
        log!(
            DEBUG,
            "[rpc_client::transfer_aptos] response: {:?} ",
            response
        );
        match self.json::<PendingTransaction>(response) {
            Ok(response) => Ok(response.into_inner()),
            Err(e) => {
                log!(
                    DEBUG,
                    "[rpc_client::transfer_aptos] response error: {:?}",
                    e
                );
                Err(e.into())
            }
        }
    }

    pub async fn get_transaction_by_hash(
        &self,
        txn_hash: String,

        forward: Option<String>,
    ) -> AptosResult<Transaction> {
        let mut req = build_rest_req(request::AtosRequest::GetTransactionByHash { txn_hash });
        log!(
            DEBUG,
            "[rpc_client::get_transaction_by_hash] request: {:?} ",
            req
        );

        let response = self.call(req, 5000, None, forward).await?;
        match self.json::<Transaction>(response) {
            Ok(response) => Ok(response.into_inner()),
            Err(e) => {
                log!(
                    DEBUG,
                    "[rpc_client::get_transaction_by_hash] response error: {:?}",
                    e
                );
                Err(e.into())
            }
        }
    }

    fn check_response(&self, response: HttpResponse) -> AptosResult<HttpResponse> {
        // Check if status is within 200-299.
        //TODO map err
        let status_code: u16 = response.status.to_owned().0.try_into().map_err(|_| {
            RestError::ParseError(format!("Invalid status code: {:?}", response.status))
        })?;
        if !(300 > status_code && status_code >= 200) {
            Err(parse_error(response))
        } else {
            // let state = parse_state(&response)?;
            Ok(response)
        }
    }

    fn json<T: serde::de::DeserializeOwned>(
        &self,
        response: HttpResponse,
    ) -> AptosResult<Response<T>> {
        let response = self.check_response(response)?;
        // let json = response.json().await.map_err(anyhow::Error::from)?;
        let json = serde_json::from_slice(&response.body)?;
        Ok(Response::new(json))
    }
}

#[cfg(test)]
mod test {
    // use aptos_api_types::PendingTransaction;

    use aptos_api_types::move_types::{EntryFunctionId, MoveType};
    use aptos_api_types::transaction::{Event, TransactionInfo, TransactionSignature};
    use aptos_api_types::{Address, HashValue, U64};
    use candid::Deserialize;
    use serde::Serialize;

    /// A transaction waiting in mempool
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct PendingTransaction {
        pub hash: HashValue,
        #[serde(flatten)]
        pub request: UserTransactionRequest,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct UserTransactionRequest {
        pub sender: Address,
        pub sequence_number: U64,
        pub max_gas_amount: U64,
        pub gas_unit_price: U64,
        pub expiration_timestamp_secs: U64,
        pub payload: TransactionPayload,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub signature: Option<TransactionSignature>,
    }

    /// An enum of the possible transaction payloads
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]

    pub enum TransactionPayload {
        EntryFunctionPayload(EntryFunctionPayload),
        // ScriptPayload(ScriptPayload),
    }
    /// Payload which runs a single entry function
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct EntryFunctionPayload {
        pub function: EntryFunctionId,
        /// Type arguments of the function
        pub type_arguments: Vec<MoveType>,
        /// Arguments of the function
        pub arguments: Vec<serde_json::Value>,
    }

    /// Enum of the different types of transactions in Aptos
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    // //#[oai(one_of, discriminator_name = "type", rename_all = "snake_case")]
    pub enum Transaction {
        PendingTransaction(PendingTransaction),
        UserTransaction(UserTransaction),
    }

    /// A transaction submitted by a user to change the state of the blockchain
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct UserTransaction {
        #[serde(flatten)]
        pub info: TransactionInfo,
        #[serde(flatten)]
        pub request: UserTransactionRequest,
        /// Events generated by the transaction
        pub events: Vec<Event>,
        pub timestamp: U64,
    }

    #[test]
    fn parse_devnet_pending_tx() {
        let json_str = r#" 
            {
            "hash": "0x169238641c3f97f2bc0b4a46707faf12457de857015f0882c6b2635e17486e4a",
            "sender": "0x140549f1a4aade6333b361764d772256c962810c3f934d451e1d84481732d874",
            "sequence_number": "1",
            "max_gas_amount": "5000",
            "gas_unit_price": "150",
            "expiration_timestamp_secs": "1740473379",
            "payload":
                {
                "function": "0x1::aptos_account::transfer_coins",
                "type_arguments": ["0x1::aptos_coin::AptosCoin"],
                "arguments":
                    [
                    "0x1961df628d2d224ecc91d56dfd0a4b9a545e9cf0ec9da2337c6c5c73f6171db8",
                    "20000000"
                    ],
                "type": "entry_function_payload"
                },
            "signature":
                {
                "public_key": "0x403baa9a6c9c303abbf463a47e39cbe36f5c7d3def5bde5d5725151264fb1de7",
                "signature": "0x4bb257dd0189ea8c0f3effdc152f05a42f65f2e076c56614a7f3c24ae1e69ed6cb6cd417b321b74d4457643f0ef9e369769db1df1705a2ffdd34d4f425b7fe08",
                "type": "ed25519_signature"
                }
            }
        "#;

        let json_response = serde_json::from_str::<PendingTransaction>(json_str);
        println!("json_response: {:#?}", json_response);
    }

    #[test]
    fn parse_devnet_finalized_tx() {
        let json_str = r#" 
            {
                "version": "47261875",
                "hash": "0x169238641c3f97f2bc0b4a46707faf12457de857015f0882c6b2635e17486e4a",
                "state_change_hash": "0x818375654f5e358d08152afc8ac5c09880f01204926abbf1c75c2d530a4bdb82",
                "event_root_hash": "0x155fe8f10d6504d6b89015f856b9e1357957565148efde0b17529974d492176a",
                "state_checkpoint_hash": null,
                "gas_used": "11",
                "success": true,
                "vm_status": "Executed successfully",
                "accumulator_root_hash": "0x18209b2057a3afc17e4cc324678b256d67f318fff5beb182adf50e1784033011",
                "changes": [
                    {
                    "address": "0x140549f1a4aade6333b361764d772256c962810c3f934d451e1d84481732d874",
                    "state_key_hash": "0x30ddb15d4d66160ea8283d3a9b2831fd592f216dd5c8b5ff5b8e21ec37c95607",
                    "data": {
                        "type": "0x1::coin::CoinStore<0x1::aptos_coin::AptosCoin>",
                        "data": {
                        "coin": {
                            "value": "69996700"
                        },
                        "deposit_events": {
                            "counter": "0",
                            "guid": {
                            "id": {
                                "addr": "0x140549f1a4aade6333b361764d772256c962810c3f934d451e1d84481732d874",
                                "creation_num": "2"
                            }
                            }
                        },
                        "frozen": false,
                        "withdraw_events": {
                            "counter": "0",
                            "guid": {
                            "id": {
                                "addr": "0x140549f1a4aade6333b361764d772256c962810c3f934d451e1d84481732d874",
                                "creation_num": "3"
                            }
                            }
                        }
                        }
                    },
                    "type": "write_resource"
                    },
                    {
                    "address": "0x140549f1a4aade6333b361764d772256c962810c3f934d451e1d84481732d874",
                    "state_key_hash": "0xb7c8165bebf33e974a6821b4d4a8faf647d7e0b694eec2a2bf313d967c9bbe37",
                    "data": {
                        "type": "0x1::account::Account",
                        "data": {
                        "authentication_key": "0x140549f1a4aade6333b361764d772256c962810c3f934d451e1d84481732d874",
                        "coin_register_events": {
                            "counter": "0",
                            "guid": {
                            "id": {
                                "addr": "0x140549f1a4aade6333b361764d772256c962810c3f934d451e1d84481732d874",
                                "creation_num": "0"
                            }
                            }
                        },
                        "guid_creation_num": "4",
                        "key_rotation_events": {
                            "counter": "0",
                            "guid": {
                            "id": {
                                "addr": "0x140549f1a4aade6333b361764d772256c962810c3f934d451e1d84481732d874",
                                "creation_num": "1"
                            }
                            }
                        },
                        "rotation_capability_offer": {
                            "for": {
                            "vec": []
                            }
                        },
                        "sequence_number": "2",
                        "signer_capability_offer": {
                            "for": {
                            "vec": []
                            }
                        }
                        }
                    },
                    "type": "write_resource"
                    },
                    {
                    "address": "0x1961df628d2d224ecc91d56dfd0a4b9a545e9cf0ec9da2337c6c5c73f6171db8",
                    "state_key_hash": "0xd01cd0adbba7a65aa819101079614191505c5e1f2717cec5807f378b981b0e6f",
                    "data": {
                        "type": "0x1::coin::CoinStore<0x1::aptos_coin::AptosCoin>",
                        "data": {
                        "coin": {
                            "value": "180000000"
                        },
                        "deposit_events": {
                            "counter": "0",
                            "guid": {
                            "id": {
                                "addr": "0x1961df628d2d224ecc91d56dfd0a4b9a545e9cf0ec9da2337c6c5c73f6171db8",
                                "creation_num": "2"
                            }
                            }
                        },
                        "frozen": false,
                        "withdraw_events": {
                            "counter": "0",
                            "guid": {
                            "id": {
                                "addr": "0x1961df628d2d224ecc91d56dfd0a4b9a545e9cf0ec9da2337c6c5c73f6171db8",
                                "creation_num": "3"
                            }
                            }
                        }
                        }
                    },
                    "type": "write_resource"
                    },
                    {
                    "state_key_hash": "0x6e4b28d40f98a106a65163530924c0dcb40c1349d3aa915d108b4d6cfc1ddb19",
                    "handle": "0x1b854694ae746cdbd8d44186ca4929b2b337df21d1c74633be19b2710552fdca",
                    "key": "0x0619dc29a0aac8fa146714058e8dd6d2d0f3bdf5f6331907bf91f3acd81e6935",
                    "value": "0x0bb8852922cb01000100000000000000",
                    "data": null,
                    "type": "write_table_item"
                    }
                ],
                "sender": "0x140549f1a4aade6333b361764d772256c962810c3f934d451e1d84481732d874",
                "sequence_number": "1",
                "max_gas_amount": "5000",
                "gas_unit_price": "150",
                "expiration_timestamp_secs": "1740473379",
                "payload": {
                    "function": "0x1::aptos_account::transfer_coins",
                    "type_arguments": [
                    "0x1::aptos_coin::AptosCoin"
                    ],
                    "arguments": [
                    "0x1961df628d2d224ecc91d56dfd0a4b9a545e9cf0ec9da2337c6c5c73f6171db8",
                    "20000000"
                    ],
                    "type": "entry_function_payload"
                },
                "signature": {
                    "public_key": "0x403baa9a6c9c303abbf463a47e39cbe36f5c7d3def5bde5d5725151264fb1de7",
                    "signature": "0x4bb257dd0189ea8c0f3effdc152f05a42f65f2e076c56614a7f3c24ae1e69ed6cb6cd417b321b74d4457643f0ef9e369769db1df1705a2ffdd34d4f425b7fe08",
                    "type": "ed25519_signature"
                },
                "events": [
                    {
                    "guid": {
                        "creation_number": "0",
                        "account_address": "0x0"
                    },
                    "sequence_number": "0",
                    "type": "0x1::coin::CoinWithdraw",
                    "data": {
                        "account": "0x140549f1a4aade6333b361764d772256c962810c3f934d451e1d84481732d874",
                        "amount": "20000000",
                        "coin_type": "0x1::aptos_coin::AptosCoin"
                    }
                    },
                    {
                    "guid": {
                        "creation_number": "0",
                        "account_address": "0x0"
                    },
                    "sequence_number": "0",
                    "type": "0x1::coin::CoinDeposit",
                    "data": {
                        "account": "0x1961df628d2d224ecc91d56dfd0a4b9a545e9cf0ec9da2337c6c5c73f6171db8",
                        "amount": "20000000",
                        "coin_type": "0x1::aptos_coin::AptosCoin"
                    }
                    },
                    {
                    "guid": {
                        "creation_number": "0",
                        "account_address": "0x0"
                    },
                    "sequence_number": "0",
                    "type": "0x1::transaction_fee::FeeStatement",
                    "data": {
                        "execution_gas_units": "5",
                        "io_gas_units": "6",
                        "storage_fee_octas": "0",
                        "storage_fee_refund_octas": "0",
                        "total_charge_gas_units": "11"
                    }
                    }
                ],
                "timestamp": "1740472885223509",
                "type": "user_transaction"
            }
        "#;

        let json_response = serde_json::from_str::<Transaction>(json_str);
        println!("json_response: {:#?}", json_response);
    }
}
