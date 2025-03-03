#![allow(unused)]
use crate::config::{mutate_config, read_config, NATIVE_KEY_TYPE};
use crate::constants::{
    COIN_MODULE, COIN_PKG_ID, DEFAULT_GAS_BUDGET, MINT_WITH_TICKET_FUNC, SUI_COIN,
    UPDATE_DESC_FUNC, UPDATE_ICON_FUNC, UPDATE_NAME_FUNC, UPDATE_SYMBOL_FUNC,
};
use crate::ic_log::{DEBUG, ERROR};

use crate::state::{mutate_state, read_state, AptosToken, UpdateType};

use aptos_crypto::ed25519::{PrivateKey, PublicKey};
use aptos_crypto::traits::signing_message;
use aptos_types::transaction::{RawTransaction, SignedTransaction};
use candid::CandidType;
use digest::Key;
// use ed25519_dalek::ed25519;
use futures::{stream, StreamExt};
use futures_core::Stream;
use ic_canister_log::log;
use ic_cdk::api;
use ic_cdk::api::management_canister::http_request::{
    http_request, CanisterHttpRequestArgument, HttpHeader, HttpMethod, HttpResponse,
    TransformContext,
};

use aptos_crypto::ed25519::{Ed25519PrivateKey, Ed25519PublicKey, Ed25519Signature};

use aptos_types::transaction::{authenticator::AuthenticationKey, SignatureCheckedTransaction};
use move_core_types::account_address::AccountAddress;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::future;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::ck_eddsa::{self, KeyType};
use anyhow::{Context, Result};
pub use aptos_api_types::deserialize_from_string;
use aptos_api_types::{Address, U64};

use super::constants::DEVNET_CHAIN_ID;
use super::error::AptosRouteError;
use super::rest_client::RestClient;
use super::tx_builder::TransactionBuilder;
use aptos_api_types::AptosError;
use move_core_types::{language_storage::StructTag, parser::parse_struct_tag};
use serde_bytes::ByteBuf;
pub type AptosResult<T> = Result<T, AptosRouteError>;

#[derive(Debug)]
pub enum LocalAccountAuthenticator {
    NativeKey(AccountKey),
    ChainKey(AccountKey),
}

impl LocalAccountAuthenticator {
    pub async fn sign_transaction(&self, txn: RawTransaction) -> SignedTransaction {
        match self {
            LocalAccountAuthenticator::NativeKey(account_key) => {
                let msg = signing_message(&txn).unwrap();
                log!(
                    DEBUG,
                    "[LocalAccountAuthenticator::NativeKey::sign_transaction] signing_message: {:?} ",
                    msg
                );
                let seed = read_state(|s| {
                    s.seeds
                        .get(&NATIVE_KEY_TYPE.to_string())
                        .unwrap_or_else(|| {
                            panic!("No key with name {:?}", &NATIVE_KEY_TYPE.to_string())
                        })
                });

                let sig_bytes = ck_eddsa::sign(msg.to_vec(), KeyType::Native(seed.to_vec()))
                    .await
                    .unwrap();
                let signature = Ed25519Signature::try_from(sig_bytes.as_slice()).unwrap();
                SignedTransaction::new(txn, account_key.public_key.clone(), signature)
            }
            LocalAccountAuthenticator::ChainKey(account_key) => {
                //TODO：

                let msg = signing_message(&txn).unwrap();
                log!(
                    DEBUG,
                    "[LocalAccountAuthenticator::ChainKey] signing_message: {:?} ",
                    msg
                );
                // devnet chain id is 174
                // let msg_hash = ck_eddsa::sha256(&msg);
                let sig_bytes = ck_eddsa::sign(msg.to_vec(), KeyType::ChainKey)
                    .await
                    .unwrap();
                let signature = Ed25519Signature::try_from(sig_bytes.as_slice()).unwrap();
                SignedTransaction::new(txn, account_key.public_key.clone(), signature)
            }
        }
    }
}

// impl<T: Into<AccountKey>> From<T> for LocalAccountAuthenticator {
//     fn from(key: T) -> Self {
//         Self::PrivateKey(key.into())
//     }
// }

