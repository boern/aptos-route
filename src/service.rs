#![allow(unused)]

use crate::aptos_client::aptos_providers::Provider;
use crate::aptos_client::constants::DEVNET_CHAIN_ID;
use crate::aptos_client::rest_client::{RestClient, RpcResult};

use crate::aptos_client::{
    rest_client, tx_builder, Account, AccountKey, AptosResult, CreateTokenReq, LocalAccount,
    ReqType, TxOptions, TxStatus,
};
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
    mutate_config, read_config, MultiRpcConfig, RouteConfig, Seqs, SnorKeyType, NATIVE_KEY_TYPE,
};
use crate::state::{replace_state, AptosPort, AptosToken, RouteState, TokenResp, UpdateType};
use crate::types::{TicketId, Token, TokenId};
// use crate::service::mint_token::MintTokenRequest;

use crate::state::{mutate_state, read_state};
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
            mutate_state(|s| s.seeds.insert(NATIVE_KEY_TYPE.to_string(), seed));
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
                    .get(&NATIVE_KEY_TYPE.to_string())
                    .unwrap_or_else(|| {
                        panic!("No key with name {:?}", &NATIVE_KEY_TYPE.to_string())
                    })
            });
            KeyType::Native(seed.to_vec())
        }
    };
    // let address = ck_eddsa::aptos_route_address(key_type).await?;
    let account_key = AccountKey::account_key(key_type)
        .await
        .map_err(|e| e.to_string())?;
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

// devops method
#[update(guard = "is_admin")]
pub async fn update_tx_option(tx_opt: TxOptions) {
    mutate_config(|s| {
        let mut config = s.get().to_owned();
        config.tx_opt = tx_opt;
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
    read_state(|s| {
        s.tokens
            .iter()
            .filter(|(token_id, token)| {
                s.atptos_tokens.contains_key(&token_id.to_string())
                    && matches!(
                        s.atptos_tokens.get(&token_id.to_string()).unwrap().status,
                        TxStatus::Finalized
                    )
            })
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

// devops method
#[update(guard = "is_admin")]
pub async fn update_port_package(package: String) {
    mutate_config(|s| {
        let mut config = s.get().to_owned();
        config.current_port_package = Some(package);
        s.set(config);
    })
}

#[query]
pub async fn aptos_ports() -> Vec<AptosPort> {
    read_state(|s| s.aptos_ports.iter().map(|(_, port)| port).collect())
}

// devops method
// after deploy or upgrade the sui port contract, call this interface to update sui token info
#[update(guard = "is_admin")]
pub async fn add_aptos_port(port: AptosPort) {
    mutate_state(|s| s.aptos_ports.insert(port.package.to_owned(), port));
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

#[query]
pub async fn aptos_token(token_id: TokenId) -> Option<AptosToken> {
    read_state(|s| s.atptos_tokens.get(&token_id))
}

// devops method
// after deploy or upgrade the sui port contract, call this interface to update sui token info
#[update(guard = "is_admin")]
pub async fn update_aptos_token(token_id: TokenId, aptos_token: AptosToken) -> Result<(), String> {
    mutate_state(|s| {
        s.atptos_tokens.insert(token_id.to_string(), aptos_token);
    });

    Ok(())
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
pub async fn get_account(address: String, ledger_version: Option<u64>) -> Result<String, String> {
    let client = RestClient::client();

    let ret = client
        .get_account(address, ledger_version, &client.forward)
        .await;
    log!(DEBUG, "[service::get_account] get_account ret: {:?}", ret);
    match ret {
        Ok(account) => {
            let account_json = serde_json::to_string(&account).map_err(|e| e.to_string())?;
            Ok(account_json)
        }
        Err(e) => {
            log!(DEBUG, "[service::get_account] get_account error : {:?}", e);
            Err(format!("Error getting account: {:?}", e))
        }
    }
}

#[update(guard = "is_admin")]
pub async fn get_account_balance(
    address: String,
    asset_type: Option<String>,
) -> Result<u64, String> {
    let client = RestClient::client();

    let ret = client
        .get_account_balance(address, asset_type, &client.forward)
        .await;
    log!(
        DEBUG,
        "[service::get_account_balance] get_account_balance ret: {:?}",
        ret
    );
    match ret {
        Ok(balance) => Ok(balance),
        Err(e) => {
            log!(
                DEBUG,
                "[service::get_account_balance] get_account_balance error : {:?}",
                e
            );
            Err(format!("Error getting account balance: {:?}", e))
        }
    }
}

#[update(guard = "is_admin")]
pub async fn get_fa_obj_from_port(
    view_func: String,
    token_id: String,
) -> Result<Vec<String>, String> {
    let client = RestClient::client();

    let ret = client
        .get_fa_obj(view_func, token_id, &client.forward)
        .await;
    log!(
        DEBUG,
        "[service::get_fa_obj_from_port] get_fa_obj ret: {:?}",
        ret
    );
    match ret {
        Ok(fa_obj) => Ok(fa_obj),
        Err(e) => {
            log!(
                DEBUG,
                "[service::get_fa_obj_from_port] get_fa_obj_from_port error : {:?}",
                e
            );
            Err(format!("Error get fa obj from port: {:?}", e))
        }
    }
}

//just for test and devops
#[update(guard = "is_admin")]
pub async fn submit_tx(req: ReqType) -> Result<String, String> {
    log!(DEBUG, "[service::submit_tx] TxReq: {:?} ", req);

    let mut local_account = LocalAccount::local_account()
        .await
        .map_err(|e| e.to_string())?;

    log!(
        DEBUG,
        "[service::submit_tx] local_account: {:?} ",
        local_account
    );

    let signed_txn = tx_builder::get_signed_tx(&mut local_account, &req, None)
        .await
        .map_err(|e| e.to_string())?;
    log!(
        DEBUG,
        "[service::submit_tx] SignedTransaction: {:#?} ",
        signed_txn
    );

    //verify tx sinature
    match signed_txn.verify_signature() {
        Ok(_) => ..,
        Err(e) => return Err(e.to_string()),
    };
    // transfer the aptos coin to a different address
    let client = RestClient::client();
    let ret = client.summit_tx(&signed_txn, &client.forward).await;
    log!(DEBUG, "[service::submit_tx] result: {:#?} ", ret);

    match ret {
        Ok(pending_tx) => {
            let pending_tx_json = serde_json::to_string(&pending_tx).map_err(|e| e.to_string())?;
            Ok(pending_tx_json)
        }
        Err(e) => {
            log!(DEBUG, "[service::submit_tx] ret error : {:?}", e);
            Err(format!("Error submit_tx: {:?}", e))
        }
    }
}

// devops
#[update(guard = "is_admin")]
pub async fn get_transaction_by_hash(txn_hash: String) -> Result<String, String> {
    let client = RestClient::client();

    let ret = client
        .get_transaction_by_hash(txn_hash, &client.forward)
        .await;
    log!(
        DEBUG,
        "[service::get_transaction_by_hash] get_transaction_by_hash ret: {:?}",
        ret
    );
    match ret {
        Ok(account) => {
            let account_json = serde_json::to_string(&account).map_err(|e| e.to_string())?;
            Ok(account_json)
        }
        Err(e) => {
            log!(
                DEBUG,
                "[service::get_transaction_by_hash] get_transaction_by_hash error : {:?}",
                e
            );
            Err(format!("Error get transaction by hash: {:?}", e))
        }
    }
}

#[query(hidden = true)]
fn cleanup_response(mut args: TransformArgs) -> TransformedHttpResponse {
    args.response.headers.clear();
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
