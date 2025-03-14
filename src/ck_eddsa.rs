use crate::aptos_client::constants::EDDSA_SIGN_COST;
use crate::config::read_config;
use crate::ic_log::DEBUG;
// use crate::ic_sui::ck_eddsa::KeyType;
use crate::state::{mutate_state, read_state};

use aptos_crypto::ed25519::{Ed25519PrivateKey, Ed25519PublicKey};

use candid::Principal;
use candid::{CandidType, Deserialize};
use ic_canister_log::log;
// use ic_crypto_ed25519::DerivationPath;
use ic_management_canister_types::{
    SchnorrAlgorithm, SchnorrKeyId, SchnorrPublicKeyArgs, SchnorrPublicKeyResult,
    SignWithSchnorrArgs, SignWithSchnorrResult,
};
use ic_stable_structures::storable::Bound;
use ic_stable_structures::Storable;

use serde::Serialize;
use serde_bytes::ByteBuf;
use sha2::Digest;
use std::borrow::Cow;
use std::vec;

#[derive(
    Default, Hash, Eq, Ord, PartialEq, PartialOrd, CandidType, Deserialize, Serialize, Debug, Clone,
)]
pub enum KeyType {
    #[default]
    ChainKey,
    Native(Vec<u8>),
}

impl Storable for KeyType {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        let bytes = bincode::serialize(&self).expect("failed to serialize KeyType");
        Cow::Owned(bytes)
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        bincode::deserialize(bytes.as_ref()).expect("failed to deserialize KeyType")
    }

    const BOUND: Bound = Bound::Unbounded;
}

// cache the sui route address to save the cycles
pub async fn public_key_ed25519(key_type: KeyType) -> Result<Vec<u8>, String> {
    let address = read_state(|s| s.route_addresses.get(&key_type));
    log!(
        DEBUG,
        "[rpc_client::public_key_ed25519] key type: {:?} and value from state: {:?} ",
        key_type,
        address,
    );

    match address {
        Some(address) => Ok(address),
        // create new address
        None => {
            let (chain_id, schnorr_key_name) = read_config(|s| {
                (
                    s.get().chain_id.to_owned(),
                    s.get().schnorr_key_name.to_owned(),
                    // s.get().sui_route_address.get(&key_type).cloned(),
                )
            });
            let derived_path = vec![ByteBuf::from(chain_id.as_bytes())];
            let pk = pub_key_ed25519(key_type.to_owned(), schnorr_key_name, derived_path).await;
            //save the new address
            mutate_state(|s| {
                s.route_addresses.insert(key_type, pk.to_owned());
            });
            Ok(pk)
        }
    }
}

pub async fn sign(msg: Vec<u8>, key_type: KeyType) -> Result<Vec<u8>, String> {
    let (chain_id, schnorr_key_name) = read_config(|s| {
        (
            s.get().chain_id.to_owned(),
            s.get().schnorr_key_name.to_owned(),
        )
    });
    let derived_path = vec![ByteBuf::from(chain_id.as_bytes())];
    // let msg = msg.as_bytes().to_vec();
    let signature = sign_with_eddsa(&key_type, schnorr_key_name, derived_path, msg).await;
    // let sig = String::from_utf8_lossy(&signature).to_string();
    Ok(signature)
}

/// Fetches the ed25519 public key from the schnorr canister.
pub async fn pub_key_ed25519(
    key_type: KeyType,
    key_name: String,
    derivation_path: Vec<ByteBuf>,
) -> Vec<u8> {
    match key_type {
        KeyType::ChainKey => {
            let res: Result<(SchnorrPublicKeyResult,), _> = ic_cdk::call(
                Principal::management_canister(),
                "schnorr_public_key",
                (SchnorrPublicKeyArgs {
                    canister_id: None,
                    derivation_path: derivation_path
                        .iter()
                        .map(|p| p.clone().into_vec())
                        .collect(),
                    key_id: SchnorrKeyId {
                        algorithm: SchnorrAlgorithm::Ed25519,
                        name: key_name,
                    },
                },),
            )
            .await;

            res.unwrap().0.public_key
        }
        KeyType::Native(seed) => {
            // let derivation_path = derivation_path_ed25519(&ic_cdk::api::id(), &derivation_path);
            native_public_key_ed25519(seed)
        }
    }
}

// just for testing
fn native_public_key_ed25519(seed: Vec<u8>) -> Vec<u8> {
    let seed_32_bytes = <[u8; 32]>::try_from(&seed[0..32]).expect("seed should be >= 32 bytes");
    let private_key = Ed25519PrivateKey::try_from(&seed_32_bytes[..]).unwrap();
    let public_key = Ed25519PublicKey::from(&private_key);
    public_key.to_bytes().to_vec()
}

/// Signs a message with an ed25519 key.
pub async fn sign_with_eddsa(
    key_type: &KeyType,
    key_name: String,
    derivation_path: Vec<ByteBuf>,
    message: Vec<u8>,
) -> Vec<u8> {
    match key_type {
        KeyType::ChainKey => {
            let res: Result<(SignWithSchnorrResult,), _> = ic_cdk::api::call::call_with_payment(
                Principal::management_canister(),
                "sign_with_schnorr",
                (SignWithSchnorrArgs {
                    message,
                    derivation_path: derivation_path
                        .iter()
                        .map(|p| p.clone().into_vec())
                        .collect(),
                    key_id: SchnorrKeyId {
                        name: key_name,
                        algorithm: SchnorrAlgorithm::Ed25519,
                    },
                },),
                // https://internetcomputer.org/docs/current/references/t-sigs-how-it-works/#fees-for-the-t-schnorr-production-key
                // 26_153_846_153,
                EDDSA_SIGN_COST as u64,
            )
            .await;

            res.unwrap().0.signature
        }
        KeyType::Native(seed) => {
            // let derivation_path = derivation_path_ed25519(&ic_cdk::api::id(), &derivation_path);
            sign_with_native_ed25519(seed, message)
        }
    }
}

// just for testing
fn sign_with_native_ed25519(seed: &Vec<u8>, message: Vec<u8>) -> Vec<u8> {
    let seed_32_bytes = <[u8; 32]>::try_from(&seed[0..32]).expect("seed should be >= 32 bytes");
    let private_key = Ed25519PrivateKey::try_from(&seed_32_bytes[..]).unwrap();
    // let secret_key = ed25519_dalek::SecretKey::from_bytes(bytes).unwrap();
    let public_key = Ed25519PublicKey::from(&private_key);
    let expanded_secret_key: ed25519_dalek::ExpandedSecretKey =
        ed25519_dalek::ExpandedSecretKey::from(&private_key.0);
    let sig = expanded_secret_key.sign(message.as_ref(), &public_key.0);
    sig.to_bytes().to_vec()
}

pub fn sha256(input: &[u8]) -> [u8; 32] {
    let mut hasher = sha2::Sha256::new();
    hasher.update(input);
    hasher.finalize().into()
}

pub fn hash_with_sha256(input: &Vec<u8>) -> String {
    let value = sha256(input);
    hex::encode(value)
}

#[cfg(test)]
mod tests {
    // use super::*;
    // use crate::types::Pubkey;

    #[test]
    fn test_sign_and_verify_native_schnorr_ed25519() {}
}
