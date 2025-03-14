#![allow(unused)]
use super::fecth_directive;
use crate::constants::CLEAR_INTERVAL;

use crate::constants::HANDLE_TX_INTERVAL;
use crate::handler::fetch_ticket;
use crate::handler::handle_tx;
// use crate::handler::update_token;

use crate::config::mutate_config;
use crate::{
    constants::{
        MINT_TOKEN_INTERVAL, QUERY_DERECTIVE_INTERVAL, QUERY_TICKET_INTERVAL, UPDATE_TOKEN_INTERVAL,
    },
    guard::{TaskType, TimerGuard},
};
use ic_canister_log::log;
use ic_cdk_timers::TimerId;

use crate::ic_log::{DEBUG, WARNING};
use std::cell::RefCell;
use std::collections::HashMap;
thread_local! {
    static TIMER_GUARD: RefCell<HashMap<TaskType,TimerId>> = RefCell::new(HashMap::default());
}

pub fn start_schedule(tasks: Option<Vec<TaskType>>) {
    match tasks {
        None => {
            fetch_directive_task();
            // update_token_meta_task();
            fetch_tickets_task();
            // mint_token_task();
            handle_tx_task();
        }
        Some(tasks) => {
            for task in tasks {
                match task {
                    TaskType::GetDirectives => fetch_directive_task(),
                    // TaskType::UpdateToken => update_token_meta_task(),
                    TaskType::GetTickets => fetch_tickets_task(),
                    // TaskType::MintToken => mint_token_task(),
                    TaskType::HandleTx => handle_tx_task(),
                }
            }
        }
    }
}

// clear the running tasks
pub fn stop_schedule(tasks: Option<Vec<TaskType>>) {
    match tasks {
        Some(t) => t.iter().for_each(|t| {
            TIMER_GUARD.with_borrow_mut(|guard| {
                guard
                    .get(&t)
                    .map(|task_id| ic_cdk_timers::clear_timer(*task_id));
                guard.remove(t)
            });
            mutate_config(|s| {
                let mut config = s.get().to_owned();
                config.active_tasks.remove(t);
                s.set(config);
            });
        }),
        None => {
            TIMER_GUARD.with_borrow_mut(|guard| {
                guard
                    .iter()
                    .for_each(|(_task_type, task_id)| ic_cdk_timers::clear_timer(*task_id));
                guard.clear()
            });

            mutate_config(|s| {
                let mut config = s.get().to_owned();
                config.active_tasks.clear();
                s.set(config);
            });
        }
    }
}

fn fetch_tickets_task() {
    // query_tickets task
    let query_ticket_timer_id = ic_cdk_timers::set_timer_interval(QUERY_TICKET_INTERVAL, || {
        ic_cdk::spawn(async {
            let _guard = match TimerGuard::new(TaskType::GetTickets) {
                Ok(guard) => guard,
                Err(e) => {
                    log!(WARNING, "TaskType::GetTickets error : {:?}", e);
                    return;
                }
            };

            fetch_ticket::query_tickets().await;
        });
    });
    log!(DEBUG, "GetTickets task id : {:?}", query_ticket_timer_id);
    TIMER_GUARD.with_borrow_mut(|guard| {
        guard.insert(TaskType::GetTickets, query_ticket_timer_id);
    });
}

fn fetch_directive_task() {
    // query_directives task
    let directive_timer_id = ic_cdk_timers::set_timer_interval(QUERY_DERECTIVE_INTERVAL, || {
        ic_cdk::spawn(async {
            let _guard = match TimerGuard::new(TaskType::GetDirectives) {
                Ok(guard) => guard,
                Err(e) => {
                    log!(WARNING, "TaskType::GetDirectives error : {:?}", e);
                    return;
                }
            };
            fecth_directive::query_directives().await;
        });
    });
    log!(DEBUG, "GetDirectives task id : {:?}", directive_timer_id);
    TIMER_GUARD.with_borrow_mut(|guard| {
        guard.insert(TaskType::GetDirectives, directive_timer_id);
    });
}

fn handle_tx_task() {
    // handle to update token metadata
    let update_token_timer_id = ic_cdk_timers::set_timer_interval(HANDLE_TX_INTERVAL, || {
        ic_cdk::spawn(async {
            let _guard = match TimerGuard::new(TaskType::HandleTx) {
                Ok(guard) => guard,
                Err(e) => {
                    log!(WARNING, "TaskType::HandleTx error : {:?}", e);
                    return;
                }
            };
            handle_tx::handle_tx_req().await;
        });
    });
    log!(DEBUG, "HandleTx task id: {:?}", update_token_timer_id);
    TIMER_GUARD.with_borrow_mut(|guard| {
        guard.insert(TaskType::HandleTx, update_token_timer_id);
    });
}
