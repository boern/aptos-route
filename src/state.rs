#![allow(unused)]
use crate::aptos_client::{CreateTokenReq, ReqType, TxReq, TxStatus, UpdateMetaReq};
use crate::ck_eddsa::{hash_with_sha256, KeyType};
use crate::config::{mutate_config, read_config, RouteConfig};

use crate::handler::gen_ticket::GenerateTicketReq;
// use crate::handler::mint_token::MintTokenRequest;
use crate::ic_log::DEBUG;
// use crate::handler::burn_token::BurnTx;
// use crate::handler::clear_ticket::ClearTx;
// use crate::handler::gen_ticket::GenerateTicketReq;
use crate::lifecycle::InitArgs;
use crate::memory::Memory;
use candid::{CandidType, Principal};
use ic_canister_log::log;
use ic_stable_structures::StableBTreeMap;
use ic_stable_structures::StableCell;

// use crate::handler::mint_token::MintTokenRequest;
use crate::types::{Chain, ChainId, Ticket, TicketId, ToggleState, Token, TokenId};
use ic_stable_structures::storable::Bound;
use ic_stable_structures::Storable;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::default;
use std::{cell::RefCell, collections::HashSet};

pub type CanisterId = Principal;
pub type Owner = String;
pub type MintAccount = String;
pub type AssociatedAccount = String;

thread_local! {

    static STATE: RefCell<Option<RouteState>> = RefCell::default();
}

#[derive(CandidType, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountType {
    MintAccount,
    AssociatedAccount,
}

#[derive(CandidType, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountStatus {
    Confirmed,
    Unknown,
}

#[derive(
    CandidType, Clone, Debug, Serialize, Deserialize, PartialOrd, Ord, PartialEq, Eq, Hash,
)]
pub enum UpdateType {
    Name(String),
    Symbol(String),
    Icon(String),
    Description(String),
}

impl Storable for UpdateType {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        let bytes = bincode::serialize(&self).expect("failed to serialize UpdateType");
        Cow::Owned(bytes)
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        bincode::deserialize(bytes.as_ref()).expect("failed to deserialize UpdateType")
    }

    const BOUND: Bound = Bound::Unbounded;
}

#[derive(CandidType, Clone, Debug, Serialize, Deserialize)]
pub struct UpdateTokenStatus {
    pub token_id: String,
    pub req: UpdateMetaReq,
    pub retry: u64,
    pub tx_hash: Option<String>,
    pub status: TxStatus,
}

impl Storable for UpdateTokenStatus {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        let bytes = bincode::serialize(&self).expect("failed to serialize UpdateTokenStatus");
        Cow::Owned(bytes)
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        bincode::deserialize(bytes.as_ref()).expect("failed to deserialize UpdateTokenStatus")
    }

    const BOUND: Bound = Bound::Unbounded;
}

impl UpdateTokenStatus {
    pub fn new(token_id: TokenId, req: UpdateMetaReq) -> Self {
        Self {
            token_id,
            req,
            retry: 0,
            tx_hash: None,
            status: TxStatus::New,
        }
    }
}

#[derive(CandidType, Clone, Debug, Serialize, Deserialize)]
pub struct TokenResp {
    pub token_id: TokenId,
    pub symbol: String,
    pub decimals: u8,
    pub icon: Option<String>,
    pub rune_id: Option<String>,
}

impl From<Token> for TokenResp {
    fn from(value: Token) -> Self {
        TokenResp {
            token_id: value.token_id,
            symbol: value.symbol,
            decimals: value.decimals,
            icon: value.icon,
            rune_id: value.metadata.get("rune_id").map(|rune_id| rune_id.clone()),
        }
    }
}

#[derive(CandidType, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct AptosPort {
    pub package: String,
    pub module: String,
    pub functions: HashSet<String>,
    pub port_owner: String,
    pub aptos_route: String,
    pub fee_addr: String,
}

impl Storable for AptosPort {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        let bytes = bincode::serialize(&self).expect("failed to serialize AptosPort");
        Cow::Owned(bytes)
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        bincode::deserialize(bytes.as_ref()).expect("failed to deserialize AptosPort")
    }

    const BOUND: Bound = Bound::Unbounded;
}

