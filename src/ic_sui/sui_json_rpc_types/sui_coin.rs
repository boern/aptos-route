
use std::collections::HashMap;

// use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use crate::ic_sui::sui_types::{
    base_types::{ObjectID, ObjectRef, SequenceNumber},
    coin::CoinMetadata,
    digests::{ObjectDigest, TransactionDigest},
    error::SuiError,
    object::Object,
    transaction::EpochId,
};

use super::Page;


use crate::ic_sui::sui_types::sui_serde::BigInt;
use crate::ic_sui::sui_types::sui_serde::SequenceNumber as AsSequenceNumber;

pub type CoinPage = Page<Coin, ObjectID>;

#[serde_as]
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Balance {
    pub coin_type: String,
    pub coin_object_count: usize,
    #[serde_as(as = "BigInt<u128>")]
    pub total_balance: u128,
    #[serde_as(as = "HashMap<BigInt<u64>, BigInt<u128>>")]
    pub locked_balance: HashMap<EpochId, u128>,
}

impl Balance {
    pub fn zero(coin_type: String) -> Self {
        Self {
            coin_type,
            coin_object_count: 0,
            total_balance: 0,
            locked_balance: HashMap::new(),
        }
    }
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Coin {
    pub coin_type: String,
    pub coin_object_id: ObjectID,
    #[serde_as(as = "AsSequenceNumber")]
    pub version: SequenceNumber,
    pub digest: ObjectDigest,

    #[serde_as(as = "BigInt<u64>")]
    pub balance: u64,
    pub previous_transaction: TransactionDigest,
}

impl Coin {
    pub fn object_ref(&self) -> ObjectRef {
        (self.coin_object_id, self.version, self.digest)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SuiCoinMetadata {
    /// Number of decimal places the coin uses.
    pub decimals: u8,
    /// Name for the token
    pub name: String,
    /// Symbol for the token
    pub symbol: String,
    /// Description of the token
    pub description: String,
    /// URL for the token logo
    pub icon_url: Option<String>,
    /// Object id for the CoinMetadata object
    pub id: Option<ObjectID>,
}

impl TryFrom<Object> for SuiCoinMetadata {
    type Error = SuiError;
    fn try_from(object: Object) -> Result<Self, Self::Error> {
        let metadata: CoinMetadata = object.try_into()?;
        let CoinMetadata {
            decimals,
            name,
            symbol,
            description,
            icon_url,
            id,
        } = metadata;
        Ok(Self {
            id: Some(*id.object_id()),
            decimals,
            name,
            symbol,
            description,
            icon_url,
        })
    }
}
