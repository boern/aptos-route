use aptos_types::transaction::SignedTransaction;
use ic_cdk::api::management_canister::http_request::{HttpHeader, HttpMethod};
use serde_json::{json, Value};
use std::fmt;

use crate::ck_eddsa::hash_with_sha256;
use crate::config::read_config;
use serde::Deserialize;
use serde::Serialize;

use super::constants::APTOS_API_VERSION;
use super::constants::IDEMPOTENCY_KEY;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum AtosRequest {
    GetAccount { address: String },
    GetAccountBalance { address: String, asset_type: String },
    SubmitTransaction { txn: SignedTransaction },
    GetTransactionByHash { txn_hash: String },
}

impl fmt::Display for AtosRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let method = match self {
            AtosRequest::GetAccount { address } => format!("/accounts/{}", address),
            AtosRequest::GetAccountBalance {
                address,
                asset_type,
            } => format!("/accounts/{}/balance/{}", address, asset_type),
            AtosRequest::SubmitTransaction { .. } => format!("/transactions"),
            AtosRequest::GetTransactionByHash { txn_hash } => {
                format!("/transactions/by_hash/{}", txn_hash)
            }
        };

        write!(f, "{method}")
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RestReq {
    pub method: HttpMethod,
    pub headers: Vec<HttpHeader>,
    pub url: String,
    pub body: Option<Vec<u8>>,
}

pub fn build_rest_req(req: AtosRequest) -> RestReq {
    let provider = read_config(|s| s.get().rpc_provider.to_owned());
    let mut req = match req {
        AtosRequest::GetAccount { address } => {
            let headers = vec![HttpHeader {
                name: "Content-Type".to_string(),
                value: "Accept: application/json, application/x-bcs".to_string(),
            }];
            RestReq {
                method: HttpMethod::GET,
                headers,
                url: format!(
                    "{}/{}/accounts/{}",
                    provider.url(),
                    APTOS_API_VERSION,
                    address
                ),
                body: None,
            }
        }
        AtosRequest::GetAccountBalance {
            address,
            asset_type,
        } => {
            let headers = vec![HttpHeader {
                name: "Content-Type".to_string(),
                value: "Accept: application/json, application/x-bcs".to_string(),
            }];
            RestReq {
                method: HttpMethod::GET,
                headers,
                url: format!(
                    "{}/{}/accounts/{}/balance/{}",
                    provider.url(),
                    APTOS_API_VERSION,
                    address,
                    asset_type
                ),
                body: None,
            }
        }
        AtosRequest::SubmitTransaction { txn } => {
            let headers = vec![HttpHeader {
                name: "Content-Type".to_string(),
                value: "Accept: application/json, application/x-bcs".to_string(),
            }];
            let txn_payload = bcs::to_bytes(&txn).expect("Failed to serialize transaction");
            RestReq {
                method: HttpMethod::POST,
                headers,
                url: format!("{}/{}/transactions", provider.url(), APTOS_API_VERSION),
                body: Some(txn_payload),
            }
        }
        AtosRequest::GetTransactionByHash { txn_hash } => {
            let headers = vec![HttpHeader {
                name: "Content-Type".to_string(),
                value: "Accept: application/json, application/x-bcs".to_string(),
            }];
            RestReq {
                method: HttpMethod::GET,
                headers,
                url: format!(
                    "{}/{}/transactions/by_hash/{}",
                    provider.url(),
                    APTOS_API_VERSION,
                    txn_hash
                ),
                body: None,
            }
        }
    };
    // add idempotency_key
    let req_bytes = serde_json::to_vec(&req).expect("JSON serialization failed");
    let idempotency_key = hash_with_sha256(&req_bytes);
    let mut headers_with_idempotency = req.headers;
    headers_with_idempotency.push(HttpHeader {
        name: IDEMPOTENCY_KEY.to_string(),
        value: idempotency_key,
    });
    // update req headers
    req.headers = headers_with_idempotency;
    req
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_rest_req() {}
}
