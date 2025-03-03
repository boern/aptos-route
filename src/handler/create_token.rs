
use crate::aptos_client::rest_client::RestClient;
use crate::aptos_client::{tx_builder, CreateTokenReq, LocalAccount, TxReq};



use crate::config::read_config;
use crate::constants::{GET_FA_OBJ, RETRY_NUM};

use crate::ic_log::{DEBUG, ERROR, WARNING};
use crate::state::{mutate_state, read_state, AptosToken, TxStatus};
use ic_canister_log::log;

pub async fn create_token() {
    let creating_tokens = read_state(|s| {
        let mut creating_tokens = vec![];
        for (token_id, token) in s.tokens.iter() {
            match s.atptos_tokens.get(&token_id) {
                None => creating_tokens.push(token.to_owned()),

                //filter account,not finallized and retry < RETRY_LIMIT_SIZE
                Some(aptos_token) => {
                    if !matches!(aptos_token.status, TxStatus::Finalized)
                        && aptos_token.retry < RETRY_NUM
                    {
                        creating_tokens.push(token.to_owned())
                    }
                }
            }
        }
        creating_tokens
    });

    for token in creating_tokens.into_iter() {
        let create_token_req = CreateTokenReq {
            token_id: token.token_id.to_string(),
            name: token.name,
            symbol: token.symbol,
            decimals: token.decimals,
            icon_uri:token.icon.unwrap_or_default(),
            max_supply:None,
            project_uri:token.metadata.get("project_uri").unwrap_or(&"https://www.omnity.network".to_string()).to_owned(),
        };
        let aptos_token = if let Some(aptos_token) =
            read_state(|s| s.atptos_tokens.get(&token.token_id))
        {
            aptos_token

        } else {
            
            let new_aptos_token = AptosToken::default();
            //save inited account info
            mutate_state(|s| {
                s.atptos_tokens
                    .insert(token.token_id.to_owned(), new_aptos_token.to_owned())
            });

            // new_account
            new_aptos_token
        };

        log!(
            DEBUG,
            "[handler::create_token] token id({:}) -> aptos token: {:?} ",
            create_token_req.token_id,aptos_token,

        );

        match &aptos_token.status {
            TxStatus::New => {
                inner_create_token(&create_token_req).await;
            }
            TxStatus::Pending => {
                match &aptos_token.tx_hash {
                    // be creating
                    None => {
                        log!(
                            DEBUG,
                            "[handler::create_token] the create_token_req ({:?}) is submited, please waite ... ",
                            create_token_req
                            
                        );
                    }
                     // signature exists,but not finallized
                    Some(hash) => {
                        log!(
                            DEBUG,
                            "[handler::create_token] the  create_token_req ({:?}) already submited and waiting for the tx({:}) to be finallized ... ",
                            create_token_req,hash
                            
                        );
                        // update status
                        update_token_status(hash.to_owned(), create_token_req.token_id.to_owned()).await;
                    }
                }
            
            }
            TxStatus::Finalized => {
                log!(
                    DEBUG,
                    "[handler::create_token] token id: {:} -> aptos_token : {:?} Already finalized !",
                    token.token_id,aptos_token,
                );
            }
            TxStatus::TxFailed { e } => {
                log!(
                    DEBUG,
                    "[handler::create_token]  failed to create token for {:},error:{:}, retry ..",
                    token.token_id,e.to_string()
                );
                //retry
                inner_create_token(&create_token_req).await;

            }
        }
    }
}

pub async fn inner_create_token(create_token_req:&CreateTokenReq) {

    match  LocalAccount::local_account().await{
        Ok(mut local_account) => {
            let tx_req = TxReq::CreateToken(create_token_req.to_owned());
            if let Ok(signed_txn) = tx_builder::get_signed_tx(&mut local_account, tx_req, None)
                .await {
                    log!(
                        DEBUG,
                        "[create_token::inner_create_token] SignedTransaction: {:#?} ",
                        signed_txn
                    );
                    let client = RestClient::new();
                    match client.summit_tx(&signed_txn, &client.forward).await {
                        Ok(tx) => {
                            log!(
                                DEBUG,
                                "[create_token::inner_create_token] summit_tx ret: {:?}  ",
                                tx
                            );
                            mutate_state(|s| {
                                if let Some(aptos_token) = s.atptos_tokens
                                    .get(&create_token_req.token_id).as_mut() {
                                        //only this place ,update signature
                                        aptos_token.tx_hash = Some(tx.hash.to_string());
                                        aptos_token.status = TxStatus::Pending;
                                        // account.retry_4_building += 1;
                                        s.atptos_tokens.insert(create_token_req.token_id.to_owned(),aptos_token.to_owned());
                                    }
                                    
                            });
                        }
                        Err(tx_error) => {
                            log!(
                                ERROR,
                                "[create_token::inner_create_token] summit_tx error: {:?}  ",
                                tx_error
                            );
                   
                            // update retry
                            mutate_state(|s| {
                                if let Some(aptos_token) = s.atptos_tokens
                                    .get(&create_token_req.token_id).as_mut() {
                                        aptos_token.status = TxStatus::TxFailed { e: tx_error.to_string()};
                                        aptos_token.retry += 1;
                                        //: reset signature
                                        aptos_token.tx_hash = None;
                                        s.atptos_tokens.insert(create_token_req.token_id.to_owned(),aptos_token.to_owned());
                                    }
                            });
                        }
                    }

            } else {
                    log!(
                        DEBUG,
                        "[create_token::inner_create_token] get_signed_tx error",
                    );
                }
        }
        Err(e) => {
            log!(
                ERROR,
                "[create_token::inner_create_token] get local_account error: {:?}  ",
                e
            );
        }
    }
   
}

pub async fn update_token_status(tx_hash: String, token_id: String) {
    // query signature status
    let client = RestClient::new();
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
                 // get fa obj id from port
                 let current_package =
                 read_config(|c| c.get().current_port_package.to_owned()).expect("port package is none!");
         
                  let port_info =
                 read_state(|s| s.aptos_ports.get(&current_package)).expect("port info is none!");
 
                 let view_func = format!("{}::{}::{}",current_package,port_info.module,GET_FA_OBJ);
                 match client.get_fa_obj(view_func, token_id.to_owned(), &client.forward).await {
                     Ok(fa_obj) => {
                         if let Some(fa_obj_id) = fa_obj.first() {
                              // update account status to Finalized
                             mutate_state(|s| {
                                 if let Some(aptos_token) = s.atptos_tokens
                                 .get(&token_id).as_mut() {
                                     aptos_token.fa_obj_id=Some(fa_obj_id.to_owned());
                                     aptos_token.status = TxStatus::Finalized;
                                     s.atptos_tokens.insert(token_id.to_owned(),aptos_token.to_owned());
                                 }
                             });
                         }
                     }
                     Err(e) => {
                        log!(
                            ERROR,
                            "[create_token::update_tx_status] get_fa_obj error: {}",
                            e.to_string(),
                            
                        );
                     }
                 }
            } else {
               // update status and retry
               mutate_state(|s| {
                if let Some(aptos_token) = s.atptos_tokens
                    .get(&token_id).as_mut() {
                        aptos_token.status = TxStatus::TxFailed { e: tx.vm_status()};
                        aptos_token.retry += 1;
                        //: reset signature
                        aptos_token.tx_hash = None;
                        s.atptos_tokens.insert(token_id.to_owned(),aptos_token.to_owned());
                    }
            });
                
            }
          
        }
    }
}
