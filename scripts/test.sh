#!/bin/bash

export DFX_WARNING="-mainnet_plaintext_identity"
# config network
NETWORK=local

# check route config 
dfx canister call aptos_route get_route_config '()' --network $NETWORK

# NETWORK=http://localhost:12345/
# NETWORK=ic
# get aptos_route_address and init it
KEYTYPE="variant { Native }"
# KEYTYPE="variant { ChainKey }"
# dfx canister call aptos_route aptos_route_address "($KEYTYPE)" --network $NETWORK 
aptos_route_address=$(dfx canister call aptos_route aptos_route_address "($KEYTYPE)" --network $NETWORK)
aptos_route_address=$(echo "$aptos_route_address" | awk -F'"' '{print $2}' | tr -d '[:space:]')
echo "aptos_route_address: $aptos_route_address"

# create a default profile 
aptso init 
# requrie faucet
faucet_amount=200000000
aptos account fund-with-faucet --account $aptos_route_address --amount $faucet_amount

aptos account balance --account $aptos_route_address --profile default

# get account from canister 
address=$aptos_route_address
# address="0x140549f1a4aade6333b361764d772256c962810c3f934d451e1d84481732d"
dfx canister call aptos_route get_account "(\"${address}\",null)" --network $NETWORK

address=$aptos_route_address
dfx canister call aptos_route get_account_balance "(\"${address}\",null)" --network $NETWORK

address="0x1961df628d2d224ecc91d56dfd0a4b9a545e9cf0ec9da2337c6c5c73f6171db8"
# asset_type="0x1::aptos_coin::AptosCoin"
asset_type="0x1::fungible_asset::Metadata"
dfx canister call aptos_route get_account_balance "(\"${address}\",\"${asset_type}\")" --network $NETWORK


dfx canister call aptos_route seqs '()' --network $NETWORK

dfx canister call aptos_route update_seqs '( record {next_ticket_seq=0:nat64; next_directive_seq=0:nat64; tx_seq=2:nat64})' --network $NETWORK

# dfx canister call aptos_route update_seqs '( record={0;0;0})' --network $NETWORK
# transfer apto from route to recipent
recipient="0x1961df628d2d224ecc91d56dfd0a4b9a545e9cf0ec9da2337c6c5c73f6171db8"
amount=20000000
# KEYTYPE="variant { Native }"
KEYTYPE="variant { ChainKey }"

txn_hash="0x76875d4098500de1a3179c0e4fe58227957e32f20140d7dd3364b34af42aabc9"
dfx canister call aptos_route get_transaction "(\"${txn_hash}\")" --network $NETWORK


# set route address on port
aptos_port_addr=0xeec548b9b358e769e74a7a4ba5c034fbb0c37a9872a4c3d47c8d0cacb2b3bd4f
aptos_route_address=0xcf56359a741035f960f82d3cc0454cbb228885d19b822fa6b610e450826fe097
module_id=aptos_port
# func_id=set_route_address
aptos move run --function-id $aptos_port_addr::$module_id::set_route_address --args \
address:${aptos_route_address} \
--profile omnity-devnet
# check
aptos move view --function-id $aptos_port_addr::$module_id::get_route --profile omnity-devnet

# update fee address on route
fee_account=$aptos_route_address
dfx canister call aptos_route update_fee_account "(\"${fee_account}\")" --network $NETWORK

dfx canister call aptos_route get_fee_account "(\"${fee_account}\")" --network $NETWORK

# set fee address on port 
aptos_port_addr=0xfa4ee5754e4d397f10a01ebaa75f0671d41b6e19f76e7ad98171a6956b90b722
aptos_route_address=0x908600700f676f02b7317741662358f57786d41c19f7c2d492812a047e08b807
module_id=aptos_port
# func_id=set_fee_address
aptos move run --function-id $aptos_port_addr::$module_id::set_fee_address --args \
address:${aptos_route_address} \
--profile omnity-devnet
# check 
aptos move view --function-id $aptos_port_addr::$module_id::get_fee_address --profile omnity-devnet


