use crate::aptos_client::rest_client::RestClient;
use crate::aptos_client::AptosResult;
use crate::config::read_config;
use crate::types::{ChainState, Error, TicketType, TxAction};
use crate::types::{Memo, Ticket};
use aptos_api_types::transaction::Transaction;
use candid::{CandidType, Principal};

use crate::ic_log::{DEBUG, WARNING};
// use crate::ic_sui::sui_types::sui_serde::BigInt;
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

use ic_canister_log::log;

// use serde_json::from_value;
// use serde_json::Value;

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

    mutate_state(|s| {
        s.gen_ticket_reqs
            .insert(req.tx_hash.to_owned(), req.to_owned())
    });

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

    if !read_state(|s| s.tokens.contains_key(&req.token_id.to_string())) {
        return Err(GenerateTicketError::UnsupportedToken(
            req.token_id.to_owned(),
        ));
    }

    if !matches!(req.action, TxAction::Redeem) {
        return Err(GenerateTicketError::UnsupportedAction(
            "[generate_ticket] Transfer action is not supported".into(),
        ));
    }

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

pub async fn verify_tx(req: GenerateTicketReq) -> Result<bool, GenerateTicketError> {
    let multi_rpc_config = read_config(|s| s.get().multi_rpc_config.to_owned());
    multi_rpc_config
        .check_config_valid()
        .map_err(|e| GenerateTicketError::TemporarilyUnavailable(e.to_string()))?;
    let client = RestClient::new();
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

    if events.len() < 3 {
        return Err(GenerateTicketError::TemporarilyUnavailable(
            "events size should be >= 3".to_string(),
        ));
    }
    // let sui_port_action = read_config(|s| s.get().current_port_package.to_owned());
    for event in &events {
        //TODO: check scripts bytecode
        // if !event.package_id.to_string().eq(&sui_port_action.package) {
        //     return Err(GenerateTicketError::TemporarilyUnavailable(
        //         "event is not from sui port action".to_string(),
        //     ));
        // }
        if let Ok(collect_fee_event) =
            serde_json::from_value::<CollectFeeEvent>(event.data.to_owned())
        {
            log!(
                DEBUG,
                "[verify_tx] collect_fee_event: {:?}",
                collect_fee_event
            );
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

            let fa_obj_id = read_state(|s| s.atptos_tokens.get(&req.token_id))
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
    pub memo: Option<String>,
}

#[cfg(test)]
mod test {
    use aptos_api_types::transaction::Transaction;
    use candid::Principal;

    use crate::{aptos_client::response::Response, handler::gen_ticket::CollectFeeEvent};

    #[test]
    fn test_management_canister() {
        let principal = Principal::management_canister();
        println!("The management principal value is: {}", principal)
    }

    #[test]
    fn parse_redeem_events() {
        let json_str = r#" 
          {
            "jsonrpc": "2.0",
            "result": [
                {
                    "id": {
                        "txDigest": "32LPck96ThoVAGUcKDs6d4As9oDfUJS4UeE3FevbfHgd",
                        "eventSeq": "0"
                    },
                    "packageId": "0x58cf166ca300288cd32bbe5d9e864955301cf3e41a3cd027c6dd4a5760bfc628",
                    "transactionModule": "action",
                    "sender": "0x021e364dfa89ce87cbfbbae322ebd730c0737ff10a41d4a3b295f1b386031c51",
                    "type": "0x58cf166ca300288cd32bbe5d9e864955301cf3e41a3cd027c6dd4a5760bfc628::action::CollectFeeEvent",
                    "parsedJson": {
                        "fee_amount": "20000000",
                        "fee_coin_id": "4091bc4cdfcbf107805aba3cc318395253be7578b881054e00224aaba2215840",
                        "recipient": "af9306cac62396be300b175046140c392eed876bd8ac0efac6301cea286fa272",
                        "sender": "021e364dfa89ce87cbfbbae322ebd730c0737ff10a41d4a3b295f1b386031c51"
                    },
                    "bcsEncoding": "base64",
                    "bcs": "QDAyMWUzNjRkZmE4OWNlODdjYmZiYmFlMzIyZWJkNzMwYzA3MzdmZjEwYTQxZDRhM2IyOTVmMWIzODYwMzFjNTFAYWY5MzA2Y2FjNjIzOTZiZTMwMGIxNzUwNDYxNDBjMzkyZWVkODc2YmQ4YWMwZWZhYzYzMDFjZWEyODZmYTI3MkA0MDkxYmM0Y2RmY2JmMTA3ODA1YWJhM2NjMzE4Mzk1MjUzYmU3NTc4Yjg4MTA1NGUwMDIyNGFhYmEyMjE1ODQwAC0xAQAAAAA="
                    },
                    {
                    "id": {
                        "txDigest": "32LPck96ThoVAGUcKDs6d4As9oDfUJS4UeE3FevbfHgd",
                        "eventSeq": "1"
                    },
                    "packageId": "0x58cf166ca300288cd32bbe5d9e864955301cf3e41a3cd027c6dd4a5760bfc628",
                    "transactionModule": "action",
                    "sender": "0x021e364dfa89ce87cbfbbae322ebd730c0737ff10a41d4a3b295f1b386031c51",
                    "type": "0x58cf166ca300288cd32bbe5d9e864955301cf3e41a3cd027c6dd4a5760bfc628::action::BurnEvent",
                    "parsedJson": {
                        "burned_amount": "700000000",
                        "burned_coin_id": "e8b9bb426c11dd4d2546191175a9816356a06208ba00d30074707c7000951737",
                        "recipient": "bdaec7bab097484feaf9719d85951c81532d584a82bd8334b96c8b484780f0e9",
                        "sender": "021e364dfa89ce87cbfbbae322ebd730c0737ff10a41d4a3b295f1b386031c51"
                    },
                    "bcsEncoding": "base64",
                    "bcs": "QDAyMWUzNjRkZmE4OWNlODdjYmZiYmFlMzIyZWJkNzMwYzA3MzdmZjEwYTQxZDRhM2IyOTVmMWIzODYwMzFjNTFAYmRhZWM3YmFiMDk3NDg0ZmVhZjk3MTlkODU5NTFjODE1MzJkNTg0YTgyYmQ4MzM0Yjk2YzhiNDg0NzgwZjBlOUBlOGI5YmI0MjZjMTFkZDRkMjU0NjE5MTE3NWE5ODE2MzU2YTA2MjA4YmEwMGQzMDA3NDcwN2M3MDAwOTUxNzM3ACe5KQAAAAA="
                    },
                    {
                    "id": {
                        "txDigest": "32LPck96ThoVAGUcKDs6d4As9oDfUJS4UeE3FevbfHgd",
                        "eventSeq": "2"
                    },
                    "packageId": "0x58cf166ca300288cd32bbe5d9e864955301cf3e41a3cd027c6dd4a5760bfc628",
                    "transactionModule": "action",
                    "sender": "0x021e364dfa89ce87cbfbbae322ebd730c0737ff10a41d4a3b295f1b386031c51",
                    "type": "0x58cf166ca300288cd32bbe5d9e864955301cf3e41a3cd027c6dd4a5760bfc628::action::RedeemEvent",
                    "parsedJson": {
                        "action": "Redeem",
                        "amount": "700000000",
                        "memo": "This ticket is redeemed from Sui to Bitcoin",
                        "receiver": "bc1qmh0chcr9f73a3ynt90k0w8qsqlydr4a6espnj6",
                        "sender": "021e364dfa89ce87cbfbbae322ebd730c0737ff10a41d4a3b295f1b386031c51",
                        "target_chain_id": "sICP",
                        "token_id": "sICP-native-ICP"
                    },
                    "bcsEncoding": "base64",
                    "bcs": "BHNJQ1APc0lDUC1uYXRpdmUtSUNQQDAyMWUzNjRkZmE4OWNlODdjYmZiYmFlMzIyZWJkNzMwYzA3MzdmZjEwYTQxZDRhM2IyOTVmMWIzODYwMzFjNTEqYmMxcW1oMGNoY3I5ZjczYTN5bnQ5MGswdzhxc3FseWRyNGE2ZXNwbmo2ACe5KQAAAAAGUmVkZWVtAStUaGlzIHRpY2tldCBpcyByZWRlZW1lZCBmcm9tIFN1aSB0byBCaXRjb2lu"
                    }
                ],
                "id": 1
            }
            "#;
        // let json_response = serde_json::from_str::<Response<Transaction>>(json_str);
        // println!("json_response: {:?}", json_response);
        // let events = json_response.unwrap().result.unwrap();
        // // println!("events: {:#?}", events);
        // for event in &events {
        //     let parsed_json = serde_json::to_string(&event.parsed_json).unwrap();
        //     println!("parsed_json: {:#?}", parsed_json);

        //     if let Ok(collect_fee_event) =
        //         serde_json::from_value::<CollectFeeEvent>(event.parsed_json.to_owned())
        //     {
        //         println!("collect_fee_event: {:#?}", collect_fee_event);
        //     } else if let Ok(burn_event) =
        //         serde_json::from_value::<BurnFAEvent>(event.parsed_json.to_owned())
        //     {
        //         println!("burn_event: {:#?}", burn_event);
        //     } else {
        //         println!(" Unknown Parsed Value: {:?}", event.parsed_json);
        //     }
        // }
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
}