#[derive(CandidType, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AptosToken {
    pub fa_obj_id: Option<String>,
    pub type_tag: Option<String>,
    // pub retry: u64,
    // pub tx_hash: Option<String>,
    // pub status: TxStatus,
}

impl Default for AptosToken {
    fn default() -> Self {
        Self {
            fa_obj_id: None,
            type_tag: None,
            // retry: 0,
            // tx_hash: None,
            // status: TxStatus::New,
        }
    }
}

impl Storable for AptosToken {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        let bytes = bincode::serialize(&self).expect("failed to serialize SuiTokenInfo");
        Cow::Owned(bytes)
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        bincode::deserialize(bytes.as_ref()).expect("failed to deserialize SuiTokenInfo")
    }

    const BOUND: Bound = Bound::Unbounded;
}

#[derive(Deserialize, Serialize)]
pub struct RouteState {
    // stable storage
    #[serde(skip, default = "crate::memory::init_ticket_queue")]
    pub tickets_queue: StableBTreeMap<u64, Ticket, Memory>,
    #[serde(skip, default = "crate::memory::init_failed_tickets")]
    pub tickets_failed_to_hub: StableBTreeMap<String, Ticket, Memory>,
    #[serde(skip, default = "crate::memory::init_counterparties")]
    pub counterparties: StableBTreeMap<ChainId, Chain, Memory>,
    #[serde(skip, default = "crate::memory::init_tokens")]
    pub tokens: StableBTreeMap<TokenId, Token, Memory>,

    #[serde(skip, default = "crate::memory::init_update_tokens")]
    pub update_token_queue: StableBTreeMap<TicketId, UpdateTokenStatus, Memory>,
    // #[serde(skip, default = "crate::memory::init_mint_token_requests")]
    // pub mint_token_requests: StableBTreeMap<TicketId, MintTokenRequest, Memory>,
    #[serde(skip, default = "crate::memory::init_gen_ticket_reqs")]
    pub gen_ticket_reqs: StableBTreeMap<TicketId, GenerateTicketReq, Memory>,
    #[serde(skip, default = "crate::memory::init_seed")]
    pub seeds: StableBTreeMap<String, [u8; 64], Memory>,
    #[serde(skip, default = "crate::memory::init_route_addresses")]
    pub route_addresses: StableBTreeMap<KeyType, Vec<u8>, Memory>,
    #[serde(skip, default = "crate::memory::init_aptos_ports")]
    pub aptos_ports: StableBTreeMap<String, AptosPort, Memory>,
    #[serde(skip, default = "crate::memory::init_aptos_tokens")]
    pub aptos_tokens: StableBTreeMap<TokenId, AptosToken, Memory>,

    //TODO: refactor tx queue key as hash_with_sha256
    #[serde(skip, default = "crate::memory::init_tx_queue")]
    pub tx_queue: StableBTreeMap<String, TxReq, Memory>,
}

impl RouteState {
    pub fn init() -> Self {
        Self {
            tickets_queue: StableBTreeMap::init(crate::memory::get_ticket_queue_memory()),
            tickets_failed_to_hub: StableBTreeMap::init(crate::memory::get_failed_tickets_memory()),
            counterparties: StableBTreeMap::init(crate::memory::get_counterparties_memory()),
            tokens: StableBTreeMap::init(crate::memory::get_tokens_memory()),

            update_token_queue: StableBTreeMap::init(crate::memory::get_update_tokens_memory()),
            // mint_token_requests: StableBTreeMap::init(
            //     crate::memory::get_mint_token_requests_memory(),
            // ),
            gen_ticket_reqs: StableBTreeMap::init(crate::memory::get_gen_ticket_req_memory()),
            seeds: StableBTreeMap::init(crate::memory::get_seeds_memory()),
            route_addresses: StableBTreeMap::init(crate::memory::get_route_addresses_memory()),
            aptos_ports: StableBTreeMap::init(crate::memory::get_aptos_ports_memory()),
            aptos_tokens: StableBTreeMap::init(crate::memory::get_aptos_tokens_memory()),
            tx_queue: StableBTreeMap::init(crate::memory::get_tx_queue_memory()),
        }
    }
    pub fn add_chain(&mut self, chain: Chain) {
        self.counterparties
            .insert(chain.chain_id.to_owned(), chain.to_owned());
    }

