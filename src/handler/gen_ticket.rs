use crate::aptos_client::rest_client::RestClient;
use crate::aptos_client::AptosResult;
use crate::config::read_config;
// use crate::state::AptosPort;
use crate::types::{ChainState, Error, TicketType, TxAction};
use crate::types::{Memo, Ticket};
use aptos_api_types::move_types::MoveType;
use aptos_api_types::transaction::Transaction;
use aptos_api_types::HashValue;
use aptos_types::account_address::AccountAddress;
use candid::{CandidType, Principal};
use omnity_types::address;

use crate::ic_log::{DEBUG, WARNING};

use crate::{
    call_error::{CallError, Reason},
    state::{mutate_state, read_state},
};
use ic_stable_structures::storable::Bound;
use ic_stable_structures::Storable;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use serde_with::DisplayFromStr;
use std::borrow::Cow;
use std::str::FromStr;

use ic_canister_log::log;

#[derive(CandidType, Clone, Debug, Deserialize, PartialEq, Eq)]
pub enum GenerateTicketError {
    TemporarilyUnavailable(String),
    UnsupportedToken(String),
    UnsupportedChainId(String),
    /// The redeem account does not hold the requested token amount.
    InsufficientFunds {
        balance: u64,
    },
    /// The caller didn't approve enough funds for spending.
    InsufficientAllowance {
        allowance: u64,
    },
    SendTicketErr(String),
    InsufficientRedeemFee {
        required: u64,
        provided: u64,
    },
    RedeemFeeNotSet,
    TransferFailure(String),
    UnsupportedAction(String),
}

#[derive(CandidType, Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct GenerateTicketReq {
    pub tx_hash: String,
    pub target_chain_id: String,
    pub sender: String,
    pub receiver: String,
    pub token_id: String,
    pub amount: u64,
    pub action: TxAction,
    pub memo: Option<String>,
}

impl Storable for GenerateTicketReq {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        let bytes = bincode::serialize(&self).expect("failed to serialize GenerateTicketReq");
        Cow::Owned(bytes)
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        bincode::deserialize(bytes.as_ref()).expect("failed to deserialize GenerateTicketReq")
    }

    const BOUND: Bound = Bound::Unbounded;
}

#[derive(CandidType, Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct GenerateTicketOk {
    pub ticket_id: String,
}

pub async fn generate_ticket(
    req: GenerateTicketReq,
) -> Result<GenerateTicketOk, GenerateTicketError> {
    log!(DEBUG, "[generate_ticket] generate_ticket req: {:#?}", req);
    validate_req(&req)?;

    let (hub_principal, chain_id) =
        read_config(|s| (s.get().hub_principal, s.get().chain_id.to_owned()));

    if !verify_tx(req.to_owned()).await? {
        return Err(GenerateTicketError::TemporarilyUnavailable(format!(
            "[generate_ticket] Unable to verify the tx ({}) ",
            req.tx_hash,
        )));
    }
    let fee = read_config(|s| s.get().get_fee(req.target_chain_id.to_owned())).unwrap_or_default();
    let memo = Memo {
        memo: req.memo,
        bridge_fee: fee,
    };
    // let memo = bridge_fee.add_to_memo(req.memo).unwrap_or_default();
    let memo_json = serde_json::to_string_pretty(&memo).map_err(|e| {
        GenerateTicketError::TemporarilyUnavailable(format!(
            "[generate_ticket] memo convert error: {}",
            e.to_string()
        ))
    })?;
    log!(DEBUG, "[generate_ticket] memo with fee: {:?}", memo_json);

    let ticket = Ticket {
        ticket_id: req.tx_hash.to_string(),
        ticket_type: TicketType::Normal,
        ticket_time: ic_cdk::api::time(),
        src_chain: chain_id,
        dst_chain: req.target_chain_id.to_owned(),
        action: req.action.to_owned(),
        token: req.token_id.to_owned(),
        amount: req.amount.to_string(),
        sender: Some(req.sender.to_owned()),
        receiver: req.receiver.to_string(),
        memo: Some(memo_json.to_bytes().to_vec()),
    };

    match send_ticket(hub_principal, ticket.to_owned()).await {
        Err(err) => {
            mutate_state(|s| {
                s.tickets_failed_to_hub
                    .insert(ticket.ticket_id.to_string(), ticket.to_owned());
            });
            log!(
                WARNING,
                "[generate_ticket] failed to send ticket: {}",
                req.tx_hash.to_string()
            );
            Err(GenerateTicketError::SendTicketErr(format!("{}", err)))
        }
        Ok(()) => {
            log!(
                DEBUG,
                "[generate_ticket] successful to send ticket: {:?}",
                ticket
            );

            mutate_state(|s| s.gen_ticket_reqs.remove(&req.tx_hash.to_owned()));
            Ok(GenerateTicketOk {
                ticket_id: req.tx_hash.to_string(),
            })
        }
    }
}

