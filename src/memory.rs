use ic_stable_structures::StableBTreeMap;
use ic_stable_structures::StableCell;
use ic_stable_structures::{
    memory_manager::{MemoryId, MemoryManager, VirtualMemory},
    DefaultMemoryImpl,
};
use std::cell::RefCell;

use crate::aptos_client::TxReq;
// use crate::aptos_client::TxStatus;
use crate::ck_eddsa::KeyType;
use crate::config::RouteConfig;

use crate::handler::gen_ticket::GenerateTicketReq;
// use crate::handler::mint_token::MintTokenRequest;
use crate::state::AptosPort;
use crate::state::{AptosToken, UpdateTokenStatus};
use crate::types::Ticket;
use crate::types::{Chain, Token};

const UPGRADES: MemoryId = MemoryId::new(0);
const CONFIG: MemoryId = MemoryId::new(1);
const TOKENS: MemoryId = MemoryId::new(2);
const APTOS_PORTS: MemoryId = MemoryId::new(3);
const APTOS_TOKENS: MemoryId = MemoryId::new(4);
const UPDATE_TOKENS: MemoryId = MemoryId::new(5);
const TICKET_QUEUE: MemoryId = MemoryId::new(6);
const FAILED_TICKETS: MemoryId = MemoryId::new(7);
const COUNTERPARTIES: MemoryId = MemoryId::new(8);
const MINT_TOKEN_REQUESTS: MemoryId = MemoryId::new(9);
const GEN_TICKET_REQS: MemoryId = MemoryId::new(10);
const SEEDS: MemoryId = MemoryId::new(11);
const ROUTE_ADDRESSES: MemoryId = MemoryId::new(12);
const TX_QUEUE: MemoryId = MemoryId::new(13);

type InnerMemory = DefaultMemoryImpl;

pub type Memory = VirtualMemory<InnerMemory>;

thread_local! {
    static MEMORY: RefCell<Option<InnerMemory>> = RefCell::new(Some(InnerMemory::default()));

    static MEMORY_MANAGER: RefCell<Option<MemoryManager<InnerMemory>>> =
        RefCell::new(Some(MemoryManager::init(MEMORY.with(|m| m.borrow().clone().unwrap()))));
}

fn with_memory_manager<R>(f: impl FnOnce(&MemoryManager<InnerMemory>) -> R) -> R {
    MEMORY_MANAGER.with(|cell| {
        f(cell
            .borrow()
            .as_ref()
            .expect("memory manager not initialized"))
    })
}

pub fn get_upgrades_memory() -> Memory {
    with_memory_manager(|m| m.get(UPGRADES))
}

pub fn get_ticket_queue_memory() -> Memory {
    with_memory_manager(|m| m.get(TICKET_QUEUE))
}

pub fn get_failed_tickets_memory() -> Memory {
    with_memory_manager(|m| m.get(FAILED_TICKETS))
}

pub fn get_counterparties_memory() -> Memory {
    with_memory_manager(|m| m.get(COUNTERPARTIES))
}

pub fn get_tokens_memory() -> Memory {
    with_memory_manager(|m| m.get(TOKENS))
}

pub fn get_update_tokens_memory() -> Memory {
    with_memory_manager(|m| m.get(UPDATE_TOKENS))
}

pub fn get_mint_token_requests_memory() -> Memory {
    with_memory_manager(|m| m.get(MINT_TOKEN_REQUESTS))
}

pub fn get_gen_ticket_req_memory() -> Memory {
    with_memory_manager(|m| m.get(GEN_TICKET_REQS))
}

pub fn get_seeds_memory() -> Memory {
    with_memory_manager(|m| m.get(SEEDS))
}

pub fn get_aptos_tokens_memory() -> Memory {
    with_memory_manager(|m| m.get(APTOS_TOKENS))
}

pub fn get_config_memory() -> Memory {
    with_memory_manager(|m| m.get(CONFIG))
}
pub fn get_route_addresses_memory() -> Memory {
    with_memory_manager(|m| m.get(ROUTE_ADDRESSES))
}

pub fn get_aptos_ports_memory() -> Memory {
    with_memory_manager(|m| m.get(APTOS_PORTS))
}

pub fn get_tx_queue_memory() -> Memory {
    with_memory_manager(|m| m.get(TX_QUEUE))
}

pub fn init_ticket_queue() -> StableBTreeMap<u64, Ticket, Memory> {
    StableBTreeMap::init(get_ticket_queue_memory())
}

pub fn init_failed_tickets() -> StableBTreeMap<String, Ticket, Memory> {
    StableBTreeMap::init(get_failed_tickets_memory())
}

pub fn init_counterparties() -> StableBTreeMap<String, Chain, Memory> {
    StableBTreeMap::init(get_counterparties_memory())
}

pub fn init_tokens() -> StableBTreeMap<String, Token, Memory> {
    StableBTreeMap::init(get_tokens_memory())
}

pub fn init_update_tokens() -> StableBTreeMap<String, UpdateTokenStatus, Memory> {
    StableBTreeMap::init(get_update_tokens_memory())
}

// pub fn init_mint_token_requests() -> StableBTreeMap<String, MintTokenRequest, Memory> {
//     StableBTreeMap::init(get_mint_token_requests_memory())
// }

pub fn init_gen_ticket_reqs() -> StableBTreeMap<String, GenerateTicketReq, Memory> {
    StableBTreeMap::init(get_gen_ticket_req_memory())
}

pub fn init_seed() -> StableBTreeMap<String, [u8; 64], Memory> {
    StableBTreeMap::init(get_seeds_memory())
}

pub fn init_aptos_tokens() -> StableBTreeMap<String, AptosToken, Memory> {
    StableBTreeMap::init(get_aptos_tokens_memory())
}

pub fn init_config() -> StableCell<RouteConfig, Memory> {
    StableCell::init(get_config_memory(), RouteConfig::default())
        .expect("failed to init sui route config")
}

pub fn init_route_addresses() -> StableBTreeMap<KeyType, Vec<u8>, Memory> {
    StableBTreeMap::init(get_route_addresses_memory())
}

pub fn init_aptos_ports() -> StableBTreeMap<String, AptosPort, Memory> {
    StableBTreeMap::init(get_aptos_ports_memory())
}

pub fn init_tx_queue() -> StableBTreeMap<String, TxReq, Memory> {
    StableBTreeMap::init(get_tx_queue_memory())
}