/// LocalAccount represents an account on the Aptos blockchain. Internally it
/// holds the private / public key pair and the address of the account. You can
/// use this struct to help transact with the blockchain, e.g. by generating a
/// new account and signing transactions.
#[derive(Debug)]
pub struct LocalAccount {
    /// Address of the account.
    address: AccountAddress,
    /// Authenticator of the account
    auth: LocalAccountAuthenticator,
    /// Latest known sequence number of the account, it can be different from validator.
    sequence_number: AtomicU64,
}
impl LocalAccount {
    pub async fn local_account() -> AptosResult<LocalAccount> {
        let key_type = read_config(|c| c.get().key_type.to_owned());
        let account_key = AccountKey::account_key(key_type.to_owned()).await?;
        let address = account_key.authentication_key().account_address();
        let auth = match key_type {
            KeyType::ChainKey => LocalAccountAuthenticator::ChainKey(account_key),
            KeyType::Native(_) => LocalAccountAuthenticator::NativeKey(account_key),
        };

        let client = RestClient::new();
        let account = client
            .get_account(format!("{}", address), None, &client.forward)
            .await?;
        log!(
            DEBUG,
            "[types::LocalAccount::local_account] get_account ret: {:?}",
            account
        );
        // let tx_seq = read_config(|s| s.get().seqs.to_owned()).tx_seq;
        let sequence_number = AtomicU64::new(account.sequence_number);
        Ok(Self {
            address,
            auth,
            sequence_number,
        })
    }
    pub fn address(&self) -> AccountAddress {
        self.address
    }

    pub fn sequence_number(&self) -> u64 {
        self.sequence_number.load(Ordering::SeqCst)
    }

    pub fn increment_sequence_number(&self) -> u64 {
        self.sequence_number.fetch_add(1, Ordering::SeqCst)
    }

    pub async fn update_seq_from_chain(&mut self) -> AptosResult<u64> {
        //
        // let (provider, nodes, forward) = read_config(|s| {
        //     (
        //         s.get().rpc_provider.to_owned(),
        //         s.get().nodes_in_subnet,
        //         s.get().forward.to_owned(),
        //     )
        // });
        let client = RestClient::new();
        let account = client
            .get_account(format!("{}", self.address), None, &client.forward)
            .await?;
        log!(
            DEBUG,
            "[types::LocalAccount::update_seq_from_chain] get_account ret: {:?}",
            account
        );
        self.sequence_number = AtomicU64::new(account.sequence_number);
        Ok(account.sequence_number)
    }

    pub async fn sign_transaction(&self, txn: RawTransaction) -> SignedTransaction {
        self.auth.sign_transaction(txn).await
    }

    pub async fn sign_with_transaction_builder(
        &self,
        builder: TransactionBuilder,
    ) -> SignedTransaction {
        // let new_seq = self.increment_sequence_number()
        let raw_txn = builder
            .sender(self.address())
            .sequence_number(self.increment_sequence_number())
            .build();
        self.sign_transaction(raw_txn).await
    }
}
pub fn get_apt_primary_store_address(address: AccountAddress) -> AccountAddress {
    let mut bytes = address.to_vec();
    bytes.append(&mut AccountAddress::ONE.to_vec());
    bytes.push(0xFC);
    AccountAddress::from_bytes(aptos_crypto::hash::HashValue::sha3_256_of(&bytes).to_vec()).unwrap()
}

#[derive(Debug)]
pub struct AccountKey {
    // just for native key type
    private_key: Option<Ed25519PrivateKey>,
    public_key: Ed25519PublicKey,
    authentication_key: AuthenticationKey,
}
impl AccountKey {
    pub async fn account_key(key_type: KeyType) -> AptosResult<AccountKey> {
        match key_type {
            KeyType::ChainKey => {
                let public_key_bytes = ck_eddsa::public_key_ed25519(key_type).await.unwrap();
                let public_key = Ed25519PublicKey::try_from(public_key_bytes.as_slice())
                    .map_err(|e| AptosRouteError::AccountKeyError(e.into()))?;
                let authentication_key = AuthenticationKey::ed25519(&public_key);

                Ok(Self {
                    private_key: None,
                    public_key,
                    authentication_key,
                })
            }
            // just for test
            KeyType::Native(seed) => {
                let seed_32_bytes =
                    <[u8; 32]>::try_from(&seed[0..32]).expect("seed should be >= 32 bytes");
                let private_key = Ed25519PrivateKey::try_from(&seed_32_bytes[..])
                    .map_err(|e| AptosRouteError::AccountKeyError(e.into()))?;
                let public_key = Ed25519PublicKey::from(&private_key);
                let authentication_key = AuthenticationKey::ed25519(&public_key);

                Ok(Self {
                    private_key: Some(private_key),
                    public_key,
                    authentication_key,
                })
            }
        }
    }

