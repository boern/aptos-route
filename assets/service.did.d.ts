import type { Principal } from '@dfinity/principal';
import type { ActorMethod } from '@dfinity/agent';
import type { IDL } from '@dfinity/candid';

export interface AptosPort {
  'port_owner' : string,
  'package' : string,
  'fee_addr' : string,
  'functions' : Array<string>,
  'module' : string,
  'aptos_route' : string,
}
export interface AptosToken {
  'fa_obj_id' : [] | [string],
  'type_tag' : [] | [string],
}
export interface BurnTokenReq {
  'memo' : [] | [string],
  'fa_obj' : string,
  'burn_acmount' : bigint,
}
export interface Chain {
  'fee_token' : [] | [string],
  'canister_id' : string,
  'chain_id' : string,
  'counterparties' : [] | [Array<string>],
  'chain_state' : ChainState,
  'chain_type' : ChainType,
  'contract_address' : [] | [string],
}
export type ChainState = { 'Active' : null } |
  { 'Deactive' : null };
export type ChainType = { 'SettlementChain' : null } |
  { 'ExecutionChain' : null };
export interface CreateTokenReq {
  'decimals' : number,
  'token_id' : string,
  'project_uri' : string,
  'name' : string,
  'icon_uri' : string,
  'max_supply' : [] | [bigint],
  'symbol' : string,
}
export interface InitArgs {
  'admin' : Principal,
  'hub_principal' : Principal,
  'gas_budget' : [] | [bigint],
  'fee_account' : string,
  'rpc_provider' : [] | [Provider],
  'chain_id' : string,
  'schnorr_key_name' : [] | [string],
  'chain_state' : ChainState,
  'nodes_in_subnet' : [] | [number],
}
export type KeyType = { 'Native' : Uint8Array | number[] } |
  { 'ChainKey' : null };
export interface MintTokenReq {
  'token_id' : string,
  'recipient' : string,
  'ticket_id' : string,
  'mint_acmount' : bigint,
  'fa_obj' : string,
}
export interface MultiRpcConfig {
  'rpc_list' : Array<string>,
  'minimum_response_count' : number,
}
export type Permission = { 'Update' : null } |
  { 'Query' : null };
export type Provider = { 'Mainnet' : null } |
  { 'Custom' : [string, string] } |
  { 'Testnet' : null } |
  { 'Devnet' : null } |
  { 'Localnet' : null };
export type ReqType = { 'CreateToken' : CreateTokenReq } |
  { 'CollectFee' : bigint } |
  { 'RemoveTicket' : string } |
  { 'TransferApt' : TransferReq } |
  { 'BurnToken' : BurnTokenReq } |
  { 'MintToken' : MintTokenReq } |
  { 'UpdateMeta' : UpdateMetaReq };
export type Result = { 'Ok' : string } |
  { 'Err' : string };
export type Result_1 = { 'Ok' : Array<string> } |
  { 'Err' : string };
export type Result_2 = { 'Ok' : bigint } |
  { 'Err' : string };
export type Result_3 = { 'Ok' : null } |
  { 'Err' : string };
export type RouteArg = { 'Upgrade' : [] | [UpgradeArgs] } |
  { 'Init' : InitArgs };
export interface RouteConfig {
  'admin' : Principal,
  'hub_principal' : Principal,
  'caller_perms' : Array<[string, Permission]>,
  'active_tasks' : Array<TaskType>,
  'gas_budget' : bigint,
  'enable_debug' : boolean,
  'fee_account' : string,
  'seqs' : Seqs,
  'rpc_provider' : Provider,
  'current_port_package' : [] | [string],
  'chain_id' : string,
  'schnorr_key_name' : string,
  'target_chain_factor' : Array<[string, bigint]>,
  'multi_rpc_config' : MultiRpcConfig,
  'key_type' : KeyType,
  'chain_state' : ChainState,
  'tx_opt' : TxOptions,
  'forward' : [] | [string],
  'nodes_in_subnet' : number,
  'fee_token_factor' : [] | [bigint],
}
export interface Seqs {
  'next_directive_seq' : bigint,
  'next_ticket_seq' : bigint,
  'tx_seq' : bigint,
}
export type SnorKeyType = { 'Native' : null } |
  { 'ChainKey' : null };
