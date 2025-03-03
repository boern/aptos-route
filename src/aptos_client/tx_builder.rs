
use std::str::FromStr;


use aptos_global_constants::{GAS_UNIT_PRICE, MAX_GAS_AMOUNT};

use ic_canister_log::log;

use crate::{
    config::read_config,
    constants::{
        BURN_TOKEN_FUNC, COLLECT_FEE_FUNC, CREATE_FUNGIBLE_ASSET, 
        MINT_WITH_TICKET_FUNC, REMOVE_TICKET_FUNC, UPDATE_META_FUNC,
    },
    ic_log::DEBUG,
    state::read_state,
};
use anyhow::Result;
use aptos_cached_packages::aptos_stdlib;

use aptos_types::{
    chain_id::ChainId,
    transaction::{
        script::{EntryFunction, Script},
        RawTransaction, SignedTransaction, TransactionPayload,
    },
};
use ic_cdk::api;
use move_core_types::{
    account_address::AccountAddress, identifier::Identifier, language_storage::ModuleId,
};

use super::{ LocalAccount, TxOptions, TxReq};

pub struct TransactionBuilder {
    sender: Option<AccountAddress>,
    sequence_number: Option<u64>,
    payload: TransactionPayload,
    max_gas_amount: u64,
    gas_unit_price: u64,
    expiration_timestamp_secs: u64,
    chain_id: ChainId,
}

impl TransactionBuilder {
    pub fn new(
        payload: TransactionPayload,
        expiration_timestamp_secs: u64,
        chain_id: ChainId,
    ) -> Self {
        Self {
            payload,
            chain_id,
            expiration_timestamp_secs,
            // TODO(Gas): double check this
            max_gas_amount: MAX_GAS_AMOUNT,
            gas_unit_price: std::cmp::max(GAS_UNIT_PRICE, 1),
            sender: None,
            sequence_number: None,
        }
    }

    pub fn sender(mut self, sender: AccountAddress) -> Self {
        self.sender = Some(sender);
        self
    }

    pub fn sequence_number(mut self, sequence_number: u64) -> Self {
        self.sequence_number = Some(sequence_number);
        self
    }

    pub fn max_gas_amount(mut self, max_gas_amount: u64) -> Self {
        self.max_gas_amount = max_gas_amount;
        self
    }

    pub fn gas_unit_price(mut self, gas_unit_price: u64) -> Self {
        self.gas_unit_price = gas_unit_price;
        self
    }

    pub fn chain_id(mut self, chain_id: ChainId) -> Self {
        self.chain_id = chain_id;
        self
    }

    pub fn expiration_timestamp_secs(mut self, expiration_timestamp_secs: u64) -> Self {
        self.expiration_timestamp_secs = expiration_timestamp_secs;
        self
    }

    pub fn build(self) -> RawTransaction {
        RawTransaction::new(
            self.sender.expect("sender must have been set"),
            self.sequence_number
                .expect("sequence number must have been set"),
            self.payload,
            self.max_gas_amount,
            self.gas_unit_price,
            self.expiration_timestamp_secs,
            self.chain_id,
        )
    }
}

#[derive(Clone, Debug)]
pub struct TransactionFactory {
    max_gas_amount: u64,
    gas_unit_price: u64,
    transaction_expiration_time: u64,
    chain_id: ChainId,
}

impl TransactionFactory {
    pub fn new(chain_id: ChainId) -> Self {
        Self {
            // TODO(Gas): double check if this right
            max_gas_amount: MAX_GAS_AMOUNT,
            gas_unit_price: GAS_UNIT_PRICE,
            transaction_expiration_time: 30,
            chain_id,
        }
    }

    pub fn with_max_gas_amount(mut self, max_gas_amount: u64) -> Self {
        self.max_gas_amount = max_gas_amount;
        self
    }

    pub fn with_gas_unit_price(mut self, gas_unit_price: u64) -> Self {
        self.gas_unit_price = gas_unit_price;
        self
    }

    pub fn with_transaction_expiration_time(mut self, transaction_expiration_time: u64) -> Self {
        self.transaction_expiration_time = transaction_expiration_time;
        self
    }

    pub fn with_chain_id(mut self, chain_id: ChainId) -> Self {
        self.chain_id = chain_id;
        self
    }

    pub fn get_max_gas_amount(&self) -> u64 {
        self.max_gas_amount
    }

    pub fn get_gas_unit_price(&self) -> u64 {
        self.gas_unit_price
    }

    pub fn get_transaction_expiration_time(&self) -> u64 {
        self.transaction_expiration_time
    }

    pub fn get_chain_id(&self) -> ChainId {
        self.chain_id
    }

    pub fn payload(&self, payload: TransactionPayload) -> TransactionBuilder {
        self.transaction_builder(payload)
    }

    pub fn entry_function(&self, func: EntryFunction) -> TransactionBuilder {
        self.payload(TransactionPayload::EntryFunction(func))
    }

    pub fn transfer(&self, to: AccountAddress, amount: u64) -> TransactionBuilder {
        self.payload(aptos_stdlib::aptos_coin_transfer(to, amount))
    }