    pub fn public_key(&self) -> &Ed25519PublicKey {
        &self.public_key
    }

    pub fn authentication_key(&self) -> AuthenticationKey {
        self.authentication_key
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Resource {
    #[serde(rename = "type", deserialize_with = "deserialize_resource_type")]
    pub resource_type: StructTag,
    pub data: serde_json::Value,
}

pub fn deserialize_from_prefixed_hex_string<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr,
    <T as FromStr>::Err: std::fmt::Display,
{
    use serde::de::Error;

    let s = <String>::deserialize(deserializer)?;
    s.trim_start_matches("0x")
        .parse::<T>()
        .map_err(D::Error::custom)
}

pub fn deserialize_resource_type<'de, D>(deserializer: D) -> Result<StructTag, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    let s = <String>::deserialize(deserializer)?;
    parse_struct_tag(&s).map_err(D::Error::custom)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, CandidType)]
pub struct Account {
    #[serde(deserialize_with = "deserialize_from_prefixed_hex_string")]
    pub authentication_key: AuthenticationKey,
    #[serde(deserialize_with = "deserialize_from_string")]
    pub sequence_number: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventHandle {
    counter: U64,
    guid: EventHandleGUID,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventHandleGUID {
    len_bytes: u8,
    guid: GUID,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GUID {
    id: ID,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ID {
    creation_num: U64,
    addr: Address,
}

use aptos_api_types::{
    X_APTOS_BLOCK_HEIGHT, X_APTOS_CHAIN_ID, X_APTOS_CURSOR, X_APTOS_EPOCH,
    X_APTOS_LEDGER_OLDEST_VERSION, X_APTOS_LEDGER_TIMESTAMP, X_APTOS_LEDGER_VERSION,
    X_APTOS_OLDEST_BLOCK_HEIGHT,
};

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, CandidType)]
pub struct State {
    pub chain_id: u8,
    pub epoch: u64,
    pub version: u64,
    pub timestamp_usecs: u64,
    pub oldest_ledger_version: u64,
    pub oldest_block_height: u64,
    pub block_height: u64,
    pub cursor: Option<String>,
}

impl State {
    pub fn from_headers(headers: &Vec<HttpHeader>) -> anyhow::Result<Self> {
        // let mut header_map = std::collections::HashMap::new();
        // for header in headers {
        //     header_map.insert(header.name.as_str(), header.value.as_str());
        // }
        let header_map: HashMap<_, _> = headers
            .iter()
            .map(|h| (h.name.as_str(), h.value.as_str()))
            .collect();

        let maybe_chain_id = header_map
            .get(X_APTOS_CHAIN_ID)
            .and_then(|s| s.parse().ok());
        let maybe_version = header_map
            .get(X_APTOS_LEDGER_VERSION)
            .and_then(|s| s.parse().ok());
        let maybe_timestamp = header_map
            .get(X_APTOS_LEDGER_TIMESTAMP)
            .and_then(|s| s.parse().ok());
        let maybe_epoch = header_map.get(X_APTOS_EPOCH).and_then(|s| s.parse().ok());
        let maybe_oldest_ledger_version = header_map
            .get(X_APTOS_LEDGER_OLDEST_VERSION)
            .and_then(|s| s.parse().ok());
        let maybe_block_height = header_map
            .get(X_APTOS_BLOCK_HEIGHT)
            .and_then(|s| s.parse().ok());
        let maybe_oldest_block_height = header_map
            .get(X_APTOS_OLDEST_BLOCK_HEIGHT)
            .and_then(|s| s.parse().ok());
        let cursor = header_map.get(X_APTOS_CURSOR).map(|s| s.to_string());

        let state = if let (
            Some(chain_id),
            Some(version),
            Some(timestamp_usecs),
            Some(epoch),
            Some(oldest_ledger_version),
            Some(block_height),
            Some(oldest_block_height),
            cursor,
        ) = (
            maybe_chain_id,
            maybe_version,
            maybe_timestamp,
            maybe_epoch,
            maybe_oldest_ledger_version,
            maybe_block_height,
            maybe_oldest_block_height,
            cursor,
        ) {
            Self {
                chain_id,
                epoch,
                version,
                timestamp_usecs,
                oldest_ledger_version,
                block_height,
                oldest_block_height,
                cursor,
            }
        } else {
            anyhow::bail!(
                "Failed to build State from headers due to missing values in response. \
                Chain ID: {:?}, Version: {:?}, Timestamp: {:?}, Epoch: {:?}, \
                Oldest Ledger Version: {:?}, Block Height: {:?} Oldest Block Height: {:?}",
                maybe_chain_id,
                maybe_version,
                maybe_timestamp,
                maybe_epoch,
                maybe_oldest_ledger_version,
                maybe_block_height,
                maybe_oldest_block_height,
            )
        };

        Ok(state)
    }
}

pub fn parse_state(response: &HttpResponse) -> AptosResult<State> {
    Ok(State::from_headers(&response.headers)?)
}

pub fn parse_state_optional(response: &HttpResponse) -> Option<State> {
    State::from_headers(&response.headers)
        .map(Some)
        .unwrap_or(None)
}

pub fn parse_error(response: HttpResponse) -> AptosRouteError {
    // let status_code: u16 = response.status.into();
    let status_code: u16 = response.status.to_owned().0.try_into().unwrap_or(500);
    let maybe_state = parse_state_optional(&response);

    match serde_json::from_slice::<AptosError>(response.body.as_slice()) {
        Ok(error) => (error, maybe_state, status_code).into(),
        Err(e) => AptosRouteError::Json(e),
    }
}

#[derive(CandidType, Deserialize, Serialize, Clone, Debug)]
pub struct TxOptions {
    pub max_gas_amount: u64,
    pub gas_unit_price: u64,
    /// This is the number of seconds from now you're willing to wait for the
    /// transaction to be committed.
    pub timeout_secs: u64,
    pub chain_id: u8,
}

impl Default for TxOptions {
    fn default() -> Self {
        Self {
            max_gas_amount: 5_000,
            gas_unit_price: 150,
            timeout_secs: 500,
            chain_id: DEVNET_CHAIN_ID,
        }
    }
}
#[derive(CandidType, Clone, Debug, Serialize, Deserialize)]
pub struct CreateTokenReq {
    pub token_id: String,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub icon_uri: String,
    pub max_supply: Option<u128>,
    pub project_uri: String,
}

#[derive(CandidType, Clone, Debug, Serialize, Deserialize)]
pub struct UpdateMetaReq {
    pub fa_obj: String,
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub decimals: Option<u8>,
    pub icon_uri: Option<String>,
    pub project_uri: Option<String>,
}

#[derive(CandidType, Clone, Debug, Serialize, Deserialize)]
pub struct MintTokenReq {
    pub ticket_id: String,
    pub fa_obj: String,
    pub recipient: String,
    pub mint_acmount: u64,
}

#[derive(CandidType, Clone, Debug, Serialize, Deserialize)]
pub struct BurnTokenReq {
    pub fa_obj: String,
    pub burn_acmount: u64,
    pub memo: Option<String>,
}

#[derive(CandidType, Clone, Debug, Serialize, Deserialize)]
pub struct TransferReq {
    pub recipient: String,
    pub amount: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, CandidType)]
pub enum TxReq {
    CreateToken(CreateTokenReq),
    UpdateMeta(UpdateMetaReq),
    MintToken(MintTokenReq),
    BurnToken(BurnTokenReq),
    CollectFee(u64),
    RemoveTicket(String),
    // Transfer(TransferReq),
}