pub fn validate_req(req: &GenerateTicketReq) -> Result<(), GenerateTicketError> {
    HashValue::from_str(&req.tx_hash)
        .map_err(|e| GenerateTicketError::TemporarilyUnavailable(e.to_string()))?;
    AccountAddress::from_str(&req.sender)
        .map_err(|e| GenerateTicketError::TemporarilyUnavailable(e.to_string()))?;

    if read_config(|s| s.get().chain_state == ChainState::Deactive) {
        return Err(GenerateTicketError::TemporarilyUnavailable(
            "chain state is deactive!".into(),
        ));
    }

    if !read_state(|s| {
        s.counterparties
            .get(&req.target_chain_id)
            .is_some_and(|c| c.chain_state == ChainState::Active)
    }) {
        return Err(GenerateTicketError::UnsupportedChainId(
            req.target_chain_id.to_owned(),
        ));
    }
    {
        address::validate_account(&req.target_chain_id, &req.receiver)
            .map_err(|e| GenerateTicketError::TemporarilyUnavailable(e.to_string()))?;
    }

    if req.amount <= 0 {
        return Err(GenerateTicketError::TemporarilyUnavailable(
            "amount must be > 0".into(),
        ));
    }

    if !read_state(|s| s.tokens.contains_key(&req.token_id.to_string())) {
        return Err(GenerateTicketError::UnsupportedToken(
            req.token_id.to_owned(),
        ));
    }

    if read_state(|s| s.gen_ticket_reqs.contains_key(&req.tx_hash.to_owned())) {
        return Err(GenerateTicketError::TemporarilyUnavailable(
            "duplicate request!".into(),
        ));
    }
    mutate_state(|s| {
        s.gen_ticket_reqs
            .insert(req.tx_hash.to_owned(), req.to_owned())
    });
    Ok(())
}

