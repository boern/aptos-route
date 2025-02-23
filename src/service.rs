#![allow(unused)]

use crate::aptos_client::aptos_providers::Provider;
use crate::aptos_client::constants::DEVNET_CHAIN_ID;
use crate::aptos_client::rpc_client::{RpcClient, RpcResult};
use crate::aptos_client::{rpc_client, transfer, AccountKey, LocalAccount};
use crate::auth::{is_admin, set_perms, Permission};
use crate::call_error::{CallError, Reason};
use crate::ck_eddsa::KeyType;
use crate::constants::SUI_COIN;
use crate::guard::TaskType;
use crate::ic_log::{DEBUG, ERROR};

use crate::memory::init_config;
use crate::{aptos_client, ck_eddsa};

use aptos_crypto::Signature;
use candid::Principal;
use ic_cdk::{init, post_upgrade, pre_upgrade, query, update};

use aptos_crypto::ed25519::Ed25519Signature;
use move_core_types::account_address::AccountAddress;
use serde_json::json;

// use crate::handler::gen_ticket::{
//     self, query_tx_from_multi_rpc, send_ticket, GenerateTicketError, GenerateTicketOk,
//     GenerateTicketReq,
// };
// use crate::handler::mint_token::{self};

// use crate::handler::scheduler;
use crate::lifecycle::{self, RouteArg, UpgradeArgs};

use crate::config::{
    mutate_config, read_config, AptosPortAction, MultiRpcConfig, RouteConfig, Seqs, SnorKeyType,
    KEY_TYPE_NAME,
};
use crate::state::{replace_state, AptosToken, RouteState, TokenResp, UpdateType};
use crate::types::{TicketId, Token, TokenId};
// use crate::service::mint_token::MintTokenRequest;

use crate::state::{mutate_state, read_state, TxStatus};
use crate::types::ChainState;
use crate::types::{Chain, ChainId, Ticket};
use ic_canister_log::log;

use crate::types::Factor;
use ic_canisters_http_types::{HttpRequest, HttpResponse, HttpResponseBuilder};
use ic_cdk::api::management_canister::http_request::{
    HttpResponse as TransformedHttpResponse, TransformArgs,
};

use std::str::FromStr;

use std::time::Duration;

async fn get_random_seed() -> [u8; 64] {
    match ic_cdk::api::management_canister::main::raw_rand().await {
        Ok(rand) => {
            let mut rand = rand.0;
            rand.extend(rand.clone());
            let rand: [u8; 64] = rand.try_into().expect("Expected a Vec of length 64");
            rand
        }
        Err(err) => {
            ic_cdk::trap(format!("Error getting random seed: {:?}", err).as_str());
        }
    }
}

#[init]
fn init(args: RouteArg) {
    log!(DEBUG, "init args: {:?}", args);
    match args {
        RouteArg::Init(args) => {
            lifecycle::init(args);
        }
        RouteArg::Upgrade(_) => {
            panic!("expected InitArgs got UpgradeArgs");
        }
    }
    // init seeds
    ic_cdk_timers::set_timer(Duration::ZERO, || {
        ic_cdk::spawn(async move {
            let seed = get_random_seed().await;
            mutate_state(|s| s.seeds.insert(KEY_TYPE_NAME.to_string(), seed));
        });
    });
}

#[pre_upgrade]
fn pre_upgrade() {
    log!(DEBUG, "begin to execute pre_upgrade ...");
    // scheduler::stop_schedule(None);
    lifecycle::pre_upgrade();
    log!(DEBUG, "pre_upgrade end!");
}

#[post_upgrade]
fn post_upgrade(args: Option<RouteArg>) {
    log!(DEBUG, "begin to execute post_upgrade with :{:?}", args);
    let mut upgrade_arg: Option<UpgradeArgs> = None;
    if let Some(route_arg) = args {
        upgrade_arg = match route_arg {
            RouteArg::Upgrade(upgrade_args) => upgrade_args,
            RouteArg::Init(_) => panic!("expected Option<UpgradeArgs> got InitArgs."),
        };
    }

    lifecycle::post_upgrade(upgrade_arg);
    // scheduler::start_schedule(None);
    log!(DEBUG, "upgrade successfully!");
}

