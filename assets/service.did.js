export const idlFactory = ({ IDL }) => {
  const Provider = IDL.Variant({
    'Mainnet' : IDL.Null,
    'Custom' : IDL.Tuple(IDL.Text, IDL.Text),
    'Testnet' : IDL.Null,
    'Devnet' : IDL.Null,
    'Localnet' : IDL.Null,
  });
  const ChainState = IDL.Variant({
    'Active' : IDL.Null,
    'Deactive' : IDL.Null,
  });
  const UpgradeArgs = IDL.Record({
    'admin' : IDL.Opt(IDL.Principal),
    'hub_principal' : IDL.Opt(IDL.Principal),
    'gas_budget' : IDL.Opt(IDL.Nat64),
    'fee_account' : IDL.Opt(IDL.Text),
    'rpc_provider' : IDL.Opt(Provider),
    'chain_id' : IDL.Opt(IDL.Text),
    'schnorr_key_name' : IDL.Opt(IDL.Text),
    'chain_state' : IDL.Opt(ChainState),
    'nodes_in_subnet' : IDL.Opt(IDL.Nat32),
  });
  const InitArgs = IDL.Record({
    'admin' : IDL.Principal,
    'hub_principal' : IDL.Principal,
    'gas_budget' : IDL.Opt(IDL.Nat64),
    'fee_account' : IDL.Text,
    'rpc_provider' : IDL.Opt(Provider),
    'chain_id' : IDL.Text,
    'schnorr_key_name' : IDL.Opt(IDL.Text),
    'chain_state' : ChainState,
    'nodes_in_subnet' : IDL.Opt(IDL.Nat32),
  });
  const RouteArg = IDL.Variant({
    'Upgrade' : IDL.Opt(UpgradeArgs),
    'Init' : InitArgs,
  });
  const AptosPort = IDL.Record({
    'port_owner' : IDL.Text,
    'package' : IDL.Text,
    'fee_addr' : IDL.Text,
    'functions' : IDL.Vec(IDL.Text),
    'module' : IDL.Text,
    'aptos_route' : IDL.Text,
  });
  const Token = IDL.Record({
    'decimals' : IDL.Nat8,
    'token_id' : IDL.Text,
    'metadata' : IDL.Vec(IDL.Tuple(IDL.Text, IDL.Text)),
    'icon' : IDL.Opt(IDL.Text),
    'name' : IDL.Text,
    'symbol' : IDL.Text,
  });
  const SnorKeyType = IDL.Variant({
    'Native' : IDL.Null,
    'ChainKey' : IDL.Null,
  });
  const Result = IDL.Variant({ 'Ok' : IDL.Text, 'Err' : IDL.Text });
  const AptosToken = IDL.Record({
    'fa_obj_id' : IDL.Opt(IDL.Text),
    'type_tag' : IDL.Opt(IDL.Text),
  });
  const Result_1 = IDL.Variant({ 'Ok' : IDL.Vec(IDL.Text), 'Err' : IDL.Text });
  const Result_2 = IDL.Variant({ 'Ok' : IDL.Nat64, 'Err' : IDL.Text });
  const ChainType = IDL.Variant({
    'SettlementChain' : IDL.Null,
    'ExecutionChain' : IDL.Null,
  });
  const Chain = IDL.Record({
    'fee_token' : IDL.Opt(IDL.Text),
    'canister_id' : IDL.Text,
    'chain_id' : IDL.Text,
    'counterparties' : IDL.Opt(IDL.Vec(IDL.Text)),
    'chain_state' : ChainState,
    'chain_type' : ChainType,
    'contract_address' : IDL.Opt(IDL.Text),
  });
  const Permission = IDL.Variant({ 'Update' : IDL.Null, 'Query' : IDL.Null });
  const TaskType = IDL.Variant({
    'GetTickets' : IDL.Null,
    'HandleTx' : IDL.Null,
    'GetDirectives' : IDL.Null,
  });
  const Seqs = IDL.Record({
    'next_directive_seq' : IDL.Nat64,
    'next_ticket_seq' : IDL.Nat64,
    'tx_seq' : IDL.Nat64,
  });
  const MultiRpcConfig = IDL.Record({
    'rpc_list' : IDL.Vec(IDL.Text),
    'minimum_response_count' : IDL.Nat32,
  });
  const KeyType = IDL.Variant({
    'Native' : IDL.Vec(IDL.Nat8),
    'ChainKey' : IDL.Null,
  });
  const TxOptions = IDL.Record({
    'max_gas_amount' : IDL.Nat64,
    'chain_id' : IDL.Nat8,
    'gas_unit_price' : IDL.Nat64,
    'timeout_secs' : IDL.Nat64,
  });
  const RouteConfig = IDL.Record({
    'admin' : IDL.Principal,
    'hub_principal' : IDL.Principal,
    'caller_perms' : IDL.Vec(IDL.Tuple(IDL.Text, Permission)),
    'active_tasks' : IDL.Vec(TaskType),
    'gas_budget' : IDL.Nat64,
    'enable_debug' : IDL.Bool,
    'fee_account' : IDL.Text,
    'seqs' : Seqs,
    'rpc_provider' : Provider,
    'current_port_package' : IDL.Opt(IDL.Text),
    'chain_id' : IDL.Text,
    'schnorr_key_name' : IDL.Text,
    'target_chain_factor' : IDL.Vec(IDL.Tuple(IDL.Text, IDL.Nat)),
    'multi_rpc_config' : MultiRpcConfig,
    'key_type' : KeyType,
    'chain_state' : ChainState,
    'tx_opt' : TxOptions,
    'forward' : IDL.Opt(IDL.Text),
    'nodes_in_subnet' : IDL.Nat32,
    'fee_token_factor' : IDL.Opt(IDL.Nat),
  });
  const TokenResp = IDL.Record({
    'decimals' : IDL.Nat8,
    'token_id' : IDL.Text,
    'icon' : IDL.Opt(IDL.Text),
    'rune_id' : IDL.Opt(IDL.Text),
    'symbol' : IDL.Text,
  });
  const CreateTokenReq = IDL.Record({
    'decimals' : IDL.Nat8,
    'token_id' : IDL.Text,
    'project_uri' : IDL.Text,
    'name' : IDL.Text,
    'icon_uri' : IDL.Text,
    'max_supply' : IDL.Opt(IDL.Nat),
    'symbol' : IDL.Text,
  });
  const TransferReq = IDL.Record({
    'recipient' : IDL.Text,
    'amount' : IDL.Nat64,
  });
  const BurnTokenReq = IDL.Record({
    'memo' : IDL.Opt(IDL.Text),
    'fa_obj' : IDL.Text,
    'burn_acmount' : IDL.Nat64,
  });
  const MintTokenReq = IDL.Record({
    'token_id' : IDL.Text,
    'recipient' : IDL.Text,
    'ticket_id' : IDL.Text,
    'mint_acmount' : IDL.Nat64,
    'fa_obj' : IDL.Text,
  });
  const UpdateMetaReq = IDL.Record({
    'decimals' : IDL.Opt(IDL.Nat8),
    'token_id' : IDL.Text,
    'project_uri' : IDL.Opt(IDL.Text),
    'name' : IDL.Opt(IDL.Text),
    'icon_uri' : IDL.Opt(IDL.Text),
    'fa_obj' : IDL.Text,
    'symbol' : IDL.Opt(IDL.Text),
  });
  const ReqType = IDL.Variant({
    'CreateToken' : CreateTokenReq,
    'CollectFee' : IDL.Nat64,
    'RemoveTicket' : IDL.Text,
    'TransferApt' : TransferReq,
    'BurnToken' : BurnTokenReq,
    'MintToken' : MintTokenReq,
    'UpdateMeta' : UpdateMetaReq,
  });
  const TxStatus = IDL.Variant({
    'New' : IDL.Null,
    'Finalized' : IDL.Null,
    'TxFailed' : IDL.Record({ 'e' : IDL.Text }),
    'Pending' : IDL.Null,
  });
  const TxReq = IDL.Record({
    'req_type' : ReqType,
    'tx_hash' : IDL.Opt(IDL.Text),
    'tx_status' : TxStatus,
    'retry' : IDL.Nat64,
  });
  const Result_3 = IDL.Variant({ 'Ok' : IDL.Null, 'Err' : IDL.Text });
  return IDL.Service({
    'add_aptos_port' : IDL.Func([AptosPort], [], []),
    'add_token' : IDL.Func([Token], [IDL.Opt(Token)], []),
    'aptos_ports' : IDL.Func([], [IDL.Vec(AptosPort)], ['query']),
    'aptos_route_address' : IDL.Func([SnorKeyType], [Result], []),
    'aptos_token' : IDL.Func([IDL.Text], [IDL.Opt(AptosToken)], ['query']),
    'fa_obj_from_port' : IDL.Func([IDL.Text, IDL.Text], [Result_1], []),
    'forward' : IDL.Func([], [IDL.Opt(IDL.Text)], ['query']),
    'get_account' : IDL.Func([IDL.Text, IDL.Opt(IDL.Nat64)], [Result], []),
    'get_account_balance' : IDL.Func(
        [IDL.Text, IDL.Opt(IDL.Text)],
        [Result_2],
        [],
      ),
    'get_chain_list' : IDL.Func([], [IDL.Vec(Chain)], ['query']),
    'get_events' : IDL.Func([IDL.Text], [Result_1], []),
    'get_fee_account' : IDL.Func([], [IDL.Text], ['query']),
    'get_gas_budget' : IDL.Func([], [IDL.Nat64], []),
    'get_redeem_fee' : IDL.Func([IDL.Text], [IDL.Opt(IDL.Nat)], ['query']),
    'get_route_config' : IDL.Func([], [RouteConfig], ['query']),
    'get_token' : IDL.Func([IDL.Text], [IDL.Opt(Token)], ['query']),
    'get_token_list' : IDL.Func([], [IDL.Vec(TokenResp)], ['query']),
    'get_transaction' : IDL.Func([IDL.Text], [Result], []),
    'get_tx_req' : IDL.Func([IDL.Text], [IDL.Opt(TxReq)], ['query']),
    'rpc_provider' : IDL.Func([], [Provider], ['query']),
    'submit_tx' : IDL.Func([ReqType], [Result], []),
    'update_aptos_token' : IDL.Func([IDL.Text, AptosToken], [Result_3], []),
    'update_gas_budget' : IDL.Func([IDL.Nat64], [], []),
    'update_port_package' : IDL.Func([IDL.Text], [], []),
    'update_rpc_provider' : IDL.Func([Provider], [], []),
    'update_tx_option' : IDL.Func([TxOptions], [], []),
  });
};
export const init = ({ IDL }) => {
  const Provider = IDL.Variant({
    'Mainnet' : IDL.Null,
    'Custom' : IDL.Tuple(IDL.Text, IDL.Text),
    'Testnet' : IDL.Null,
    'Devnet' : IDL.Null,
    'Localnet' : IDL.Null,
  });
  const ChainState = IDL.Variant({
    'Active' : IDL.Null,
    'Deactive' : IDL.Null,
  });
  const UpgradeArgs = IDL.Record({
    'admin' : IDL.Opt(IDL.Principal),
    'hub_principal' : IDL.Opt(IDL.Principal),
    'gas_budget' : IDL.Opt(IDL.Nat64),
    'fee_account' : IDL.Opt(IDL.Text),
    'rpc_provider' : IDL.Opt(Provider),
    'chain_id' : IDL.Opt(IDL.Text),
    'schnorr_key_name' : IDL.Opt(IDL.Text),
    'chain_state' : IDL.Opt(ChainState),
    'nodes_in_subnet' : IDL.Opt(IDL.Nat32),
  });
  const InitArgs = IDL.Record({
    'admin' : IDL.Principal,
    'hub_principal' : IDL.Principal,
    'gas_budget' : IDL.Opt(IDL.Nat64),
    'fee_account' : IDL.Text,
    'rpc_provider' : IDL.Opt(Provider),
    'chain_id' : IDL.Text,
    'schnorr_key_name' : IDL.Opt(IDL.Text),
    'chain_state' : ChainState,
    'nodes_in_subnet' : IDL.Opt(IDL.Nat32),
  });
  const RouteArg = IDL.Variant({
    'Upgrade' : IDL.Opt(UpgradeArgs),
    'Init' : InitArgs,
  });
  return [RouteArg];
};