pub async fn verify_tx(req: GenerateTicketReq) -> Result<bool, GenerateTicketError> {
    let multi_rpc_config = read_config(|s| s.get().multi_rpc_config.to_owned());
    multi_rpc_config
        .check_config_valid()
        .map_err(|e| GenerateTicketError::TemporarilyUnavailable(e.to_string()))?;
    let client = RestClient::client();
    let events = query_tx_from_multi_rpc(
        &client,
        req.tx_hash.to_owned(),
        multi_rpc_config.rpc_list.to_owned(),
    )
    .await;

    let events = multi_rpc_config
        .valid_and_get_result(&events)
        .map_err(|e| GenerateTicketError::TemporarilyUnavailable(e.to_string()))?;

    let mut collect_fee_ok = false;
    let mut burn_token_ok = false;

    let current_package =
        read_config(|c| c.get().current_port_package.to_owned()).expect("port package is none!");

    let port_info =
        read_state(|s| s.aptos_ports.get(&current_package)).expect("port info is none!");

    for event in &events {
        if let Ok(collect_fee_event) =
            serde_json::from_value::<CollectFeeEvent>(event.data.to_owned())
        {
            log!(
                DEBUG,
                "[verify_tx] collect_fee_event: {:?}",
                collect_fee_event
            );
            if let MoveType::Struct(type_info) = &event.typ {
                log!(DEBUG, "[verify_tx] type_info: {:?}", type_info);
                if !(type_info.address.to_string().eq(&port_info.package)
                    && type_info.module.to_string().eq(&port_info.module)
                    && type_info
                        .name
                        .to_string()
                        .eq(&"CollectFeeEvent".to_string()))
                {
                    return Err(GenerateTicketError::TemporarilyUnavailable(format!(
                        "[verify_tx] the collect fee event is not from aptos port! ",
                    )));
                }
            } else {
                return Err(GenerateTicketError::TemporarilyUnavailable(format!(
                    "[verify_tx] move type is not struct",
                )));
            }

            let fee = read_config(|s| s.get().get_fee(req.target_chain_id.to_owned())).ok_or(
                GenerateTicketError::TemporarilyUnavailable(format!(
                    "[verify_tx] No found fee for {}",
                    req.target_chain_id
                )),
            )?;

            log!(DEBUG, "[verify_tx] fee from route: {}", fee);

            let collect_amount = collect_fee_event.fee_amount as u128;

            let fee_account = read_config(|s| s.get().fee_account.to_string());

            log!(DEBUG, "[verify_tx] fee_account from route: {}", fee_account);

            if !(collect_fee_event.sender.to_string().eq(&req.sender)
                && collect_fee_event.recipient.to_string().eq(&fee_account)
                && collect_amount == fee)
            {
                return Err(GenerateTicketError::TemporarilyUnavailable(format!(
                    "[verify_tx] Unable to verify the collect fee info",
                )));
            }
            collect_fee_ok = true
        } else if let Ok(burn_event) = serde_json::from_value::<BurnFAEvent>(event.data.to_owned())
        {
            log!(DEBUG, "[verify_tx] burn_event: {:?}", burn_event);
            if let MoveType::Struct(type_info) = &event.typ {
                log!(DEBUG, "[verify_tx] type_info: {:?}", type_info);
                if !(type_info.address.to_string().eq(&port_info.package)
                    && type_info.module.to_string().eq(&port_info.module)
                    && type_info.name.to_string().eq(&"BurnFAEvent".to_string()))
                {
                    return Err(GenerateTicketError::TemporarilyUnavailable(format!(
                        "[verify_tx] the burn fa event is not from aptos port! ",
                    )));
                }
            } else {
                return Err(GenerateTicketError::TemporarilyUnavailable(format!(
                    "[verify_tx] move type is not struct",
                )));
            }

            let fa_obj_id = read_state(|s| s.aptos_tokens.get(&req.token_id))
                .expect("Not found aptos token")
                .fa_obj_id
                .expect("fa obj id is None");

            if burn_event.sender.to_string().eq(&req.sender)
                && burn_event.fa_obj.eq(&fa_obj_id)
                && burn_event.amount == req.amount
            {
                burn_token_ok = true;
            }
        } else {
            log!(DEBUG, "[verify_tx] Unknown Parsed Value: {:#?}", event);
        }
    }
    log!(
        DEBUG,
        "[verify_tx] verify tx ,collect_fee :{},burn_token:{}",
        collect_fee_ok,
        burn_token_ok,
    );
    Ok(collect_fee_ok && burn_token_ok)
}

/// send ticket to hub
pub async fn send_ticket(hub_principal: Principal, ticket: Ticket) -> Result<(), CallError> {
    let resp: (Result<(), Error>,) =
        ic_cdk::api::call::call(hub_principal, "send_ticket", (ticket,))
            .await
            .map_err(|(code, message)| CallError {
                method: "send_ticket".to_string(),
                reason: Reason::from_reject(code, message),
            })?;
    let data = resp.0.map_err(|err| CallError {
        method: "send_ticket".to_string(),
        reason: Reason::CanisterError(err.to_string()),
    })?;
    Ok(data)
}