# update aptos port action info
port_pkg=0xeec548b9b358e769e74a7a4ba5c034fbb0c37a9872a4c3d47c8d0cacb2b3bd4f
dfx canister call aptos_route update_port_package "(\"${port_pkg}\")" --network $NETWORK

# update port info on route
port_owner=$aptos_port_addr
port_pkg=$aptos_port_addr
module=$module_id
fee_addr=$aptos_route_address
aptos_route=$aptos_route_address

# update aptos port action info to aptos route
dfx canister call aptos_route add_aptos_port "(
    record {
       port_owner = \"${port_owner}\";
       package = \"$port_pkg\";
       module = \"$module\";
       fee_addr =  \"$fee_addr\";
       aptos_route =  \"$aptos_route\";
       functions = vec { \"set_route_address\";
                         \"set_fee_address\";
                         \"create_fa\";
                         \"mint_fa_with_ticket\";
                         \"burn_fa\";
                         \"collect_fee\";
                         \"remove_ticket\";
                         \"mutate_metadata\";
                         };
    }
)" --network $NETWORK

dfx canister call aptos_route aptos_ports '()' --network $NETWORK

dfx canister call aptos_route update_key_type '(variant {Native})' --network $NETWORK
dfx canister call aptos_route query_key_type '()' --network $NETWORK


# sumbit create fa req
token_id="Bitcoin-runes-SIX•TOKEN"
token_name="Six Token"
symbol=ST
icon_uri="https://raw.githubusercontent.com/PanoraExchange/Aptos-Tokens/main/logos/APT.svg"
project_uri="https://www.omnity.network/"
# rune_id="840000:846"

dfx canister call aptos_route add_token "(record {
        token_id=\"${token_id}\";
        name=\"${token_name}\";
        symbol=\"${symbol}\";
        decimals=9:nat8;
        icon=opt \"${icon_uri}\";
        metadata = vec{ record {\"rune_id\" ; \"840000:888\"}};
})" --network $NETWORK

dfx canister call aptos_route get_token "(\"${token_id}\")" --network $NETWORK


dfx canister call aptos_route submit_tx  "(variant { CreateToken =
        record {token_id=\"${token_id}\";  
                name=\"${token_name}\"; 
                 symbol=\"${symbol}\";
                 decimals=9;
                 icon_uri=\"${icon_uri}\";
                 max_supply=null;
                 project_uri=\"${project_uri}\"
                } 
            } )" --network $NETWORK



txn_hash="0x4ad4eaa2c9dbcbb18bd270999dd5a89280a65573c6856215cd57e9669d6aed29"
dfx canister call aptos_route get_transaction "(\"${txn_hash}\")" --network $NETWORK

# query fa object id from port
aptos move view --function-id $aptos_port_addr::aptos_port::get_registry --profile omnity-devnet

# update aptos token info for "Bitcoin-runes-FOUR•TOKEN"
token_id="sICP-native-ICP"
fa_obj="0x19b1bb5f38ed05902e344d83c2ba06e5133a20b4e3a28690c2fb1c90784227f1"
type_tag="0x1::fungible_asset::FungibleAsset"

dfx canister call aptos_route update_aptos_token "(
    \"$token_id\",
    record {
       fa_obj_id = opt\"$fa_obj\";
       type_tag = opt \"$type_tag\";
    }
)" --network $NETWORK

dfx canister call aptos_route aptos_token "(\"$token_id\")" --network $NETWORK


fa_obj="0xc5bf0f66a2fbf429fc71c525bb64fff5ed150e2a4d9de38ca42321594b4318ff"
recipient=0x1961df628d2d224ecc91d56dfd0a4b9a545e9cf0ec9da2337c6c5c73f6171db8
timestamp=$(date +"%Y%m%d%H%M")
ticket_id=${token_id}-$timestamp
mint_acmount=8000000000
# KEYTYPE="variant { ChainKey }"
# KEYTYPE="variant { Native }"
dfx canister call aptos_route submit_tx  "(variant { MintToken =
        record { 
                 token_id=\"${token_id}\";
                 ticket_id=\"${ticket_id}\";
                 fa_obj=\"${fa_obj}\";
                 recipient=\"${recipient}\"; 
                 mint_acmount=${mint_acmount};
                } 
        }
    )" --network $NETWORK