// devops method
#[query(guard = "is_admin")]
pub async fn get_route_config() -> RouteConfig {
    read_config(|s| s.get().to_owned())
}
// devops method
#[update(guard = "is_admin", hidden = true)]
pub fn start_schedule(tasks: Option<Vec<TaskType>>) {
    log!(DEBUG, "start schedule task: {:?} ... ", tasks);
    // scheduler::start_schedule(tasks);
}

// devops method
#[update(guard = "is_admin", hidden = true)]
pub fn stop_schedule(tasks: Option<Vec<TaskType>>) {
    log!(DEBUG, "stop schedule task: {:?} ...", tasks);
    // scheduler::stop_schedule(tasks);
}

// devops method
#[query(guard = "is_admin", hidden = true)]
pub async fn active_tasks() -> Vec<TaskType> {
    read_config(|s| s.get().active_tasks.iter().map(|t| t.to_owned()).collect())
}

// devops method
#[update(guard = "is_admin", hidden = true)]
pub async fn update_schnorr_key(key_name: String) {
    mutate_config(|s| {
        let mut config = s.get().to_owned();
        config.schnorr_key_name = key_name;
        s.set(config);
    })
}

// devops method
#[query(guard = "is_admin")]
pub async fn forward() -> Option<String> {
    read_config(|s| s.get().forward.to_owned())
}

// devops method
#[update(guard = "is_admin", hidden = true)]
pub async fn update_forward(forward: Option<String>) {
    mutate_config(|s| {
        let mut config = s.get().to_owned();
        config.forward = forward;
        s.set(config);
    })
}

// devops method
#[update(guard = "is_admin", hidden = true)]
pub async fn query_key_type() -> SnorKeyType {
    read_config(|s| s.get().key_type.to_owned().into())
}

// devops method
#[update(guard = "is_admin", hidden = true)]
pub async fn update_key_type(key_type: SnorKeyType) {
    let key_type = match key_type {
        SnorKeyType::ChainKey => KeyType::ChainKey,
        SnorKeyType::Native => {
            let seed = get_random_seed().await;
            KeyType::Native(seed.to_vec())
        }
    };
    mutate_config(|s| {
        let mut config = s.get().to_owned();
        config.key_type = key_type;
        s.set(config);
    })
}

// devops method
#[update]
pub async fn aptos_route_address(key_type: SnorKeyType) -> Result<String, String> {
    let key_type = match key_type {
        SnorKeyType::ChainKey => KeyType::ChainKey,
        SnorKeyType::Native => {
            let seed = read_state(|s| {
                s.seeds
                    .get(&KEY_TYPE_NAME.to_string())
                    .unwrap_or_else(|| panic!("No key with name {:?}", &KEY_TYPE_NAME.to_string()))
            });
            KeyType::Native(seed.to_vec())
        }
    };
    // let address = ck_eddsa::aptos_route_address(key_type).await?;
    let account_key = AccountKey::account_key(key_type).await?;
    let account_address = account_key.authentication_key().account_address();
    let address = format!("{}", account_address);
    Ok(address)
}

// devops method
#[query(guard = "is_admin", hidden = true)]
pub async fn multi_rpc_config() -> MultiRpcConfig {
    read_config(|s| s.get().multi_rpc_config.to_owned())
}

// devops method
#[update(guard = "is_admin", hidden = true)]
pub async fn update_multi_rpc(multi_prc_cofig: MultiRpcConfig) {
    mutate_config(|s| {
        let mut config = s.get().to_owned();
        config.multi_rpc_config = multi_prc_cofig;
        s.set(config);
    })
}

// devops method
#[query(guard = "is_admin")]
pub async fn rpc_provider() -> Provider {
    read_config(|s| s.get().rpc_provider.to_owned())
}