pub async fn query_tx_from_multi_rpc(
    client: &RestClient,
    tx_hash: String,
    rpc_url_vec: Vec<String>,
) -> Vec<AptosResult<Transaction>> {
    let mut fut = Vec::with_capacity(rpc_url_vec.len());
    for rpc_url in rpc_url_vec {
        fut.push(async {
            client
                .get_transaction_by_hash(tx_hash.to_owned(), &Some(rpc_url))
                .await
        });
    }
    futures::future::join_all(fut).await
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
// #[serde(rename_all = "camelCase")]
pub struct CollectFeeEvent {
    pub sender: String,
    pub recipient: String,
    #[serde_as(as = "DisplayFromStr")]
    pub fee_amount: u64,
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct BurnFAEvent {
    pub sender: String,
    pub fa_obj: String,
    #[serde_as(as = "DisplayFromStr")]
    pub amount: u64,
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use aptos_api_types::{move_types::MoveType, transaction::Transaction, HashValue};
    use aptos_types::account_address::AccountAddress;
    use candid::Principal;
    use omnity_types::address;

    use crate::handler::gen_ticket::{BurnFAEvent, CollectFeeEvent};

    #[test]
    fn test_management_canister() {
        let principal = Principal::management_canister();
        println!("The management principal value is: {}", principal)
    }

    #[test]
    fn parse_redeem_events() {
        let json_str = r#" 
        {
            "version": "2304331",
            "hash": "0xea605ccb9904d64fb0612ec1703ee62dbda8128af380cddd80a8c659438b9c7e",
            "state_change_hash": "0x7c6ed520dbdac4970bcf5be5790523efa5041d3953db2ff63dafa04016202f0a",
            "event_root_hash": "0x55a0e57c329013a259a2b602785edb70a28b90e75987c4c222afbded7aeee530",
            "state_checkpoint_hash": null,
            "gas_used": "17",
            "success": true,
            "vm_status": "Executed successfully",
            "accumulator_root_hash": "0x9f606bdb244d8acd8b79272303180847fa9e65541af6967e521a200a12b2e007",
            "changes": [
                {
                "address": "0x19b1bb5f38ed05902e344d83c2ba06e5133a20b4e3a28690c2fb1c90784227f1",
                "state_key_hash": "0x28506c778fd16944c1b182846b54df9cde27a8b64596d897d223bb95b92e736b",
                "data": {
                    "type": "0x1::fungible_asset::ConcurrentSupply",
                    "data": {
                    "current": {
                        "max_value": "340282366920938463463374607431768211455",
                        "value": "499777778"
                    }
                    }
                },
                "type": "write_resource"
                },
                {
                "address": "0x19b1bb5f38ed05902e344d83c2ba06e5133a20b4e3a28690c2fb1c90784227f1",
                "state_key_hash": "0x28506c778fd16944c1b182846b54df9cde27a8b64596d897d223bb95b92e736b",
                "data": {
                    "type": "0x1::fungible_asset::Metadata",
                    "data": {
                    "decimals": 8,
                    "icon_uri": "https://raw.githubusercontent.com/octopus-network/omnity-interoperability/9061b7e2ea9e0717b47010279ff1ffd6f1f4c1fc/assets/token_logo/icp.svg",
                    "name": "ICP",
                    "project_uri": "https://www.omnity.network",
                    "symbol": "ICP"
                    }
                },
                "type": "write_resource"
                },
                {
                "address": "0x19b1bb5f38ed05902e344d83c2ba06e5133a20b4e3a28690c2fb1c90784227f1",
                "state_key_hash": "0x28506c778fd16944c1b182846b54df9cde27a8b64596d897d223bb95b92e736b",
                "data": {
                    "type": "0x1::object::ObjectCore",
                    "data": {
                    "allow_ungated_transfer": true,
                    "guid_creation_num": "1125899906842625",
                    "owner": "0x6544025e55c00ac724b64abf2d1a9871139a1b04140cdc3faef44510e9ce7ae6",
                    "transfer_events": {
                        "counter": "0",
                        "guid": {
                        "id": {
                            "addr": "0x19b1bb5f38ed05902e344d83c2ba06e5133a20b4e3a28690c2fb1c90784227f1",
                            "creation_num": "1125899906842624"
                        }
                        }
                    }
                    }
                },
                "type": "write_resource"
                },
                {
                "address": "0x19b1bb5f38ed05902e344d83c2ba06e5133a20b4e3a28690c2fb1c90784227f1",
                "state_key_hash": "0x28506c778fd16944c1b182846b54df9cde27a8b64596d897d223bb95b92e736b",
                "data": {
                    "type": "0x1::primary_fungible_store::DeriveRefPod",
                    "data": {
                    "metadata_derive_ref": {
                        "self": "0x19b1bb5f38ed05902e344d83c2ba06e5133a20b4e3a28690c2fb1c90784227f1"
                    }
                    }
                },
                "type": "write_resource"
                },
                {
                "address": "0x41e3dc5123cdc14ab1248206155fe90d90ab115daf03635aafd91a67c27459bf",
                "state_key_hash": "0x4b626899cbd95b7f21a437e3f45ac9e377f2399748e6a335d23fbdebd823fa89",
                "data": {
                    "type": "0x1::fungible_asset::FungibleStore",
                    "data": {
                    "balance": "199777778",
                    "frozen": false,
                    "metadata": {
                        "inner": "0x19b1bb5f38ed05902e344d83c2ba06e5133a20b4e3a28690c2fb1c90784227f1"
                    }
                    }
                },
                "type": "write_resource"
                },
                {
                "address": "0x41e3dc5123cdc14ab1248206155fe90d90ab115daf03635aafd91a67c27459bf",
                "state_key_hash": "0x4b626899cbd95b7f21a437e3f45ac9e377f2399748e6a335d23fbdebd823fa89",
                "data": {
                    "type": "0x1::object::ObjectCore",
                    "data": {
                    "allow_ungated_transfer": false,
                    "guid_creation_num": "1125899906842625",
                    "owner": "0xeec548b9b358e769e74a7a4ba5c034fbb0c37a9872a4c3d47c8d0cacb2b3bd4f",
                    "transfer_events": {
                        "counter": "0",
                        "guid": {
                        "id": {
                            "addr": "0x41e3dc5123cdc14ab1248206155fe90d90ab115daf03635aafd91a67c27459bf",
                            "creation_num": "1125899906842624"
                        }
                        }
                    }
                    }
                },
                "type": "write_resource"
                },
                {
                "address": "0x7c9a6dbadedb68849b0be9dcbe15e20874fa83b3277c6be00588aa6a2d2f6556",
                "state_key_hash": "0x0000957da16725def225498a5f1a4fab0cab287d50f6cb8c67b6b9d3834644d4",
                "data": {
                    "type": "0x1::coin::CoinStore<0x1::aptos_coin::AptosCoin>",
                    "data": {
                    "coin": {
                        "value": "100440016"
                    },
                    "deposit_events": {
                        "counter": "2",
                        "guid": {
                        "id": {
                            "addr": "0x7c9a6dbadedb68849b0be9dcbe15e20874fa83b3277c6be00588aa6a2d2f6556",
                            "creation_num": "2"
                        }
                        }
                    },
                    "frozen": false,
                    "withdraw_events": {
                        "counter": "0",
                        "guid": {
                        "id": {
                            "addr": "0x7c9a6dbadedb68849b0be9dcbe15e20874fa83b3277c6be00588aa6a2d2f6556",
                            "creation_num": "3"
                        }
                        }
                    }
                    }
                },
                "type": "write_resource"
                },
                {
                "address": "0xeec548b9b358e769e74a7a4ba5c034fbb0c37a9872a4c3d47c8d0cacb2b3bd4f",
                "state_key_hash": "0x5c98d77bc99cca9d5138bc3b694152a518a96e92cc9d0554dcdc173f056e4759",
                "data": {
                    "type": "0x1::coin::CoinStore<0x1::aptos_coin::AptosCoin>",
                    "data": {
                    "coin": {
                        "value": "198648834"
                    },
                    "deposit_events": {
                        "counter": "2",
                        "guid": {
                        "id": {
                            "addr": "0xeec548b9b358e769e74a7a4ba5c034fbb0c37a9872a4c3d47c8d0cacb2b3bd4f",
                            "creation_num": "2"
                        }
                        }
                    },
                    "frozen": false,
                    "withdraw_events": {
                        "counter": "1",
                        "guid": {
                        "id": {
                            "addr": "0xeec548b9b358e769e74a7a4ba5c034fbb0c37a9872a4c3d47c8d0cacb2b3bd4f",
                            "creation_num": "3"
                        }
                        }
                    }
                    }
                },
                "type": "write_resource"
                },
                {
                "address": "0xeec548b9b358e769e74a7a4ba5c034fbb0c37a9872a4c3d47c8d0cacb2b3bd4f",
                "state_key_hash": "0xd3b89743e119a9c408828bdb34ac33ca8524e909a133fe530b8d31ef4f7dd837",
                "data": {
                    "type": "0x1::account::Account",
                    "data": {
                    "authentication_key": "0xeec548b9b358e769e74a7a4ba5c034fbb0c37a9872a4c3d47c8d0cacb2b3bd4f",
                    "coin_register_events": {
                        "counter": "0",
                        "guid": {
                        "id": {
                            "addr": "0xeec548b9b358e769e74a7a4ba5c034fbb0c37a9872a4c3d47c8d0cacb2b3bd4f",
                            "creation_num": "0"
                        }
                        }
                    },
                    "guid_creation_num": "4",
                    "key_rotation_events": {
                        "counter": "0",
                        "guid": {
                        "id": {
                            "addr": "0xeec548b9b358e769e74a7a4ba5c034fbb0c37a9872a4c3d47c8d0cacb2b3bd4f",
                            "creation_num": "1"
                        }
                        }
                    },
                    "rotation_capability_offer": {
                        "for": {
                        "vec": []
                        }
                    },
                    "sequence_number": "4",
                    "signer_capability_offer": {
                        "for": {
                        "vec": []
                        }
                    }
                    }
                },
                "type": "write_resource"
                },
                {
                "state_key_hash": "0x6e4b28d40f98a106a65163530924c0dcb40c1349d3aa915d108b4d6cfc1ddb19",
                "handle": "0x1b854694ae746cdbd8d44186ca4929b2b337df21d1c74633be19b2710552fdca",
                "key": "0x0619dc29a0aac8fa146714058e8dd6d2d0f3bdf5f6331907bf91f3acd81e6935",
                "value": "0xfd2a2a1af6c801000100000000000000",
                "data": null,
                "type": "write_table_item"
                }
            ],
            "sender": "0xeec548b9b358e769e74a7a4ba5c034fbb0c37a9872a4c3d47c8d0cacb2b3bd4f",
            "sequence_number": "3",
            "max_gas_amount": "25",
            "gas_unit_price": "100",
            "expiration_timestamp_secs": "1741922665",
            "payload": {
                "code": {
                "bytecode": "0xa11ceb0b0700000a0701000602060a03100c051c180734550889016010e9011f01030105020600020701000101040b0002070201000102080301000104060c030b00010801030002060c0303060c0b0001080103083c53454c463e5f300672656465656d064f626a656374066f626a656374084d657461646174610e66756e6769626c655f61737365740a6170746f735f706f72740b636f6c6c6563745f666565076275726e5f6661ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff0000000000000000000000000000000000000000000000000000000000000001eec548b9b358e769e74a7a4ba5c034fbb0c37a9872a4c3d47c8d0cacb2b3bd4f14636f6d70696c6174696f6e5f6d65746164617461090003322e3003322e31000001080a000b0111000b000b020b03110102",
                "abi": {
                    "name": "main",
                    "visibility": "public",
                    "is_entry": true,
                    "is_view": false,
                    "generic_type_params": [],
                    "params": [
                    "&signer",
                    "u64",
                    "0x1::object::Object<0x1::fungible_asset::Metadata>",
                    "u64"
                    ],
                    "return": []
                }
                },
                "type_arguments": [],
                "arguments": [
                "666666",
                {
                    "inner": "0x19b1bb5f38ed05902e344d83c2ba06e5133a20b4e3a28690c2fb1c90784227f1"
                },
                "222222"
                ],
                "type": "script_payload"
            },
            "signature": {
                "public_key": "0xe64397bf4d560286773172a13ae334d82352938e8d71347e237db443ab1073e3",
                "signature": "0xa54f91e35f62a7ee3afa9e2e2ad8c989011cca37a3d6ffe5fc4b24286cbf79618a200f24496520f08394fa5706e5a07405c2d0471426b15a87120601d74ad808",
                "type": "ed25519_signature"
            },
            "events": [
                {
                "guid": {
                    "creation_number": "3",
                    "account_address": "0xeec548b9b358e769e74a7a4ba5c034fbb0c37a9872a4c3d47c8d0cacb2b3bd4f"
                },
                "sequence_number": "0",
                "type": "0x1::coin::WithdrawEvent",
                "data": {
                    "amount": "666666"
                }
                },
                {
                "guid": {
                    "creation_number": "2",
                    "account_address": "0x7c9a6dbadedb68849b0be9dcbe15e20874fa83b3277c6be00588aa6a2d2f6556"
                },
                "sequence_number": "1",
                "type": "0x1::coin::DepositEvent",
                "data": {
                    "amount": "666666"
                }
                },
                {
                "guid": {
                    "creation_number": "0",
                    "account_address": "0x0"
                },
                "sequence_number": "0",
                "type": "0xeec548b9b358e769e74a7a4ba5c034fbb0c37a9872a4c3d47c8d0cacb2b3bd4f::aptos_port::CollectFeeEvent",
                "data": {
                    "fee_amount": "666666",
                    "recipient": "0x7c9a6dbadedb68849b0be9dcbe15e20874fa83b3277c6be00588aa6a2d2f6556",
                    "sender": "0xeec548b9b358e769e74a7a4ba5c034fbb0c37a9872a4c3d47c8d0cacb2b3bd4f"
                }
                },
                {
                "guid": {
                    "creation_number": "0",
                    "account_address": "0x0"
                },
                "sequence_number": "0",
                "type": "0x1::fungible_asset::Withdraw",
                "data": {
                    "amount": "222222",
                    "store": "0x41e3dc5123cdc14ab1248206155fe90d90ab115daf03635aafd91a67c27459bf"
                }
                },
                {
                "guid": {
                    "creation_number": "0",
                    "account_address": "0x0"
                },
                "sequence_number": "0",
                "type": "0xeec548b9b358e769e74a7a4ba5c034fbb0c37a9872a4c3d47c8d0cacb2b3bd4f::aptos_port::BurnFAEvent",
                "data": {
                    "amount": "222222",
                    "fa_obj": "0x19b1bb5f38ed05902e344d83c2ba06e5133a20b4e3a28690c2fb1c90784227f1",
                    "sender": "0xeec548b9b358e769e74a7a4ba5c034fbb0c37a9872a4c3d47c8d0cacb2b3bd4f"
                }
                },
                {
                "guid": {
                    "creation_number": "0",
                    "account_address": "0x0"
                },
                "sequence_number": "0",
                "type": "0x1::transaction_fee::FeeStatement",
                "data": {
                    "execution_gas_units": "6",
                    "io_gas_units": "12",
                    "storage_fee_octas": "0",
                    "storage_fee_refund_octas": "0",
                    "total_charge_gas_units": "17"
                }
                }
            ],
            "timestamp": "1741922635593312",
            "type": "user_transaction"
        }"#;
        let json_tx = serde_json::from_str::<Transaction>(json_str);
        println!("json_response: {:?}", json_tx);
        let tx = json_tx.unwrap();
        let events = if let Transaction::UserTransaction(user_tx) = tx {
            user_tx.events
        } else {
            vec![]
        };
        // println!("tx events: {:#?}", events);
        for event in &events {
            println!("event: {:#?}", event);
            if let MoveType::Struct(type_info) = &event.typ {
                println!("type_info: {:#?}", type_info);
            }
            // let parsed_json = serde_json::to_string(&event.parsed_json).unwrap();

            if let Ok(collect_fee_event) =
                serde_json::from_value::<CollectFeeEvent>(event.data.to_owned())
            {
                println!("collect_fee_event: {:#?}", collect_fee_event);
            } else if let Ok(burn_event) =
                serde_json::from_value::<BurnFAEvent>(event.data.to_owned())
            {
                println!("burn_event: {:#?}", burn_event);
            } else {
                println!(" Unknown Parsed Value: {:?}", event.data);
            }
        }
    }

    #[test]
    fn memo_with_fee() {
        use crate::types::Memo;
        // user memo is Some(...)
        let memo = Some("some memo".to_string());
        let fee = 20000000 as u128;
        let memo_with_fee = Memo {
            memo,
            bridge_fee: fee,
        };

        let memo = serde_json::to_string_pretty(&memo_with_fee).unwrap();
        println!("[generate_ticket] Memo is some and fee: {:?}", memo);

        // user memo is None
        let memo = None;
        let fee = 20000000 as u128;
        let memo_with_fee = Memo {
            memo,
            bridge_fee: fee,
        };
        let memo = serde_json::to_string_pretty(&memo_with_fee).unwrap();

        println!("[generate_ticket] Memo is None and fee: {:?}", memo);
    }

    #[test]
    fn test_gen_req_valid() {
        let tx_hash_str = "0xea605ccb9904d64fb0612ec1703ee62dbda8128af380cddd80a8c659438b9c7e";
        let tx_hash = HashValue::from_str(tx_hash_str);
        println!("tx hash: {:?}", tx_hash);
        assert!(tx_hash.is_ok());
        let send_str =
            "0xeec548b9b358e769e74a7a4ba5c034fbb0c37a9872a4c3d47c8d0cacb2b3bd4f".to_string();
        let send = AccountAddress::from_str(&send_str);
        println!("aptos address: {:?}", send);
        assert!(send.is_ok());

        let chain_id = "sICP";
        let receiver_str =
            "ytoqu-ey42w-sb2ul-m7xgn-oc7xo-i4btp-kuxjc-b6pt4-dwdzu-kfqs4-nae".to_string();
        let ret = address::validate_account(&chain_id.to_string(), &receiver_str);
        println!("address::validate_account ret: {:?}", ret);
        assert!(ret.is_ok());
    }
}
