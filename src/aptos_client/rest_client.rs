#![allow(unused)]
use crate::aptos_client::constants::{
    FORWARD_KEY, HEADER_SIZE_LIMIT, IDEMPOTENCY_KEY, TRANSACTION_RESPONSE_SIZE_ESTIMATE,
};
use crate::aptos_client::error::AptosRouteError;
use crate::aptos_client::request::{self, build_rest_req};
use crate::aptos_client::utils::get_http_request_cost;
use crate::config::{mutate_config, read_config};
use crate::constants::{
    COIN_MODULE, COIN_PKG_ID, DEFAULT_GAS_BUDGET, MINT_WITH_TICKET_FUNC, SUI_COIN,
    UPDATE_DESC_FUNC, UPDATE_ICON_FUNC, UPDATE_NAME_FUNC, UPDATE_SYMBOL_FUNC,
};
use crate::ic_log::{DEBUG, ERROR};

use crate::service::forward;
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
    pub forward: Option<String>,
}

impl RestClient {
    pub fn new() -> Self {
        let (provider, nodes_in_subnet, forward) = read_config(|s| {
            (
                s.get().rpc_provider.to_owned(),
                s.get().nodes_in_subnet,
                s.get().forward.to_owned(),
            )
        });
        Self {
            provider,
            forward,
            nodes_in_subnet: Some(nodes_in_subnet),
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
        forward: &Option<String>,
    ) -> AptosResult<HttpResponse> {
        let transform = transform.unwrap_or(TransformContext::from_name(
            "cleanup_response".to_owned(),
            vec![],
        ));

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
                Err(AptosRouteError::HttpCallError(format!("({r:?}) {m:?}")))
            }
        }
    }

    pub async fn get_account(
        &self,
        address: String,
        ledger_version: Option<u64>,
        forward: &Option<String>,
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
        forward: &Option<String>,
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
            .map_err(|e| AptosRouteError::ParseError(e.to_string()))?
            .parse::<u64>()
            .map_err(|e| AptosRouteError::ParseError(e.to_string()))?;

        Ok(balance)
    }

    pub async fn get_fa_obj(
        &self,
        view_func: String,
        token_id: String,
        forward: &Option<String>,
    ) -> AptosResult<Vec<String>> {
        let mut req = build_rest_req(request::AtosRequest::GetFaObj {
            view_func,
            token_id,
        });
        log!(DEBUG, "[rpc_client::get_fa_obj] request: {:?} ", req);

        let response = self.call(req, 1000, None, forward).await?;
        match self.json::<Vec<String>>(response) {
            Ok(response) => Ok(response.into_inner()),
            Err(e) => {
                log!(DEBUG, "[rpc_client::get_fa_obj] response error: {:?}", e);
                Err(e.into())
            }
        }
    }

    pub async fn summit_tx(
        &self,
        txn: &SignedTransaction,
        forward: &Option<String>,
    ) -> AptosResult<PendingTransaction> {
        let mut req = build_rest_req(request::AtosRequest::SubmitTransaction {
            txn: txn.to_owned(),
        });
        log!(DEBUG, "[rpc_client::summit_tx] request: {:?} ", req);

        let response = self.call(req, 5000, None, forward).await?;
        log!(DEBUG, "[rpc_client::summit_tx] response: {:?} ", response);
        match self.json::<PendingTransaction>(response) {
            Ok(response) => Ok(response.into_inner()),
            Err(e) => {
                log!(DEBUG, "[rpc_client::summit_tx] response error: {:?}", e);
                Err(e.into())
            }
        }
    }

    pub async fn get_transaction_by_hash(
        &self,
        txn_hash: String,
        forward: &Option<String>,
    ) -> AptosResult<Transaction> {
        let mut req = build_rest_req(request::AtosRequest::GetTransactionByHash { txn_hash });
        log!(
            DEBUG,
            "[rpc_client::get_transaction_by_hash] request: {:?} ",
            req
        );

        let response = self
            .call(req, TRANSACTION_RESPONSE_SIZE_ESTIMATE, None, forward)
            .await?;
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
            AptosRouteError::ParseError(format!("Invalid status code: {:?}", response.status))
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
mod test {}
