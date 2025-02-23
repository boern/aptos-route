// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Context, Result};
use aptos_types::{
    chain_id::ChainId,
    transaction::{EntryFunction, SignedTransaction, TransactionPayload},
};
use move_core_types::{
    account_address::AccountAddress,
    identifier::Identifier,
    language_storage::{ModuleId, TypeTag},
};
use std::{
    str::FromStr,
    // time::{SystemTime, UNIX_EPOCH},
};

use super::{tx_builder::TransactionBuilder, LocalAccount};

pub async fn get_signed_transfer_txn(
    chain_id: u8,
    from_account: &mut LocalAccount,
    to_account: AccountAddress,
    amount: u64,
    options: Option<TransferOptions<'_>>,
) -> Result<SignedTransaction> {
    let options = options.unwrap_or_default();
    use ic_cdk::api;

    // 获取当前时间戳（纳秒）
    let now_ns = api::time();

    // 转换为秒
    let now_s = now_ns / 1_000_000_000;
    let transaction_builder = TransactionBuilder::new(
        TransactionPayload::EntryFunction(EntryFunction::new(
            ModuleId::new(
                AccountAddress::ONE,
                Identifier::new("aptos_account").unwrap(),
            ),
            Identifier::new("transfer_coins").unwrap(),
            vec![TypeTag::from_str(options.coin_type).unwrap()],
            vec![
                bcs::to_bytes(&to_account).unwrap(),
                bcs::to_bytes(&amount).unwrap(),
            ],
        )),
        // SystemTime::now()
        //     .duration_since(UNIX_EPOCH)
        //     .unwrap()
        //     .as_secs()
        //     + options.timeout_secs,
        // 300,
        now_s + options.timeout_secs,
        ChainId::new(chain_id),
    )
    .sender(from_account.address())
    .sequence_number(from_account.sequence_number())
    .max_gas_amount(options.max_gas_amount)
    .gas_unit_price(options.gas_unit_price);
    let signed_txn = from_account
        .sign_with_transaction_builder(transaction_builder)
        .await;
    Ok(signed_txn)
}
pub struct TransferOptions<'a> {
    pub max_gas_amount: u64,

    pub gas_unit_price: u64,

    /// This is the number of seconds from now you're willing to wait for the
    /// transaction to be committed.
    pub timeout_secs: u64,

    /// This is the coin type to transfer.
    pub coin_type: &'a str,
}

impl<'a> Default for TransferOptions<'a> {
    fn default() -> Self {
        Self {
            max_gas_amount: 5_000,
            gas_unit_price: 150,
            timeout_secs: 500,
            coin_type: "0x1::aptos_coin::AptosCoin",
        }
    }
}
