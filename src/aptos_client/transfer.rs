use anyhow::Result;
use aptos_types::{
    chain_id::ChainId,
    transaction::{EntryFunction, SignedTransaction, TransactionPayload},
};
use move_core_types::{
    account_address::AccountAddress,
    identifier::Identifier,
    language_storage::{ModuleId, TypeTag},
};
use std::str::FromStr;

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
    let now_s = api::time() / 1_000_000_000;
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

    pub timeout_secs: u64,

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