    pub fn account_transfer(&self, to: AccountAddress, amount: u64) -> TransactionBuilder {
        self.payload(
            aptos_cached_packages::aptos_framework_sdk_builder::aptos_account_transfer(to, amount),
        )
    }

    pub fn script(&self, script: Script) -> TransactionBuilder {
        self.payload(TransactionPayload::Script(script))
    }

    fn transaction_builder(&self, payload: TransactionPayload) -> TransactionBuilder {
        TransactionBuilder {
            sender: None,
            sequence_number: None,
            payload,
            max_gas_amount: self.max_gas_amount,
            gas_unit_price: self.gas_unit_price,
            expiration_timestamp_secs: self.expiration_timestamp(),
            chain_id: self.chain_id,
        }
    }

    fn expiration_timestamp(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + self.transaction_expiration_time
    }
}

pub async fn get_signed_tx(
    from_account: &mut LocalAccount,
    req: TxReq,
    options: Option<TxOptions>,
) -> Result<SignedTransaction> {
    let (func_id, type_args, args) = match req {
        TxReq::CreateToken(req) => {
            let func_id = Identifier::new(CREATE_FUNGIBLE_ASSET)?;
            let type_args = vec![];
            let args = vec![
                bcs::to_bytes(&req.token_id)?,
                bcs::to_bytes(&req.name)?,
                bcs::to_bytes(&req.symbol)?,
                bcs::to_bytes(&req.decimals)?,
                bcs::to_bytes(&req.icon_uri)?,
                bcs::to_bytes(&req.max_supply)?,
                bcs::to_bytes(&req.project_uri)?,
            ];
            (func_id, type_args, args)
        }
        TxReq::UpdateMeta(req) => {
            let func_id = Identifier::new(UPDATE_META_FUNC)?;
            let type_args = vec![];
            let fa_obj = AccountAddress::from_str(&req.fa_obj)?;
            let args = vec![
                bcs::to_bytes(&fa_obj)?,
                bcs::to_bytes(&req.name)?,
                bcs::to_bytes(&req.symbol)?,
                bcs::to_bytes(&req.decimals)?,
                bcs::to_bytes(&req.icon_uri)?,
                bcs::to_bytes(&req.project_uri)?,
            ];
            (func_id, type_args, args)
        }
        TxReq::MintToken(req) => {
            let func_id = Identifier::new(MINT_WITH_TICKET_FUNC)?;
            let type_args = vec![];
            let fa_obj = AccountAddress::from_str(&req.fa_obj)?;
            let recipient = AccountAddress::from_str(&req.recipient)?;
            let args = vec![
                bcs::to_bytes(&req.ticket_id)?,
                bcs::to_bytes(&fa_obj)?,
                bcs::to_bytes(&recipient)?,
                bcs::to_bytes(&req.mint_acmount)?,
            ];
            (func_id, type_args, args)
        }
        TxReq::BurnToken(req) => {
            let func_id = Identifier::new(BURN_TOKEN_FUNC)?;
            let type_args = vec![];
            let fa_obj = AccountAddress::from_str(&req.fa_obj)?;
            let args = vec![
                bcs::to_bytes(&fa_obj)?,
                bcs::to_bytes(&req.burn_acmount)?,
                bcs::to_bytes(&req.memo)?,
            ];
            (func_id, type_args, args)
        }
        TxReq::CollectFee(fee_amount) => {
            let func_id = Identifier::new(COLLECT_FEE_FUNC)?;
            let type_args = vec![];
            let args = vec![bcs::to_bytes(&fee_amount)?];
            (func_id, type_args, args)
        }
        TxReq::RemoveTicket(ticket_id) => {
            let func_id = Identifier::new(REMOVE_TICKET_FUNC)?;
            let type_args = vec![];
            let args = vec![bcs::to_bytes(&ticket_id)?];
            (func_id, type_args, args)
        }
    };
    log!(
        DEBUG,
        "[tx_builder::get_signed_tx] func_id: {:?}, type_args:{:?}, args:{:?} ",
        func_id,
        type_args,
        args
    );

    let options = options.unwrap_or_default();
    let current_package =
        read_config(|c| c.get().current_port_package.to_owned()).expect("port package is none!");
    let port_address = AccountAddress::from_str(&current_package)?;
    let port_info =
        read_state(|s| s.aptos_ports.get(&current_package)).expect("port info is none!");
    let module_id = Identifier::new(port_info.module)?;

    // get current timestamp and conver to second
    let now_s = api::time() / 1_000_000_000;

    let seq_num = from_account.update_seq_from_chain().await?;
    log!(
        DEBUG,
        "[tx_builder::get_signed_tx] latest seq number: {} ",
        seq_num
    );
    let transaction_builder = TransactionBuilder::new(
        TransactionPayload::EntryFunction(EntryFunction::new(
            ModuleId::new(port_address, module_id),
            func_id,
            type_args,
            args,
        )),
        now_s + options.timeout_secs,
        ChainId::new(options.chain_id),
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
