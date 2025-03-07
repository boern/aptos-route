use crate::aptos_client::rest_client::RestClient;
use crate::aptos_client::{tx_builder, LocalAccount, ReqType, TxReq, TxStatus};

use crate::constants::TAKE_SIZE;
use crate::ic_log::{DEBUG, ERROR, WARNING};
use crate::state::{mutate_state, read_state};
use ic_canister_log::log;

use super::create_token::update_fa_obj;
use super::mint_token::update_tx_to_hub;

pub trait TxReqStatus {
    fn handle_new_req(&self);
    fn handle_pending(&self);
    fn hanle_finalized(&self);
    fn hanled_failed(&self);
}

pub async fn handle_req_status() {
    let reqs = read_state(|s| {
        s.tx_queue
            .iter()
            .take(TAKE_SIZE.try_into().unwrap())
            .map(|(tx_req, tx_status)| (tx_req, tx_status))
            .collect::<Vec<_>>()
    });

    for (req, status) in reqs.into_iter() {
        match &status {
            TxStatus::New | TxStatus::TxFailed { .. } => {
                build_and_send_tx(&req).await;
            }
            TxStatus::Pending => {
                // req.handle_pending();
                update_req_status(&req).await;
            }
            TxStatus::Finalized => {
                // req.hanle_finalized()
                log!(
                    DEBUG,
                    "[handler_tx::handle_req_status] req ({:?}) already finalized!",
                    req
                );
            }
        }
    }
}

pub async fn build_and_send_tx(req: &TxReq) {
    log!(DEBUG, "[handler_tx::build_and_send_tx] req ({:?})", req,);

    match LocalAccount::local_account().await {
        Ok(mut local_account) => {
            if let Ok(signed_txn) =
                tx_builder::get_signed_tx(&mut local_account, &req.req_type, None).await
            {
                log!(
                    DEBUG,
                    "[handler_tx::build_and_send_tx] SignedTransaction: {:#?} ",
                    signed_txn
                );
                let client = RestClient::client();
                match client.summit_tx(&signed_txn, &client.forward).await {
                    Ok(pending_tx) => {
                        log!(
                            DEBUG,
                            "[handler_tx::build_and_send_tx] summit_tx ret: {:?}  ",
                            pending_tx
                        );
                        //update req
                        mutate_state(|s| {
                            if let Some(mut tx_status) = s.tx_queue.get(req) {
                                tx_status = TxStatus::Pending;
                                let n_req = TxReq {
                                    req_type: req.req_type.to_owned(),
                                    retry: req.retry,
                                    tx_hash: Some(pending_tx.hash.to_string()),
                                };
                                s.tx_queue.insert(n_req, tx_status);
                            }
                        });
                    }
                    Err(tx_error) => {
                        log!(
                            ERROR,
                            "[handler_tx::build_and_send_tx] summit_tx error: {:?}  ",
                            tx_error
                        );

                        // update req status to failed and retry later
                        mutate_state(|s| {
                            if let Some(_) = s.tx_queue.get(&req).as_mut() {
                                let tx_status = TxStatus::TxFailed {
                                    e: tx_error.to_string(),
                                };
                                let n_req = TxReq {
                                    req_type: req.req_type.to_owned(),
                                    retry: req.retry + 1,
                                    tx_hash: None,
                                };

                                s.tx_queue.insert(n_req, tx_status);
                            }
                        });
                    }
                }
            } else {
                log!(DEBUG, "[handler_tx::build_and_send_tx] get_signed_tx error",);
            }
        }
        Err(e) => {
            log!(
                ERROR,
                "[handler_tx::build_and_send_tx] get local_account error: {:?} ",
                e
            );
        }
    }
}

pub async fn update_req_status(req: &TxReq) {
    // query signature status
    let client = RestClient::client();
    let tx = client
        .get_transaction_by_hash(
            req.tx_hash.to_owned().expect("tx hash is None!"),
            &client.forward,
        )
        .await;
    match tx {
        Err(e) => {
            log!(
                WARNING,
                "[handler_tx::update_req_status] get_transaction_by_hash for {:?} ,err: {:?}",
                req.tx_hash,
                e
            );
        }
        Ok(tx) => {
            if tx.is_pending() {
                log!(
                    DEBUG,
                    "[handler_tx::update_req_status] tx {:?} is pending, pls waiting ...",
                    req.tx_hash,
                );
                return;
            }
            if tx.success() {
                // req finalized
                mutate_state(|s| {
                    if let Some(mut tx_status) = s.tx_queue.get(req) {
                        tx_status = TxStatus::Finalized;
                        let n_req = TxReq {
                            req_type: req.req_type.to_owned(),
                            retry: req.retry,
                            tx_hash: req.tx_hash.to_owned(),
                        };
                        s.tx_queue.insert(n_req, tx_status);
                    }
                });

                if let ReqType::CreateToken(create_token_req) = &req.req_type {
                    update_fa_obj(&client, &create_token_req.token_id).await;
                }
                // update tx hash to hub
                if let ReqType::MintToken(mint_token_req) = &req.req_type {
                    update_tx_to_hub(&mint_token_req.ticket_id, &mint_token_req.token_id).await;
                }
                // remove req from queue
                mutate_state(|s| {
                    s.tx_queue.remove(&req);
                });
            } else {
                log!(
                    ERROR,
                    "[handler_tx::update_req_status] req: {:?},failed: {} ",
                    req.tx_hash,
                    tx.vm_status(),
                );
                // update status and retry
                mutate_state(|s| {
                    if let Some(_) = s.tx_queue.get(&req).as_mut() {
                        let tx_status = TxStatus::TxFailed { e: tx.vm_status() };
                        let n_req = TxReq {
                            req_type: req.req_type.to_owned(),
                            retry: req.retry + 1,
                            tx_hash: None,
                        };
                        s.tx_queue.insert(n_req, tx_status);
                    }
                });
            }
        }
    }
}
