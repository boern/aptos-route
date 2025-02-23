#![allow(unused)]
use crate::aptos_client::constants::{FORWARD_KEY, HEADER_SIZE_LIMIT, IDEMPOTENCY_KEY};
use crate::aptos_client::utils::get_http_request_cost;
use crate::config::{mutate_config, read_config, AptosPortAction};
use crate::constants::{
    BURN_FUNC, COIN_MODULE, COIN_PKG_ID, DEFAULT_GAS_BUDGET, MINT_FUNC, MINT_WITH_TICKET_FUNC,
    SUI_COIN, UPDATE_DESC_FUNC, UPDATE_ICON_FUNC, UPDATE_NAME_FUNC, UPDATE_SYMBOL_FUNC,
};
use crate::ic_log::{DEBUG, ERROR};

use crate::state::{mutate_state, read_state, AptosToken, UpdateType};

use aptos_types::transaction::SignedTransaction;
use candid::CandidType;
use futures::{stream, StreamExt};
use futures_core::Stream;
use ic_canister_log::log;
use ic_cdk::api;
use ic_cdk::api::management_canister::http_request::{
    http_request, CanisterHttpRequestArgument, HttpHeader, HttpMethod, TransformContext,
};
use move_core_types::account_address::AccountAddress;

use serde::Deserialize;
use serde::Serialize;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::future;
use std::str::FromStr;
use std::sync::Arc;

use crate::ck_eddsa::{hash_with_sha256, hash_with_sha256_byte, KeyType};
use serde_bytes::ByteBuf;

use super::aptos_providers::Provider;

thread_local! {
    static NEXT_ID: RefCell<u64> = RefCell::default();
}

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
    #[error("RPC request error: {0}")]
    RpcRequestError(String),
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
pub struct RpcClient {
    pub provider: Provider,
    pub nodes_in_subnet: Option<u32>,
}

