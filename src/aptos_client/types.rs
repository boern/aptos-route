#![allow(unused)]
use crate::config::{mutate_config, read_config, AptosPortAction, KEY_TYPE_NAME};
use crate::constants::{
    BURN_FUNC, COIN_MODULE, COIN_PKG_ID, DEFAULT_GAS_BUDGET, MINT_FUNC, MINT_WITH_TICKET_FUNC,
    SUI_COIN, UPDATE_DESC_FUNC, UPDATE_ICON_FUNC, UPDATE_NAME_FUNC, UPDATE_SYMBOL_FUNC,
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
    http_request, CanisterHttpRequestArgument, HttpHeader, HttpMethod, TransformContext,
};

use aptos_crypto::ed25519::{Ed25519PrivateKey, Ed25519PublicKey, Ed25519Signature};

use aptos_types::transaction::{authenticator::AuthenticationKey, SignatureCheckedTransaction};
use move_core_types::account_address::AccountAddress;
use serde::Deserialize;
use serde::Serialize;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::future;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::ck_eddsa::{self, KeyType};

use serde_bytes::ByteBuf;

use super::tx_builder::TransactionBuilder;

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
                    s.seeds.get(&KEY_TYPE_NAME.to_string()).unwrap_or_else(|| {
                        panic!("No key with name {:?}", &KEY_TYPE_NAME.to_string())
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
    pub async fn local_account(key_type: KeyType) -> Self {
        let account_key = AccountKey::account_key(key_type.to_owned()).await.unwrap();
        let address = account_key.authentication_key().account_address();
        let auth = match key_type {
            KeyType::ChainKey => LocalAccountAuthenticator::ChainKey(account_key),
            KeyType::Native(_) => LocalAccountAuthenticator::NativeKey(account_key),
        };

        let tx_seq = read_config(|s| s.get().seqs.to_owned()).tx_seq;
        let sequence_number = AtomicU64::new(tx_seq);
        Self {
            address,
            auth,
            sequence_number,
        }
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

    pub async fn sign_transaction(&self, txn: RawTransaction) -> SignedTransaction {
        self.auth.sign_transaction(txn).await
    }

    pub async fn sign_with_transaction_builder(
        &self,
        builder: TransactionBuilder,
    ) -> SignedTransaction {
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
    // pub fn generate<R>(rng: &mut R) -> Self
    // where
    //     R: rand_core::RngCore + rand_core::CryptoRng,
    // {
    //     let private_key = Ed25519PrivateKey::generate(rng);
    //     Self::from_private_key(private_key)
    // }

    pub async fn account_key(key_type: KeyType) -> Result<AccountKey, String> {
        match key_type {
            KeyType::ChainKey => {
                let public_key_bytes = ck_eddsa::public_key_ed25519(key_type).await.unwrap();
                let public_key = Ed25519PublicKey::try_from(public_key_bytes.as_slice())
                    .map_err(|e| e.to_string())?;
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
                let private_key = Ed25519PrivateKey::try_from(&seed_32_bytes[..]).unwrap();
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

    // pub fn private_key(&self) -> &Ed25519PrivateKey {
    //     &self.private_key.expect("No private key")
    // }

    pub fn public_key(&self) -> &Ed25519PublicKey {
        &self.public_key
    }

    pub fn authentication_key(&self) -> AuthenticationKey {
        self.authentication_key
    }
}

//Note: The sui address must be: hash(signature schema + sender public key bytes)
// pub async fn aptos_route_address(key_type: KeyType) -> Result<AccountAddress, String> {
//     let account_key = AccountKey::from_chain_key(key_type).await;
//     let account_address = account_key.authentication_key().account_address();
//     // let pk = Ed25519PublicKey(ed25519_dalek::PublicKey::from_bytes(&pk_bytes).unwrap());
//     // let authentication_key = AuthenticationKey::ed25519(&public_key);
//     // let address = AccountAddress::new(pk_bytes.try_into().map_err(|_| "Invalid length")?);

//     Ok(account_address)
// }
// impl From<Ed25519PrivateKey> for AccountKey {
//     fn from(private_key: Ed25519PrivateKey) -> Self {
//         Self::from_private_key(private_key)
//     }
// }

// #[derive(Debug)]
// pub struct Ed25519ChainKey {
//     authentication_key: AuthenticationKey,
// }