export type TaskType = { 'GetTickets' : null } |
  { 'HandleTx' : null } |
  { 'GetDirectives' : null };
export interface Token {
  'decimals' : number,
  'token_id' : string,
  'metadata' : Array<[string, string]>,
  'icon' : [] | [string],
  'name' : string,
  'symbol' : string,
}
export interface TokenResp {
  'decimals' : number,
  'token_id' : string,
  'icon' : [] | [string],
  'rune_id' : [] | [string],
  'symbol' : string,
}
export interface TransferReq { 'recipient' : string, 'amount' : bigint }
export interface TxOptions {
  'max_gas_amount' : bigint,
  'chain_id' : number,
  'gas_unit_price' : bigint,
  'timeout_secs' : bigint,
}
export interface TxReq {
  'req_type' : ReqType,
  'tx_hash' : [] | [string],
  'tx_status' : TxStatus,
  'retry' : bigint,
}
export type TxStatus = { 'New' : null } |
  { 'Finalized' : null } |
  { 'TxFailed' : { 'e' : string } } |
  { 'Pending' : null };
export interface UpdateMetaReq {
  'decimals' : [] | [number],
  'token_id' : string,
  'project_uri' : [] | [string],
  'name' : [] | [string],
  'icon_uri' : [] | [string],
  'fa_obj' : string,
  'symbol' : [] | [string],
}
export interface UpgradeArgs {
  'admin' : [] | [Principal],
  'hub_principal' : [] | [Principal],
  'gas_budget' : [] | [bigint],
  'fee_account' : [] | [string],
  'rpc_provider' : [] | [Provider],
  'chain_id' : [] | [string],
  'schnorr_key_name' : [] | [string],
  'chain_state' : [] | [ChainState],
  'nodes_in_subnet' : [] | [number],
}
export interface _SERVICE {
  'add_aptos_port' : ActorMethod<[AptosPort], undefined>,
  'add_token' : ActorMethod<[Token], [] | [Token]>,
  'aptos_ports' : ActorMethod<[], Array<AptosPort>>,
  'aptos_route_address' : ActorMethod<[SnorKeyType], Result>,
  'aptos_token' : ActorMethod<[string], [] | [AptosToken]>,
  'fa_obj_from_port' : ActorMethod<[string, string], Result_1>,
  'forward' : ActorMethod<[], [] | [string]>,
  'get_account' : ActorMethod<[string, [] | [bigint]], Result>,
  'get_account_balance' : ActorMethod<[string, [] | [string]], Result_2>,
  'get_chain_list' : ActorMethod<[], Array<Chain>>,
  'get_events' : ActorMethod<[string], Result_1>,
  'get_fee_account' : ActorMethod<[], string>,
  'get_gas_budget' : ActorMethod<[], bigint>,
  'get_redeem_fee' : ActorMethod<[string], [] | [bigint]>,
  'get_route_config' : ActorMethod<[], RouteConfig>,
  'get_token' : ActorMethod<[string], [] | [Token]>,
  'get_token_list' : ActorMethod<[], Array<TokenResp>>,
  'get_transaction' : ActorMethod<[string], Result>,
  'get_tx_req' : ActorMethod<[string], [] | [TxReq]>,
  'rpc_provider' : ActorMethod<[], Provider>,
  'submit_tx' : ActorMethod<[ReqType], Result>,
  'update_aptos_token' : ActorMethod<[string, AptosToken], Result_3>,
  'update_gas_budget' : ActorMethod<[bigint], undefined>,
  'update_port_package' : ActorMethod<[string], undefined>,
  'update_rpc_provider' : ActorMethod<[Provider], undefined>,
  'update_tx_option' : ActorMethod<[TxOptions], undefined>,
}
export declare const idlFactory: IDL.InterfaceFactory;
export declare const init: (args: { IDL: typeof IDL }) => IDL.Type[];