// devops method
#[update(guard = "is_admin")]
pub async fn update_rpc_provider(provider: Provider) {
    mutate_config(|s| {
        let mut config = s.get().to_owned();
        config.rpc_provider = provider;
        s.set(config);
    })
}

// query supported chain list
#[query]
fn get_chain_list() -> Vec<Chain> {
    read_state(|s| {
        s.counterparties
            .iter()
            .filter(|(_, chain)| matches!(chain.chain_state, ChainState::Active))
            .map(|(_, chain)| chain.to_owned())
            .collect()
    })
}

// query supported chain list
#[query]
fn get_token_list() -> Vec<TokenResp> {
    //TODO: check sui token state
    read_state(|s| {
        s.tokens
            .iter()
            .map(|(_, token)| token.to_owned().into())
            .collect()
    })
}

// devops method
#[query(guard = "is_admin")]
fn get_token(token_id: TokenId) -> Option<Token> {
    read_state(|s| s.tokens.get(&token_id))
}

// devops method
#[update(guard = "is_admin")]
pub async fn get_gas_budget() -> u64 {
    read_config(|s| s.get().gas_budget)
}
// devops method
#[update(guard = "is_admin")]
pub async fn update_gas_budget(gas_budget: u64) {
    mutate_config(|s| {
        let mut config = s.get().to_owned();
        config.gas_budget = gas_budget;
        s.set(config);
    })
}

#[query]
pub async fn aptos_port_info() -> AptosPortAction {
    read_config(|s| s.get().sui_port_action.to_owned())
}

// devops method
// after deploy or upgrade the sui port contract, call this interface to update sui token info
#[update(guard = "is_admin")]
pub async fn update_aptos_port_info(action: AptosPortAction) {
    mutate_config(|s| {
        let mut config = s.get().to_owned();
        config.sui_port_action = action;
        s.set(config);
    })
}

#[query]
pub async fn aptos_token(token_id: TokenId) -> Option<AptosToken> {
    read_state(|s| s.sui_tokens.get(&token_id))
}

// devops method
// after deploy or upgrade the sui port contract, call this interface to update sui token info
#[update(guard = "is_admin")]
pub async fn update_aptos_token(token_id: TokenId, sui_token: AptosToken) -> Result<(), String> {
    mutate_state(|s| {
        s.sui_tokens.insert(token_id.to_string(), sui_token);
    });

    Ok(())
}

// devops method, add token manually
#[update(guard = "is_admin")]
pub async fn add_token(token: Token) -> Option<Token> {
    mutate_state(|s| {
        s.tokens
            .insert(token.token_id.to_string(), token.to_owned())
    })
}

// devops method
#[update(guard = "is_admin", hidden = true)]
fn update_token(token: Token) -> Result<Option<Token>, CallError> {
    mutate_state(|s| match s.tokens.get(&token.token_id) {
        None => Err(CallError {
            method: "[service::update_token] update_token".to_string(),
            reason: Reason::CanisterError(format!(
                "Not found token id {} ",
                token.token_id.to_string()
            )),
        }),
        Some(_) => Ok(s
            .tokens
            .insert(token.token_id.to_string(), token.to_owned())),
    })
    // Ok(())
}

// devops method
#[query(hidden = true)]
fn get_ticket_from_queue(ticket_id: String) -> Option<(u64, Ticket)> {
    read_state(|s| {
        s.tickets_queue
            .iter()
            .find(|(_seq, ticket)| ticket.ticket_id.eq(&ticket_id))
    })
}

// devops method
#[query(hidden = true)]
fn get_tickets_from_queue() -> Vec<(u64, Ticket)> {
    read_state(|s| {
        s.tickets_queue
            .iter()
            .map(|(seq, ticket)| (seq, ticket))
            .collect()
    })
}

// devops method
#[update(guard = "is_admin", hidden = true)]
pub async fn remove_ticket_from_quene(ticket_id: String) -> Option<Ticket> {
    mutate_state(|s| {
        let ticket = s
            .tickets_queue
            .iter()
            .find(|(_seq, ticket)| ticket.ticket_id.eq(&ticket_id));

        match ticket {
            None => None,
            Some((seq, _ticket)) => s.tickets_queue.remove(&seq),
        }
    })
}

