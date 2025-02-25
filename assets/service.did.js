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
  const Token = IDL.Record({
    'decimals' : IDL.Nat8,
    'token_id' : IDL.Text,
    'metadata' : IDL.Vec(IDL.Tuple(IDL.Text, IDL.Text)),
    'icon' : IDL.Opt(IDL.Text),
    'name' : IDL.Text,
    'symbol' : IDL.Text,
  });
  const AptosPortAction = IDL.Record({
    'package' : IDL.Text,
    'upgrade_cap' : IDL.Text,
    'ticket_table' : IDL.Text,
    'port_owner_cap' : IDL.Text,
    'functions' : IDL.Vec(IDL.Text),
    'module' : IDL.Text,
  });
  const SnorKeyType = IDL.Variant({
    'Native' : IDL.Null,
    'ChainKey' : IDL.Null,
  });
  const Result = IDL.Variant({ 'Ok' : IDL.Text, 'Err' : IDL.Text });
  const AptosToken = IDL.Record({
    'treasury_cap' : IDL.Text,
    'metadata' : IDL.Text,
    'package' : IDL.Text,
    'upgrade_cap' : IDL.Text,
    'functions' : IDL.Vec(IDL.Text),
    'module' : IDL.Text,
    'type_tag' : IDL.Text,
  });
  const Result_1 = IDL.Variant({ 'Ok' : IDL.Nat64, 'Err' : IDL.Text });
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
    'ClearTicket' : IDL.Null,
    'BurnToken' : IDL.Null,
    'GetDirectives' : IDL.Null,
    'MintToken' : IDL.Null,
    'UpdateToken' : IDL.Null,
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
  const RouteConfig = IDL.Record({
    'sui_port_action' : AptosPortAction,
    'admin' : IDL.Principal,
    'hub_principal' : IDL.Principal,
    'caller_perms' : IDL.Vec(IDL.Tuple(IDL.Text, Permission)),
    'active_tasks' : IDL.Vec(TaskType),
    'gas_budget' : IDL.Nat64,
    'enable_debug' : IDL.Bool,
    'fee_account' : IDL.Text,
    'seqs' : Seqs,
    'rpc_provider' : Provider,
    'chain_id' : IDL.Text,
    'schnorr_key_name' : IDL.Text,
    'target_chain_factor' : IDL.Vec(IDL.Tuple(IDL.Text, IDL.Nat)),
    'multi_rpc_config' : MultiRpcConfig,
    'key_type' : KeyType,
    'chain_state' : ChainState,
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
  const Result_2 = IDL.Variant({ 'Ok' : IDL.Null, 'Err' : IDL.Text });
  const RpcError = IDL.Variant({
    'Text' : IDL.Text,
    'HttpCallError' : IDL.Text,
    'ParseError' : IDL.Text,
    'RpcResponseError' : IDL.Record({
      'code' : IDL.Int64,
      'data' : IDL.Opt(IDL.Text),
      'message' : IDL.Text,
    }),
  });
  const Result_3 = IDL.Variant({ 'Ok' : IDL.Bool, 'Err' : RpcError });
  return IDL.Service({
    'add_token' : IDL.Func([Token], [IDL.Opt(Token)], []),
    'aptos_port_info' : IDL.Func([], [AptosPortAction], ['query']),
    'aptos_route_address' : IDL.Func([SnorKeyType], [Result], []),
    'aptos_token' : IDL.Func([IDL.Text], [IDL.Opt(AptosToken)], ['query']),
    'forward' : IDL.Func([], [IDL.Opt(IDL.Text)], ['query']),
    'get_account' : IDL.Func([IDL.Text, IDL.Opt(IDL.Nat64)], [Result], []),
    'get_account_balance' : IDL.Func(
        [IDL.Text, IDL.Opt(IDL.Text)],
        [Result_1],
        [],
      ),
    'get_chain_list' : IDL.Func([], [IDL.Vec(Chain)], ['query']),
    'get_fee_account' : IDL.Func([], [IDL.Text], ['query']),
    'get_gas_budget' : IDL.Func([], [IDL.Nat64], []),
    'get_redeem_fee' : IDL.Func([IDL.Text], [IDL.Opt(IDL.Nat)], ['query']),
    'get_route_config' : IDL.Func([], [RouteConfig], ['query']),
    'get_token' : IDL.Func([IDL.Text], [IDL.Opt(Token)], ['query']),
    'get_token_list' : IDL.Func([], [IDL.Vec(TokenResp)], ['query']),
    'get_transaction_by_hash' : IDL.Func([IDL.Text], [Result], []),
    'rpc_provider' : IDL.Func([], [Provider], ['query']),
    'transfer_aptos_from_route' : IDL.Func(
        [IDL.Text, IDL.Nat64, SnorKeyType],
        [Result],
        [],
      ),
    'update_aptos_port_info' : IDL.Func([AptosPortAction], [], []),
    'update_aptos_token' : IDL.Func([IDL.Text, AptosToken], [Result_2], []),
    'update_gas_budget' : IDL.Func([IDL.Nat64], [], []),
    'update_rpc_provider' : IDL.Func([Provider], [], []),
    'verfy_txn' : IDL.Func([IDL.Text, IDL.Nat64, SnorKeyType], [Result_3], []),
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