    pub fn add_token(&mut self, token: Token) {
        self.tokens
            .insert(token.token_id.to_owned(), token.to_owned());
        if self.aptos_tokens.get(&token.token_id).is_none() {
            let aptos_token = AptosToken::default();
            self.aptos_tokens
                .insert(token.token_id.to_owned(), aptos_token.to_owned());
            let create_token_req = CreateTokenReq {
                token_id: token.token_id.to_owned(),
                name: token.name.to_owned(),
                symbol: token.symbol.to_owned(),
                decimals: token.decimals.to_owned(),
                icon_uri: token.icon.to_owned().unwrap_or_default(),
                max_supply: None,
                project_uri: token
                    .metadata
                    .get("project_uri")
                    .unwrap_or(&"https://www.omnity.network".to_string())
                    .to_owned(),
            };
            let req_id = hash_with_sha256(
                &bincode::serialize(&create_token_req)
                    .expect("failed to serialize create_token_req "),
            );
            let tx_req = TxReq {
                req_type: ReqType::CreateToken(create_token_req.to_owned()),
                tx_hash: None,
                tx_status: TxStatus::New,
                retry: 0,
            };

            self.tx_queue.insert(req_id, tx_req);
        }
    }

    pub fn update_token(&mut self, update_token: Token) {
        self.tokens
            .insert(update_token.token_id.to_owned(), update_token.to_owned());
        if let Some(current_token) = self.tokens.get(&update_token.token_id) {
            // only update name,symbol and icon
            if !current_token.name.eq(&update_token.name)
                || !current_token.symbol.eq(&update_token.symbol)
                || !current_token.icon.eq(&update_token.icon)
            {
                let aptos_token = read_state(|s| s.aptos_tokens.get(&update_token.token_id))
                    .expect("aptos token is None");
                let update_meta_req = UpdateMetaReq {
                    token_id: update_token.token_id.to_owned(),
                    fa_obj: aptos_token
                        .fa_obj_id
                        .expect("fungible asset object id is None"),
                    name: Some(update_token.name.to_owned()),
                    symbol: Some(update_token.symbol.to_owned()),
                    decimals: None,
                    icon_uri: update_token.icon.to_owned(),
                    project_uri: None,
                };
                let req_id = hash_with_sha256(
                    &bincode::serialize(&update_meta_req)
                        .expect("failed to serialize update_meta_req"),
                );
                let tx_req = TxReq {
                    req_type: ReqType::UpdateMeta(update_meta_req.to_owned()),
                    tx_hash: None,
                    tx_status: TxStatus::New,
                    retry: 0,
                };
                mutate_state(|s| s.tx_queue.insert(req_id, tx_req));
            }
        }
    }

    pub fn toggle_chain_state(&mut self, toggle: ToggleState) {
        let chain_id = read_config(|c| c.get().chain_id.to_owned());
        if toggle.chain_id == chain_id {
            mutate_config(|c| {
                let mut config = c.get().to_owned();
                config.chain_state = toggle.action.into();
                c.set(config);
            });
        } else if let Some(chain) = self.counterparties.get(&toggle.chain_id).as_mut() {
            chain.chain_state = toggle.action.into();
            // update chain state
            self.counterparties
                .insert(chain.chain_id.to_string(), chain.to_owned());
        }
    }
}

pub fn take_state<F, R>(f: F) -> R
where
    F: FnOnce(RouteState) -> R,
{
    STATE.with(|s| f(s.take().expect("State not initialized!")))
}

pub fn mutate_state<F, R>(f: F) -> R
where
    F: FnOnce(&mut RouteState) -> R,
{
    STATE.with(|s| f(s.borrow_mut().as_mut().expect("State not initialized!")))
}

pub fn read_state<F, R>(f: F) -> R
where
    F: FnOnce(&RouteState) -> R,
{
    STATE.with(|s| f(s.borrow().as_ref().expect("State not initialized!")))
}

pub fn replace_state(state: RouteState) {
    STATE.with(|s| {
        *s.borrow_mut() = Some(state);
    });
}
