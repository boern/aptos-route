
use crate::aptos_client::rest_client::RestClient;
use crate::aptos_client::{tx_builder, LocalAccount, MintTokenReq, TxReq};
use crate::types::{ Error,TicketId};
use candid::{ CandidType, Principal};

use ic_stable_structures::Storable;
use ic_stable_structures::storable::Bound;

use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use crate::state::TxStatus;
use crate::{
    call_error::{CallError, Reason},
    state::{mutate_state, read_state},
};
use crate::config::read_config;

use crate::constants::{ RETRY_NUM, TAKE_SIZE};
use ic_canister_log::log;
use crate::ic_log::{ WARNING, DEBUG, ERROR};


#[derive(CandidType, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum MintTokenError {
    NotFoundToken(String),
    UnsupportedToken(String),
    AlreadyProcessed(TicketId),
    TemporarilyUnavailable(String),
    TxError(String),
}

#[derive(CandidType, Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct MintTokenRequest {
    pub ticket_id: TicketId,
    pub token_id: String,
    pub recipient: String,
    pub amount: u64,
    pub status: TxStatus,
    pub tx_hash: Option<String>,
    pub object: Option<String>,
    pub retry:u64,
}

impl Storable for MintTokenRequest {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {

        let bytes = bincode::serialize(&self).expect("failed to serialize MintTokenRequest");
        Cow::Owned(bytes)
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
      
        bincode::deserialize(bytes.as_ref()).expect("failed to deserialize MintTokenRequest")
    }

    const BOUND: Bound = Bound::Unbounded;
}

pub async fn mint_token() {
    // take tickets to mint,very time
    let tickets = read_state(|s| {
        s.tickets_queue
            .iter()
            .take(TAKE_SIZE.try_into().unwrap())
            .map(|(seq, ticket)| (seq, ticket))
            .collect::<Vec<_>>()
    });
    
    for (seq, ticket) in tickets.into_iter() {
      
        let mint_req = match read_state(|s| s.mint_token_requests.get(&ticket.ticket_id)){
            None => {
                // new req
                let mint_req= MintTokenRequest {
                    ticket_id: ticket.ticket_id.to_owned(),
                    token_id: ticket.token.to_owned(),
                    recipient: ticket.receiver.to_owned(),
                    amount: ticket.amount.parse::<u64>().unwrap(),
                    status: TxStatus::New,
                    tx_hash: None,
                    object:None,
                    retry:0
                };
                // save new token req
                mutate_state(|s| s.mint_token_requests.insert(mint_req.ticket_id.to_string(), mint_req.to_owned()));
                mint_req
            }
            Some(mint_req) => {
                if mint_req.retry >= RETRY_NUM {
                  
                    log!(
                        WARNING,
                       "[mint_token::mint_token] failed to mint token for ticket id: {}, and reach to max retry,pls contact your administrator",
                        ticket.ticket_id
                    );
                    continue;
                }    
                mint_req
            }
            
        };

        log!(DEBUG, "[mint_token::mint_token] mint token request: {:?} ", mint_req);

        match &mint_req.status {
            TxStatus::New => {
               
                inner_mint_token(&mint_req).await;
                
              
            },
            TxStatus::Pending => {
                log!(
                    DEBUG,
                    "[mint_token::mint_token] the mint token request ({:?}) is handling, pls wait ...",
                    mint_req
                );
                let tx_hash = mint_req.tx_hash.expect("Not found tx hash");
                 // update status
                 update_tx_status(tx_hash, mint_req.token_id.to_owned()).await;
      
               
            }
            TxStatus::Finalized  => {
                log!(
                    DEBUG,
                    "[mint_token::mint_token] the mint token request ({:?}) is finalized !",
                    mint_req
                );
               
                //only finalized mint_req, remove the handled ticket from queue  
                mutate_state(|s|{ 
                    s.tickets_queue.remove(&seq);
                   
                });
                // update tx hash to hub
                let hub_principal = read_config(|s| s.get().hub_principal);
                let digest = mint_req.tx_hash.unwrap();
                       
                match update_tx_to_hub(hub_principal, ticket.ticket_id.to_string(), digest.to_owned()).await {
                   Ok(()) =>{
                       log!(
                           DEBUG,
                           "[mint_token::mint_token] mint req tx({:?}) already finallized and update tx digest to hub! ",
                           digest
                       );
                   }
                   Err(err) =>  {
                       log!(
                        ERROR,
                           "[mint_token::mint_token] failed to update tx hash to hub:{}",
                           err
                       );
                   }
               }   
                                      
           }
            TxStatus::TxFailed { e } => {
              
                if mint_req.retry < RETRY_NUM {
                    log!(
                        WARNING,
                       "[mint_token::mint_token] failed to mint token for ticket id: {}, error: {:} , and retry ... ",
                        ticket.ticket_id,e 
                    );
                    inner_mint_token(&mint_req).await;
                } 
            },
            
        }
         
    }
}