impl RpcClient {
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
        forward: Option<String>,
        payload: &str,
        max_response_bytes: u64,
        transform: Option<TransformContext>,
    ) -> RpcResult<String> {
        let transform = transform.unwrap_or(TransformContext::from_name(
            "cleanup_response".to_owned(),
            vec![],
        ));

        let mut headers = vec![HttpHeader {
            name: "Content-Type".to_string(),
            value: "application/json".to_string(),
        }];
        // add idempotency_key
        let idempotency_key = hash_with_sha256(payload);

        headers.push(HttpHeader {
            name: IDEMPOTENCY_KEY.to_string(),
            value: idempotency_key,
        });

        // add forward address
        if let Some(forward) = forward {
            headers.push(HttpHeader {
                name: FORWARD_KEY.to_string(),
                value: forward,
            });
        }

        let request = CanisterHttpRequestArgument {
            url: self.provider.url().to_string(),
            max_response_bytes: Some(max_response_bytes + HEADER_SIZE_LIMIT),
            // max_response_bytes: None,
            method: HttpMethod::POST,
            headers: headers,
            body: Some(payload.as_bytes().to_vec()),
            transform: Some(transform),
        };

        let url = self.provider.url();

        let cycles = get_http_request_cost(
            request.body.as_ref().map_or(0, |b| b.len() as u64),
            request.max_response_bytes.unwrap_or(2 * 1024 * 1024), // default 2Mb
        );

        log!(
            DEBUG,
            "Calling url: {url} with payload: {payload}. Cycles: {cycles}"
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

                match String::from_utf8(response.body) {
                    Ok(body) => Ok(body),
                    Err(error) => Err(RpcError::ParseError(error.to_string())),
                }
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
                Err(RpcError::RpcRequestError(format!("({r:?}) {m:?}")))
            }
        }
    }

    pub async fn rest_call(
        &self,
        forward: Option<String>,
        headers: Vec<HttpHeader>,
        req: RestReq,
        max_response_bytes: u64,
        transform: Option<TransformContext>,
    ) -> RpcResult<String> {
        let transform = transform.unwrap_or(TransformContext::from_name(
            "cleanup_response".to_owned(),
            vec![],
        ));

        let request = CanisterHttpRequestArgument {
            url: req.url.to_string(),
            max_response_bytes: Some(max_response_bytes + HEADER_SIZE_LIMIT),
            // max_response_bytes: None,
            method: req.method,
            headers: headers,
            body: req.body,
            transform: Some(transform),
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

                match String::from_utf8(response.body) {
                    Ok(body) => Ok(body),
                    Err(error) => Err(RpcError::ParseError(error.to_string())),
                }
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
                Err(RpcError::RpcRequestError(format!("({r:?}) {m:?}")))
            }
        }
    }

    pub fn next_request_id(&self) -> u64 {
        NEXT_ID.with(|next_id| {
            let mut next_id = next_id.borrow_mut();
            let id = *next_id;
            *next_id = next_id.wrapping_add(1);
            id
        })
    }

    pub async fn get_account(
        &self,
        address: String,
        ledger_version: Option<u64>,
        forward: Option<String>,
    ) -> RpcResult<String> {
        let mut headers = vec![HttpHeader {
            name: "Content-Type".to_string(),
            value: "application/json".to_string(),
        }];
        let version = "v1".to_string();
        let req_url = format!("{}/{}/accounts/{}", self.provider.url(), version, address);
        let req = RestReq {
            // headers: headers.to_owned(),
            method: HttpMethod::GET,
            url: req_url,
            body: None,
        };
        log!(DEBUG, "[rpc_client::get_account] request: {:?} ", req);
        // add idempotency_key
        let req_bytes = serde_json::to_vec(&req).expect("JSON serialization failed");
        let idempotency_key = hash_with_sha256_byte(&req_bytes);
        headers.push(HttpHeader {
            name: IDEMPOTENCY_KEY.to_string(),
            value: idempotency_key,
        });

        // add forward address
        if let Some(forward) = forward.to_owned() {
            headers.push(HttpHeader {
                name: FORWARD_KEY.to_string(),
                value: forward,
            });
        }
        log!(DEBUG, "[rpc_client::get_account] headers: {:?} ", headers);
        self.rest_call(forward, headers, req, 1000, None).await
    }

    pub async fn transfer_aptos(
        &self,
        txn: &SignedTransaction,
        forward: Option<String>,
    ) -> RpcResult<String> {
        let mut headers = vec![HttpHeader {
            name: "Content-Type".to_string(),
            value: "application/x.aptos.signed_transaction+bcs".to_string(),
        }];
        let version = "v1".to_string();
        let path = "transactions".to_string();
        let req_url = format!("{}/{}/{}", self.provider.url(), version, path);
        let txn_payload = bcs::to_bytes(txn).unwrap();
        let req = RestReq {
            // headers: headers.to_owned(),
            method: HttpMethod::POST,
            url: req_url,
            body: Some(txn_payload),
        };
        log!(DEBUG, "[rpc_client::transfer_aptos] request: {:?} ", req);
        // add idempotency_key
        let req_bytes = serde_json::to_vec(&req).expect("JSON serialization failed");
        let idempotency_key = hash_with_sha256_byte(&req_bytes);
        headers.push(HttpHeader {
            name: IDEMPOTENCY_KEY.to_string(),
            value: idempotency_key,
        });

        // add forward address
        if let Some(forward) = forward.to_owned() {
            headers.push(HttpHeader {
                name: FORWARD_KEY.to_string(),
                value: forward,
            });
        }
        log!(
            DEBUG,
            "[rpc_client::transfer_aptos] headers: {:?} ",
            headers
        );
        self.rest_call(forward, headers, req, 5000, None).await
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RestReq {
    // pub headers: Vec<HttpHeader>,
    // pub version: String
    pub method: HttpMethod,
    pub url: String,
    pub body: Option<Vec<u8>>,
}

#[cfg(test)]
mod test {}