// query collect fee account
#[query]
pub async fn get_fee_account() -> String {
    read_config(|s| s.get().fee_account.to_string())
}

// update collect fee account
#[update(guard = "is_admin", hidden = true)]
pub async fn update_fee_account(fee_account: String) {
    mutate_config(|s| {
        let mut config = s.get().to_owned();
        config.fee_account = fee_account;
        s.set(config);
    })
}

// query fee account for the dst chain
#[query]
pub fn get_redeem_fee(chain_id: ChainId) -> Option<u128> {
    read_config(|s| s.get().get_fee(chain_id))
}

#[update(guard = "is_admin", hidden = true)]
pub async fn update_redeem_fee(fee: Factor) {
    mutate_config(|s| {
        let mut config = s.get().to_owned();
        config.update_fee(fee);
        s.set(config);
    })
}

// devops method
#[query(guard = "is_admin", hidden = true)]
pub fn get_failed_tickets_to_hub() -> Vec<Ticket> {
    read_state(|s| {
        s.tickets_failed_to_hub
            .iter()
            .map(|(_, ticket)| ticket)
            .collect()
    })
}

// devops method
#[query(guard = "is_admin", hidden = true)]
pub fn get_failed_ticket_to_hub(ticket_id: String) -> Option<Ticket> {
    read_state(|s| s.tickets_failed_to_hub.get(&ticket_id))
}

// devops method
#[update(guard = "is_admin", hidden = true)]
pub async fn remove_failed_tickets_to_hub(ticket_id: String) -> Option<Ticket> {
    mutate_state(|state| state.tickets_failed_to_hub.remove(&ticket_id))
}

// devops method
#[query(guard = "is_admin", hidden = true)]
pub async fn seqs() -> Seqs {
    read_config(|s| s.get().seqs.to_owned())
}

// devops method
#[update(guard = "is_admin", hidden = true)]
pub async fn update_seqs(seqs: Seqs) {
    mutate_config(|s| {
        let mut config = s.get().to_owned();
        config.seqs = seqs;
        s.set(config);
    })
}

// devops method
#[update(guard = "is_admin", hidden = true)]
pub async fn set_permissions(caller: Principal, perm: Permission) {
    set_perms(caller.to_string(), perm)
}

// devops method
#[update(guard = "is_admin", hidden = true)]
pub fn debug(enable: bool) {
    mutate_config(|s| {
        let mut config = s.get().to_owned();
        config.enable_debug = enable;
        s.set(config);
    });
}

#[update(guard = "is_admin")]
pub async fn get_account(address: String, ledger_version: Option<u64>) -> RpcResult<String> {
    let (provider, nodes, forward) = read_config(|s| {
        (
            s.get().rpc_provider.to_owned(),
            s.get().nodes_in_subnet,
            s.get().forward.to_owned(),
        )
    });
    let client = RpcClient::new(provider, Some(nodes));

    client.get_account(address, ledger_version, forward).await
}

