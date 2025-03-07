
use crate::aptos_client::rest_client::RestClient;
use crate::aptos_client::{tx_builder, LocalAccount, ReqType, TxStatus};

use crate::constants::RETRY_NUM;
use crate::ic_log::{DEBUG, ERROR, WARNING};

use crate::state::UpdateTokenStatus;


use crate::state::{mutate_state, read_state};
use ic_canister_log::log;

pub async fn update_token() {
    if read_state(|s| s.update_token_queue.is_empty()) {
        return;
    }

    if let Some((_token_id, update_status)) = mutate_state(|s| s.update_token_queue.pop_first()) {
        if update_status.retry >= RETRY_NUM {
            return;
        }

        let update_token_req = read_state(|s| s.update_token_queue.get(&update_status.token_id))
            .expect("aptos token should exists");

            match &update_token_req.status {
                TxStatus::New => {
                    inner_update_token(&update_token_req).await;
                }
                TxStatus::Pending => {
                    match &update_token_req.tx_hash {
                        // be creating
                        None => {
                            log!(
                                DEBUG,
                                "[handler::update_token] the update token req ({:?}) is submited, please waite ... ",
                                update_token_req.req
                                
                            );
                        }
                         // signature exists,but not finallized
                        Some(hash) => {
                            log!(
                                DEBUG,
                                "[handler::update_token] the  update token req  ({:?}) already submited and waiting for the tx({:}) to be finallized ... ",
                                update_token_req.req,hash
                                
                            );
                            // update status
                            update_req_status(hash.to_owned(), &update_token_req).await;
                        }
                    }
                
                }
                TxStatus::Finalized => {
                    log!(
                        DEBUG,
                        "[handler::update_token] update token: {:?} Already finalized !",
                       update_token_req,
                    );
                }
                TxStatus::TxFailed { e } => {
                    log!(
                        DEBUG,
                        "[handler::update_token]  failed to  update token for {:},error:{:}, retry ..",
                        update_token_req.token_id,e.to_string()
                    );
                    //retry
                    inner_update_token(&update_token_req).await;
    
                }
            }
    }
}

pub async fn inner_update_token(update_status: &UpdateTokenStatus) {
    match LocalAccount::local_account().await {
        Ok(mut local_account) => {
            let tx_req = ReqType::UpdateMeta(update_status.req.to_owned());
            if let Ok(signed_txn) =
                tx_builder::get_signed_tx(&mut local_account, &tx_req, None).await
            {
                log!(
                    DEBUG,
                    "[update_token::inner_update_token] SignedTransaction: {:#?} ",
                    signed_txn
                );
                let client = RestClient::client();
                match client.summit_tx(&signed_txn, &client.forward).await {
                    Ok(tx) => {
                        log!(
                            DEBUG,
                            "[update_token::inner_update_token] summit_tx ret: {:?}  ",
                            tx
                        );
                        let n_update = UpdateTokenStatus {
                            token_id: update_status.token_id.to_owned(),
                            req: update_status.req.to_owned(),
                            retry: update_status.retry,
                            tx_hash: Some(tx.hash.to_string()),
                            status: TxStatus::Pending,
                        };
                        mutate_state(|s| {
                            s.update_token_queue
                                .insert(update_status.token_id.to_owned(), n_update)
                        });
                    }
                    Err(tx_error) => {
                        log!(
                            ERROR,
                            "[update_token::inner_update_token] summit_tx error: {:?}  ",
                            tx_error
                        );
                        let re_update = UpdateTokenStatus {
                            token_id: update_status.token_id.to_owned(),
                            req: update_status.req.to_owned(),
                            retry: update_status.retry + 1,
                            tx_hash: update_status.tx_hash.to_owned(),
                            status: TxStatus::TxFailed {
                                e: tx_error.to_string(),
                            },
                        };
                        mutate_state(|s| {
                            s.update_token_queue
                                .insert(update_status.token_id.to_owned(), re_update)
                        });
                    }
                }
            } else {
                log!(
                    DEBUG,
                    "[update_token::inner_update_token] get_signed_tx error",
                );
            }
        }
        Err(e) => {
            log!(
                ERROR,
                "[update_token::inner_update_token] get local_account error: {:?}  ",
                e
            );
        }
    }
}


pub async fn update_req_status(tx_hash: String, update_status: &UpdateTokenStatus) {
    // query signature status
    let client = RestClient::client();
    let tx = client.get_transaction_by_hash(tx_hash.to_owned(), &client.forward).await;
    match tx {
        Err(e) => {
            log!(
                WARNING,
                "[create_token::update_tx_status] get_transaction_by_hash for {} ,err: {:?}",
                tx_hash,
                e
            );
            
        }
        Ok(tx) => {
            if tx.is_pending() {
                log!(
                    DEBUG,
                    "[create_token::update_tx_status] tx {} is pending, pls waiting ...",
                    tx_hash.to_string(),
                    
                );
                return
            }
            if tx.success() {
                let n_update = UpdateTokenStatus {
                    token_id: update_status.token_id.to_owned(),
                    req: update_status.req.to_owned(),
                    retry: update_status.retry,
                    tx_hash:  update_status.tx_hash.to_owned(),
                    status: TxStatus::Finalized,
                };
                mutate_state(|s| {
                    s.update_token_queue
                        .insert(update_status.token_id.to_owned(), n_update)
                });

            } else {
               // update status and retry
               let re_update = UpdateTokenStatus {
                token_id: update_status.token_id.to_owned(),
                req: update_status.req.to_owned(),
                retry: update_status.retry + 1,
                tx_hash: update_status.tx_hash.to_owned(),
                status: update_status.status.to_owned()};
            
                mutate_state(|s| {
                    s.update_token_queue
                        .insert(update_status.token_id.to_owned(), re_update)
                });
                    
            }
          
        }
    }
}
