import type { Principal } from '@dfinity/principal';
import type { ActorMethod } from '@dfinity/agent';
import type { IDL } from '@dfinity/candid';

export interface AptosPortAction {
  'package' : string,
  'upgrade_cap' : string,
  'ticket_table' : string,
  'port_owner_cap' : string,
  'functions' : Array<string>,
  'module' : string,
}
export interface AptosToken {
  'treasury_cap' : string,
  'metadata' : string,
  'package' : string,
  'upgrade_cap' : string,
  'functions' : Array<string>,
  'module' : string,
  'type_tag' : string,
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
export type Result = { 'Ok' : string } |
  { 'Err' : string };
export type Result_1 = { 'Ok' : bigint } |
  { 'Err' : string };
export type Result_2 = { 'Ok' : null } |
  { 'Err' : string };
export type Result_3 = { 'Ok' : boolean } |
  { 'Err' : RpcError };
export type RouteArg = { 'Upgrade' : [] | [UpgradeArgs] } |
  { 'Init' : InitArgs };
export interface RouteConfig {
  'sui_port_action' : AptosPortAction,
  'admin' : Principal,
  'hub_principal' : Principal,
  'caller_perms' : Array<[string, Permission]>,
  'active_tasks' : Array<TaskType>,
  'gas_budget' : bigint,
  'enable_debug' : boolean,
  'fee_account' : string,
  'seqs' : Seqs,
  'rpc_provider' : Provider,
  'chain_id' : string,
  'schnorr_key_name' : string,
  'target_chain_factor' : Array<[string, bigint]>,
  'multi_rpc_config' : MultiRpcConfig,
  'key_type' : KeyType,
  'chain_state' : ChainState,
  'forward' : [] | [string],
  'nodes_in_subnet' : number,
  'fee_token_factor' : [] | [bigint],
}
export type RpcError = { 'Text' : string } |
  { 'HttpCallError' : string } |
  { 'ParseError' : string } |
  {
    'RpcResponseError' : {
      'code' : bigint,
      'data' : [] | [string],
      'message' : string,
    }
  };
export interface Seqs {
  'next_directive_seq' : bigint,
  'next_ticket_seq' : bigint,
  'tx_seq' : bigint,
}
export type SnorKeyType = { 'Native' : null } |
  { 'ChainKey' : null };
export type TaskType = { 'GetTickets' : null } |
  { 'ClearTicket' : null } |
  { 'BurnToken' : null } |
  { 'GetDirectives' : null } |
  { 'MintToken' : null } |
  { 'UpdateToken' : null };
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
  'add_token' : ActorMethod<[Token], [] | [Token]>,
  'aptos_port_info' : ActorMethod<[], AptosPortAction>,
  'aptos_route_address' : ActorMethod<[SnorKeyType], Result>,
  'aptos_token' : ActorMethod<[string], [] | [AptosToken]>,
  'forward' : ActorMethod<[], [] | [string]>,
  'get_account' : ActorMethod<[string, [] | [bigint]], Result>,
  'get_account_balance' : ActorMethod<[string, [] | [string]], Result_1>,
  'get_chain_list' : ActorMethod<[], Array<Chain>>,
  'get_fee_account' : ActorMethod<[], string>,
  'get_gas_budget' : ActorMethod<[], bigint>,
  'get_redeem_fee' : ActorMethod<[string], [] | [bigint]>,
  'get_route_config' : ActorMethod<[], RouteConfig>,
  'get_token' : ActorMethod<[string], [] | [Token]>,
  'get_token_list' : ActorMethod<[], Array<TokenResp>>,
  'get_transaction_by_hash' : ActorMethod<[string], Result>,
  'rpc_provider' : ActorMethod<[], Provider>,
  'transfer_aptos_from_route' : ActorMethod<
    [string, bigint, SnorKeyType],
    Result
  >,
  'update_aptos_port_info' : ActorMethod<[AptosPortAction], undefined>,
  'update_aptos_token' : ActorMethod<[string, AptosToken], Result_2>,
  'update_gas_budget' : ActorMethod<[bigint], undefined>,
  'update_rpc_provider' : ActorMethod<[Provider], undefined>,
  'verfy_txn' : ActorMethod<[string, bigint, SnorKeyType], Result_3>,
}
export declare const idlFactory: IDL.InterfaceFactory;
export declare const init: (args: { IDL: typeof IDL }) => IDL.Type[];