pub async fn inner_mint_token(mint_req:&MintTokenRequest) {

    let fa_obj_id = read_state(|s|s.atptos_tokens.get(&mint_req.token_id)).expect("aptos token is None").fa_obj_id.expect("fa obj id is None");
    let req = MintTokenReq {
         ticket_id: mint_req.ticket_id.to_owned(),
        fa_obj: fa_obj_id,
        recipient: mint_req.recipient.to_owned(),
        mint_acmount: mint_req.amount,
    };
    match  LocalAccount::local_account().await{
        Ok(mut local_account) => {
            let tx_req = TxReq::MintToken(req);
            if let Ok(signed_txn) = tx_builder::get_signed_tx(&mut local_account, tx_req, None)
                .await {
                    log!(
                        DEBUG,
                        "[mint_token::inner_mint_token] SignedTransaction: {:#?} ",
                        signed_txn
                    );
                    let client = RestClient::new();
                    match client.summit_tx(&signed_txn, &client.forward).await {
                        Ok(tx) => {
                            log!(
                                DEBUG,
                                "[mint_token::inner_mint_token] summit_tx ret: {:?}  ",
                                tx
                            );
               
                            mutate_state(|s| {
                                if let Some(req)=s.mint_token_requests.get(&mint_req.ticket_id).as_mut() {
                                    req.status= TxStatus::Pending;
                                    req.tx_hash = Some(tx.hash.to_string());
                                 
                                    s.mint_token_requests.insert(mint_req.ticket_id.to_owned(),req.to_owned());
                                }
                            });
                        }
                        Err(tx_error) => {
                            log!(
                                ERROR,
                                "[mint_token::inner_mint_token] summit_tx error: {:?}  ",
                                tx_error
                            );
                   
                            // update retry
                         
                            mutate_state(|s| {
                                if let Some(req)=s.mint_token_requests.get(&mint_req.ticket_id).as_mut() {
                                    req.status =TxStatus::TxFailed { e: tx_error.to_string() };
                                    req.retry +=1;
                                    s.mint_token_requests.insert(mint_req.ticket_id.to_string(),req.to_owned());
                                }
                            });
                        }
                    }

            } else {
                    log!(
                        DEBUG,
                        "[mint_token::inner_mint_token] get_signed_tx error",
                    );
                }
        }
        Err(e) => {
            log!(
                ERROR,
                "[mint_token::inner_mint_token] get local_account error: {:?}  ",
                e
            );
        }
    }
   
}


pub async fn update_tx_status(tx_hash: String, ticket_id: String) {
    // query signature status
    let client = RestClient::new();
    let tx = client.get_transaction_by_hash(tx_hash.to_owned(), &client.forward).await;
    match tx {
        Err(e) => {
            log!(
                WARNING,
                "[mint_token::update_tx_status] get_transaction_by_hash for {} ,err: {:?}",
                tx_hash,
                e
            );
            
        }
        Ok(tx) => {
            if tx.is_pending() {
                log!(
                    DEBUG,
                    "[mint_token::update_tx_status] tx {} is pending, pls waiting ...",
                    tx_hash.to_string(),
                    
                );
                return
            }
            if tx.success() {
                log!(
                    DEBUG,
                    "[mint_token::update_tx_status] mint token req for ticket id: {} successfully!",
                    ticket_id
                );
                mutate_state(|s| {
                    if let Some(req)=s.mint_token_requests.get(&ticket_id).as_mut() {
                        req.status= TxStatus::Finalized ;
                        s.mint_token_requests.insert(ticket_id.to_owned(),req.to_owned());
                    }
                });
            } else {
               // update status and retry
               log!(
                ERROR,
                    "[mint_token::update_tx_status] tx execute failured: {} ",tx.vm_status()
                );
                mutate_state(|s| {
                    if let Some(req)=s.mint_token_requests.get(&ticket_id).as_mut() {
                        req.status =TxStatus::TxFailed { e: tx.vm_status() };
                        req.retry +=1;
                        req.tx_hash= None;
                        s.mint_token_requests.insert(ticket_id.to_string(),req.to_owned());
                    }
                });
                
            }
          
        }
    }
}



pub async fn update_tx_to_hub(
    hub_principal: Principal,
    ticket_id: TicketId,
    mint_tx_hash: String,
) -> Result<(), CallError> {
    let resp: (Result<(), Error>,) =
        ic_cdk::api::call::call(hub_principal, "update_tx_hash", (ticket_id, mint_tx_hash))
            .await
            .map_err(|(code, message)| CallError {
                method: "update_tx_hash".to_string(),
                reason: Reason::from_reject(code, message),
            })?;
    resp.0.map_err(|err| CallError {
        method: "update_tx_hash".to_string(),
        reason: Reason::CanisterError(err.to_string()),
    })?;
    Ok(())
}

#[cfg(test)]
mod test {
    #[test]
    fn test_match_status_error() {
        
    }
}
