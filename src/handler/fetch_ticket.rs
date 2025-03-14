#![allow(unused)]
use std::str::FromStr;

use crate::aptos_client::{MintTokenReq, ReqType, TxReq, TxStatus};
use crate::ck_eddsa::hash_with_sha256;
use crate::config::{mutate_config, read_config};
use crate::constants::TICKET_LIMIT_SIZE;
use crate::ic_log::ERROR;

use crate::state::read_state;
use crate::types::{ChainId, ChainState, Error, Seq, Ticket};
use aptos_types::account_address::AccountAddress;
use candid::Principal;

use crate::{
    call_error::{CallError, Reason},
    state::mutate_state,
};

use ic_canister_log::log;

/// handler tickets from customs to sui
pub async fn query_tickets() {
    if read_config(|s| s.get().chain_state == ChainState::Deactive) {
        return;
    }

    let (hub_principal, offset) =
        read_config(|s| (s.get().hub_principal, s.get().seqs.next_ticket_seq));
    match inner_query_tickets(hub_principal, offset, TICKET_LIMIT_SIZE).await {
        Ok(tickets) => {
            let mut next_seq = offset;
            for (seq, ticket) in &tickets {
                if let Err(e) = AccountAddress::from_str(&ticket.receiver) {
                    log!(
                        ERROR,
                        "[fetch_ticket::query_tickets] failed to parse ticket receiver: {}, error:{}",
                        ticket.receiver,
                        e.to_string()
                    );
                    next_seq = seq + 1;
                    continue;
                };
                if let Err(e) = ticket.amount.parse::<u64>() {
                    log!(
                        ERROR,
                        "[fetch_ticket::query_tickets] failed to parse ticket amount: {}, Error:{}",
                        ticket.amount,
                        e.to_string()
                    );
                    next_seq = seq + 1;
                    continue;
                };
                let fa_obj_id = read_state(|s| s.aptos_tokens.get(&ticket.token))
                    .expect("aptos token is None")
                    .fa_obj_id
                    .expect("fungible asset object id is None");
                let req_id = hash_with_sha256(
                    &bincode::serialize(&ticket).expect("failed to serialize ticket"),
                );
                let mint_req = MintTokenReq {
                    ticket_id: ticket.ticket_id.to_owned(),
                    token_id: ticket.token.to_owned(),
                    fa_obj: fa_obj_id,
                    recipient: ticket.receiver.to_owned(),
                    mint_acmount: ticket.amount.parse::<u64>().unwrap(),
                };
                let tx_req = TxReq {
                    req_type: ReqType::MintToken(mint_req.to_owned()),
                    tx_hash: None,
                    tx_status: TxStatus::New,
                    retry: 0,
                };

                mutate_state(|s| s.tx_queue.insert(req_id, tx_req));

                next_seq = seq + 1;
            }
            mutate_config(|s| {
                let mut config = s.get().to_owned();
                config.seqs.next_ticket_seq = next_seq;
                s.set(config);
            })
        }
        Err(e) => {
            log!(
                ERROR,
                "[fetch_ticket::query_tickets] failed to query tickets, err: {}",
                e.to_string()
            );
        }
    }
}

/// query ticket from hub
pub async fn inner_query_tickets(
    hub_principal: Principal,
    offset: u64,
    limit: u64,
) -> Result<Vec<(Seq, Ticket)>, CallError> {
    let resp: (Result<Vec<(Seq, Ticket)>, Error>,) = ic_cdk::api::call::call(
        hub_principal,
        "query_tickets",
        (None::<Option<ChainId>>, offset, limit),
    )
    .await
    .map_err(|(code, message)| CallError {
        method: "query_tickets".to_string(),
        reason: Reason::from_reject(code, message),
    })?;
    let data = resp.0.map_err(|err| CallError {
        method: "query_tickets".to_string(),
        reason: Reason::CanisterError(err.to_string()),
    })?;
    Ok(data)
}
