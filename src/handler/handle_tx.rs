use crate::aptos_client::rest_client::RestClient;
use crate::aptos_client::{tx_builder, LocalAccount, ReqType, TxReq, TxStatus};

use crate::call_error::{CallError, Reason};
use crate::config::read_config;
use crate::constants::{GET_FA_OBJ, RETRY_NUM, TAKE_SIZE};
use crate::ic_log::{DEBUG, ERROR, WARNING};
use crate::state::{mutate_state, read_state};
use ic_canister_log::log;

pub async fn handle_tx_req() {
    let reqs = read_state(|s| {
        s.tx_queue
            .iter()
            .take(TAKE_SIZE.try_into().unwrap())
            .map(|(tx_req, tx_status)| (tx_req, tx_status))
            .collect::<Vec<_>>()
    });

    for (req_id, mut req) in reqs.into_iter() {
        if req.retry >= RETRY_NUM {
            continue;
        }
        log!(
            DEBUG,
            "[handler_tx::handle_req_status] req_id: {} -> req ({:?}) already finalized!",
            req_id,
            req
        );
        match &req.tx_status {
            TxStatus::New | TxStatus::TxFailed { .. } => {
                //TODO: for the creating token req,first get fa obj id
                build_and_send_tx(&req_id, &mut req).await;
            }
            TxStatus::Pending => {
                update_tx_status(&req_id, &mut req).await;
            }
            TxStatus::Finalized => {
                log!(
                    DEBUG,
                    "[handler_tx::handle_req_status] req ({:?}) already finalized!",
                    req
                );
                // remove req from queue
                mutate_state(|s| {
                    s.tx_queue.remove(&req_id);
                });
            }
        }
    }
}

pub async fn build_and_send_tx(req_id: &String, req: &mut TxReq) {
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
                match client.summit_tx(&signed_txn).await {
                    Ok(pending_tx) => {
                        log!(
                            DEBUG,
                            "[handler_tx::build_and_send_tx] summit_tx ret: {:?}  ",
                            pending_tx
                        );
                        //update req
                        req.tx_status = TxStatus::Pending;
                        req.tx_hash = Some(pending_tx.hash.to_string());
                        mutate_state(|s| {
                            s.tx_queue.insert(req_id.to_owned(), req.to_owned());
                        });
                    }
                    Err(tx_error) => {
                        //reduce error log
                        if req.retry >= RETRY_NUM {
                            log!(
                                ERROR,
                                "[handler_tx::build_and_send_tx] summit_tx error: {:?}  ",
                                tx_error
                            );
                        } else {
                            log!(
                                WARNING,
                                "[handler_tx::build_and_send_tx] summit_tx error: {:?}  ",
                                tx_error
                            );
                        }

                        // update req status to failed and retry later
                        req.retry += 1;
                        req.tx_status = TxStatus::TxFailed {
                            e: tx_error.to_string(),
                        };
                        // req.tx_hash = None;
                        mutate_state(|s| {
                            s.tx_queue.insert(req_id.to_owned(), req.to_owned());
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

pub async fn update_tx_status(req_id: &String, req: &mut TxReq) {
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
                req.tx_status = TxStatus::Finalized;

                mutate_state(|s| {
                    s.tx_queue.insert(req_id.to_owned(), req.to_owned());
                });

                if let ReqType::CreateToken(create_token_req) = &req.req_type {
                    update_fa_obj(&client, &create_token_req.token_id).await;
                }
                // update tx hash to hub
                if let ReqType::MintToken(mint_token_req) = &req.req_type {
                    update_tx_to_hub(&mint_token_req.ticket_id, &mint_token_req.token_id).await;
                }
            } else {
                //reduce error log
                if req.retry >= RETRY_NUM {
                    log!(
                        ERROR,
                        "[handler_tx::update_req_status] req: {:?},failed: {} ",
                        req.tx_hash,
                        tx.vm_status(),
                    );
                } else {
                    log!(
                        WARNING,
                        "[handler_tx::update_req_status] req: {:?},failed: {} ",
                        req.tx_hash,
                        tx.vm_status(),
                    );
                }
                // update status and retry
                req.retry += 1;
                req.tx_status = TxStatus::TxFailed { e: tx.vm_status() };
                req.tx_hash = None;
                mutate_state(|s| {
                    s.tx_queue.insert(req_id.to_owned(), req.to_owned());
                });
            }
        }
    }
}

pub async fn update_fa_obj(client: &RestClient, token_id: &String) {
    // get fa obj id from port
    let current_package =
        read_config(|c| c.get().current_port_package.to_owned()).expect("port package is none!");

    let port_info =
        read_state(|s| s.aptos_ports.get(&current_package)).expect("port info is none!");

    let view_func = format!("{}::{}::{}", current_package, port_info.module, GET_FA_OBJ);
    match client.get_fa_obj(view_func, token_id.to_owned()).await {
        Ok(fa_obj) => {
            if let Some(fa_obj_id) = fa_obj.first() {
                log!(ERROR, "[handler_tx::update_fa_obj] fa_obj: {:?}", fa_obj);

                mutate_state(|s| {
                    if let Some(aptos_token) = s.aptos_tokens.get(token_id).as_mut() {
                        aptos_token.fa_obj_id = Some(fa_obj_id.to_owned());
                        aptos_token.type_tag =
                            Some("0x1::fungible_asset::FungibleAsset".to_string());
                        s.aptos_tokens
                            .insert(token_id.to_owned(), aptos_token.to_owned());
                    }
                });
            }
        }
        Err(e) => {
            log!(
                ERROR,
                "[handler_tx::update_fa_obj] get_fa_obj error: {}",
                e.to_string(),
            );
            //TODO,retry get fa obj
        }
    }
}

pub async fn update_tx_to_hub(ticket_id: &String, tx_hash: &String) {
    let hub_principal = read_config(|s| s.get().hub_principal);
    let tx_hash = tx_hash.to_owned();
    match ic_cdk::api::call::call(
        hub_principal,
        "update_tx_hash",
        (ticket_id.to_owned(), tx_hash.to_owned()),
    )
    .await
    .map_err(|(code, message)| CallError {
        method: "update_tx_hash".to_string(),
        reason: Reason::from_reject(code, message),
    }) {
        Ok(()) => {
            log!(DEBUG,
                        "[handler_tx::update_tx_to_hub] mint req tx({:?}) already finallized and update tx hash to hub! ",
                        tx_hash
                    );
        }
        Err(err) => {
            log!(
                ERROR,
                "[handler_tx::update_tx_to_hub] failed to update tx hash to hub:{}",
                err
            );
            //TODO: save the req into failed queue;
        }
    }
}