txn_hash="0x76219bb06927d3403dfcd785ea6619f6816531e4f9ca2875761d6bfaa9a63bfb"
dfx canister call aptos_route get_transaction "(\"${txn_hash}\")" --network $NETWORK

# burn token
fa_obj="0xc5bf0f66a2fbf429fc71c525bb64fff5ed150e2a4d9de38ca42321594b4318ff"
burn_acmount=2000000000
# KEYTYPE="variant { ChainKey }"
KEYTYPE="variant { Native }"
dfx canister call aptos_route submit_tx  "(variant { BurnToken =
        record { 
                 fa_obj=\"${fa_obj}\";
                 burn_acmount=${burn_acmount};
                 memo=null;
                } 
        }
    )" --network $NETWORK

txn_hash="0xfe83962f7d9b2994dec037410717f2af9335aed04a596785543548448e8b93a0"
dfx canister call aptos_route get_transaction "(\"${txn_hash}\")" --network $NETWORK

# collect fee
fa_obj="0xf6350330a6fd3735ff35c6665c7ed56ac38f2c3fd48602140b7e81bc7244d49b"
fee_acmount=50000000
# KEYTYPE="variant { ChainKey }"
KEYTYPE="variant { Native }"
dfx canister call aptos_route submit_tx  "(variant { CollectFee =${fee_acmount}}
    )" --network $NETWORK

txn_hash="0xf3cceff6f3fd9c923628d6b16646b310ff3f0819b83d3d7a6a60d040e4e1965f"
dfx canister call aptos_route get_transaction "(\"${txn_hash}\")" --network $NETWORK

# remove ticket
ticket_id=-202503011901
# KEYTYPE="variant { ChainKey }"
KEYTYPE="variant { Native }"
dfx canister call aptos_route submit_tx  "(variant { RemoveTicket =\"${ticket_id}\"}
    )" --network $NETWORK

txn_hash="0x43a161ae13cbf8733b6d85da3a0acd9f032f64f6050681676269c0ecd4b2d220"
dfx canister call aptos_route get_transaction "(\"${txn_hash}\")" --network $NETWORK


# update token meta
symbol=ST2
dfx canister call aptos_route submit_tx  "(variant { UpdateMeta =
            record {
                token_id=\"${token_id}\";
                fa_obj=\"${fa_obj}\"; 
                name=null; 
                symbol=opt \"${symbol}\";
                decimals=null;
                icon_uri=null;
                project_uri=null;
             } 
            }
    )" --network $NETWORK

txn_hash="0x93449319dce1cd62b7e067166e809c6868ec7cb9656b8e5334768fd50bb4a207"
dfx canister call aptos_route get_transaction "(\"${txn_hash}\")" --network $NETWORK

# transfer apt
recipient="0x1961df628d2d224ecc91d56dfd0a4b9a545e9cf0ec9da2337c6c5c73f6171db8"
amount=20000000
dfx canister call aptos_route submit_tx  "(variant { TransferApt =
        record { 
                 recipient=\"${recipient}\"; 
                 amount=$amount;
                } 
            }
     )" --network $NETWORK


txn_hash="0xb7d8c5b55eaecb5b256e051fff7c1d218eaf633cc260aff0e02aa2298e8f7a76"
dfx canister call aptos_route get_transaction "(\"${txn_hash}\")" --network $NETWORK

aptos move view --function-id $aptos_port_addr::$module_id::get_fa_obj --args \
string:$token_id \
--profile omnity-devnet

view_func="0xa67e91bfc6ff1520ae025aa4a2c9472a2fef95a7f18fcc34941f9a8747daff2e::aptos_port::get_fa_obj"
token_id=" Bitcoin-runes-FIVE•TOKEN"
dfx canister call aptos_route get_fa_obj_from_port "(\"${view_func}\",\"${token_id}\")" --network $NETWORK

