// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use super::types::State;
use aptos_api_types::AptosError;
// use reqwest::StatusCode;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AptosRouteError {
    #[error("http outcall error: {0}")]
    HttpCallError(String),
    #[error("API error {0}")]
    Api(AptosErrorResponse),
    #[error("BCS ser/de error {0}")]
    Bcs(bcs::Error),
    #[error("JSON er/de error {0}")]
    Json(serde_json::Error),
    #[error("URL Parse error {0}")]
    UrlParse(url::ParseError),
    #[error("Timeout waiting for transaction {0}")]
    Timeout(&'static str),
    #[error("Unknown error {0}")]
    ParseError(String),
    #[error("Account key error {0}")]
    AccountKeyError(anyhow::Error),
    #[error("Unknown error {0}")]
    Unknown(anyhow::Error),
    // #[error("HTTP error {0}: {1}")]
    // Http(u16, reqwest::Error),
}

impl From<(AptosError, Option<State>, u16)> for AptosRouteError {
    fn from((error, state, status_code): (AptosError, Option<State>, u16)) -> Self {
        Self::Api(AptosErrorResponse {
            error,
            state,
            status_code,
        })
    }
}

impl From<bcs::Error> for AptosRouteError {
    fn from(err: bcs::Error) -> Self {
        Self::Bcs(err)
    }
}

impl From<url::ParseError> for AptosRouteError {
    fn from(err: url::ParseError) -> Self {
        Self::UrlParse(err)
    }
}

impl From<serde_json::Error> for AptosRouteError {
    fn from(err: serde_json::Error) -> Self {
        Self::Json(err)
    }
}

impl From<anyhow::Error> for AptosRouteError {
    fn from(err: anyhow::Error) -> Self {
        Self::Unknown(err)
    }
}

// impl From<reqwest::Error> for RestError {
//     fn from(err: reqwest::Error) -> Self {
//         if let Some(status) = err.status() {
//             RestError::Http(status, err)
//         } else {
//             RestError::Unknown(err.into())
//         }
//     }
// }

#[derive(Debug)]
pub struct AptosErrorResponse {
    pub error: AptosError,
    pub state: Option<State>,
    pub status_code: u16,
}

impl std::fmt::Display for AptosErrorResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error)
    }
}