#[update(guard = "is_admin")]
pub async fn verfy_txn(recipient: String, amount: u64, key_type: SnorKeyType) -> RpcResult<bool> {
    log!(
        DEBUG,
        "[service::verfy_txn] recipient: {}, amount: {}, keyt_type: {:?}",
        recipient,
        amount,
        key_type
    );
    let (provider, nodes, forward) = read_config(|s| {
        (
            s.get().rpc_provider.to_owned(),
            s.get().nodes_in_subnet,
            s.get().forward.to_owned(),
        )
    });
    let key_type = match key_type {
        SnorKeyType::ChainKey => KeyType::ChainKey,
        SnorKeyType::Native => {
            let seed = read_state(|s| {
                s.seeds
                    .get(&KEY_TYPE_NAME.to_string())
                    .unwrap_or_else(|| panic!("No key with name {:?}", &KEY_TYPE_NAME.to_string()))
            });
            KeyType::Native(seed.to_vec())
        }
    };
    let mut local_account = LocalAccount::local_account(key_type).await;
    log!(
        DEBUG,
        "[service::verfy_txn] local_account: {:?} ",
        local_account
    );
    let to_account = AccountAddress::from_str(&recipient).unwrap();
    log!(DEBUG, "[service::verfy_txn] to_account: {:?} ", to_account);
    // devnet chain id is 174
    // let chain_id = 174;
    let txn = transfer::get_signed_transfer_txn(
        DEVNET_CHAIN_ID,
        &mut local_account,
        to_account,
        amount,
        None,
    )
    .await
    .unwrap();
    log!(DEBUG, "[service::verfy_txn] SignedTransaction: {:#?} ", txn);
    match txn.verify_signature() {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

#[update(guard = "is_admin")]
pub async fn transfer_aptos_from_route(
    recipient: String,
    amount: u64,
    key_type: SnorKeyType,
) -> RpcResult<String> {
    let (provider, nodes, forward) = read_config(|s| {
        (
            s.get().rpc_provider.to_owned(),
            s.get().nodes_in_subnet,
            s.get().forward.to_owned(),
        )
    });

    let key_type = match key_type {
        SnorKeyType::ChainKey => KeyType::ChainKey,
        SnorKeyType::Native => {
            let seed = read_state(|s| {
                s.seeds
                    .get(&KEY_TYPE_NAME.to_string())
                    .unwrap_or_else(|| panic!("No key with name {:?}", &KEY_TYPE_NAME.to_string()))
            });
            KeyType::Native(seed.to_vec())
        }
    };
    let mut local_account = LocalAccount::local_account(key_type).await;
    log!(
        DEBUG,
        "[service::transfer_aptos] local_account: {:?} ",
        local_account
    );
    let to_account = AccountAddress::from_str(&recipient).unwrap();
    log!(
        DEBUG,
        "[service::transfer_aptos] to_account: {:?} ",
        to_account
    );
    // devnet chain id is 174
    // let chain_id = 174;
    let txn = transfer::get_signed_transfer_txn(
        DEVNET_CHAIN_ID,
        &mut local_account,
        to_account,
        amount,
        None,
    )
    .await
    .unwrap();
    log!(
        DEBUG,
        "[service::transfer_aptos] SignedTransaction: {:#?} ",
        txn
    );
    // transfer the aptos coin to a different address
    let client = RpcClient::new(provider, Some(nodes));
    let ret = client.transfer_aptos(&txn, forward).await?;
    log!(DEBUG, "[service::transfer_aptos] result: {:#?} ", ret);
    //increase sequence number
    // let latest_tx_seq = local_account.increment_sequence_number();
    // let latest_tx_seq = local_account.sequence_number();
    // mutate_config(|s| {
    //     let mut config = s.get().to_owned();
    //     config.seqs.tx_seq = latest_tx_seq;
    //     s.set(config);
    // });

    Ok(ret)
}

/// Cleans up the HTTP response headers to make them deterministic.
///
/// # Arguments
///
/// * `args` - Transformation arguments containing the HTTP response.
///
#[query(hidden = true)]
fn cleanup_response(mut args: TransformArgs) -> TransformedHttpResponse {
    // The response header contains non-deterministic fields that make it impossible to reach consensus!
    // Errors seem deterministic and do not contain data that can break consensus.
    // Clear non-deterministic fields from the response headers.

    // log!(
    //     DEBUG,
    //     "[service::cleanup_response] cleanup_response TransformArgs: {:?}",
    //     args
    // );
    args.response.headers.clear();
    // log!(
    //     DEBUG,
    //     "[service::cleanup_response] response.headers: {:?}",
    //     args.response.headers
    // );
    args.response
}

#[query(hidden = true)]
fn http_request(req: HttpRequest) -> HttpResponse {
    match req.path() {
        "/logs" => {
            let endable_debug = read_config(|s| s.get().enable_debug);
            crate::ic_log::http_log(req, endable_debug)
        }

        _ => HttpResponseBuilder::not_found().build(),
    }
}

// Enable Candid export
ic_cdk::export_candid!();