# mint token to recipient
token_id="Bitcoin-runes-HOPE•YOU•GET•RICH"
timestamp=$(date +"%Y%m%d%H%M")
ticket_id=${token_id}-$timestamp
echo ticket_id: $ticket_id
# recipient=0xaf9306cac62396be300b175046140c392eed876bd8ac0efac6301cea286fa272
# recipient=$(aptos client active-address)
recipient=$aptos_route_address
echo recipient: $recipient
amount=10000
echo mint amount: $amount

dfx canister call aptos_route mint_to_with_ticket "(
    \"$ticket_id\",
    \"$token_id\",
    \"$recipient\",
    $amount:nat64
)" --network $NETWORK 

digest="4JCVazuKaeeGhVKjCfVrPf2b23RXsEV35nvu5cSTZ53F"
dfx canister call aptos_route get_events "(\"${digest}\")" --network $NETWORK

# update fee
TARGET_CHAIN_ID=sICP
TARGET_CHAIN_FACTOR=2000
# aptos_CHAIN_ID="eaptos"
FEE_TOKEN_FACTOR=10000
aptos_FEE="aptos"

dfx canister call aptos_route update_redeem_fee "(variant { UpdateTargetChainFactor =
        record { target_chain_id=\"${TARGET_CHAIN_ID}\"; 
                 target_chain_factor=$TARGET_CHAIN_FACTOR : nat}})" --network $NETWORK
dfx canister call aptos_route update_redeem_fee "(variant { UpdateFeeTokenFactor = 
        record { fee_token=\"${aptos_FEE}\"; 
                fee_token_factor=$FEE_TOKEN_FACTOR : nat}})" --network $NETWORK

dfx canister call aptos_route get_redeem_fee "(\"${TARGET_CHAIN_ID}\")" --network $NETWORK

fee_account=$aptos_route_address
fee_amount=50000000
echo "fee account: $fee_account"
echo "fee amount: $fee_amount"


dfx canister call aptos_route get_chain_list '()' --network $NETWORK
dfx canister call aptos_route get_token_list '()' --network $NETWORK
# get events
digest=7NozueMkxV5VvLTDapBi8uynvoy6GwG9MUKvKMR7HRqj
dfx canister call aptos_route get_events "(\"${digest}\")" --network $NETWORK
dfx canister call aptos_route valid_tx_from_multi_rpc "(\"${digest}\")" --network $NETWORK


# check route config 
dfx canister call aptos_route get_route_config '()' --network $NETWORK

dfx canister call aptos_route multi_rpc_config '()' --network $NETWORK
rpc1="https://fullnode.testnet.aptos.io:443"
rpc2="https://fullnode.testnet.aptos.io:443"
rpc3="https://fullnode.testnet.aptos.io:443"
dfx canister call aptos_route update_multi_rpc "(record { 
    rpc_list = vec {\"${rpc1}\";
                     \"${rpc2}\";
                     \"${rpc3}\";};\
    minimum_response_count = 2:nat32;})" --network $NETWORK
dfx canister call aptos_route multi_rpc_config '()' --network $NETWORK

dfx canister call aptos_route start_schedule '(null)' --network $NETWORK
dfx canister call aptos_route active_tasks '()' --network $NETWORK
dfx canister call aptos_route stop_schedule '(null)' --network $NETWORK
dfx canister call aptos_route seqs '()' --network $NETWORK

dfx canister call aptos_route forward '()' --network $NETWORK
forward="https://fullnode.testnet.aptos.io:443"
forward=https://aptos.nownodes.io
dfx canister call aptos_route update_forward "(\"${forward}\")" --network $NETWORK

http_url="https://solana-rpc-proxy-398338012986.us-central1.run.app"
ws_url="wss://solana-rpc-proxy-398338012986.us-central1.run.app"
dfx canister call aptos_route rpc_provider '()' --network $NETWORK

dfx canister call aptos_route update_rpc_provider "(variant {Custom=record {
    \"${http_url}\";\"${ws_url}\"}})" --network $NETWORK

dfx canister call aptos_route rpc_provider '()' --network $NETWORK


#e2e test
