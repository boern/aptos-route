#!/usr/bin/env bash

export DFX_WARNING="-mainnet_plaintext_identity"
# config network
NETWORK=local

ADMIN=$(dfx identity get-principal)
echo "admin id: $ADMIN"
echo 

# Deploy hub
# dfx canister create omnity_hub
echo deploy omnity_hub ...
dfx deploy omnity_hub --argument "(variant { Init = record { 
    admin = principal \"${ADMIN}\" 
    } })" --mode=reinstall -y --network $NETWORK
HUB_CANISTER_ID=$(dfx canister id omnity_hub --network $NETWORK )
echo "Omnity hub canister id: $HUB_CANISTER_ID"
dfx canister status omnity_hub  --network $NETWORK
echo 

# TODO: deploy customs


SCHNORR_KEY_NAME="dfx_test_key"
# SCHNORR_KEY_NAME="test_key_1"
# SCHNORR_KEY_NAME="key_1"

APTOS_CHAIN_ID="eAptos"
FEE_ACCOUNT="0x1961df628d2d224ecc91d56dfd0a4b9a545e9cf0ec9da2337c6c5c73f6171db8"
nodes_in_subnet=34
provider=Devnet
gas_budget=10000000

# echo deploy aptos_route ...
# dfx deploy aptos_route --mode=reinstall --argument "(variant { Init = record {
#     admin = principal \"${ADMIN}\";
#     chain_id = \"${APTOS_CHAIN_ID}\";
#     hub_principal = principal \"${HUB_CANISTER_ID}\";
#     chain_state= variant { Active };
#     fee_account = \"${FEE_ACCOUNT}\";
#     schnorr_key_name = opt \"${SCHNORR_KEY_NAME}\";
#     rpc_provider = opt variant { $provider };
#     nodes_in_subnet = opt ${nodes_in_subnet} : nat32;
#     gas_budget = opt $gas_budget ;
#     } 
# })"  --yes --network $NETWORK 

# dfx deploy aptos_route --argument "(variant { Init = record {
#     admin = principal \"rv3oc-smtnf-i2ert-ryxod-7uj7v-j7z3q-qfa5c-bhz35-szt3n-k3zks-fqe\";
#     chain_id = \"eSui\";
#     hub_principal = principal \"bd3sg-teaaa-aaaaa-qaaba-cai\";
#     chain_state= variant { Active };
#     schnorr_key_name = opt \"dfx_test_key\";
#     rpc_provider = opt variant { Testnet };
#     nodes_in_subnet = opt 34:nat32;
#     fee_account = \"0xaf9306cac62396be300b175046140c392eed876bd8ac0efac6301cea286fa272\";
#     gas_budget = opt 10000000:nat64
#     } 
# })" --mode=reinstall -y --network local 

dfx canister install aptos_route --argument "(variant { Init = record {
    admin = principal \"${ADMIN}\"; \
    chain_id = \"${APTOS_CHAIN_ID}\"; \
    hub_principal = principal \"${HUB_CANISTER_ID}\"; \
    chain_state= variant { Active }; \
    fee_account = \"${FEE_ACCOUNT}\"; \
    schnorr_key_name = opt \"${SCHNORR_KEY_NAME}\"; \
    rpc_provider = opt variant { ${provider} }; \
    nodes_in_subnet = opt 34; \
    gas_budget = opt 10000000 ; \
    } })" --mode=reinstall -y --wasm=./assets/aptos_route.wasm.gz --network $NETWORK

aptos_route_id=$(dfx canister id aptos_route --network $NETWORK )
echo "Sui route canister id: $aptos_route_id"
dfx canister status aptos_route --network $NETWORK  

# check route config 
dfx canister call aptos_route get_route_config '()' --network $NETWORK

# init sui route
# change log level for debugging
dfx canister call aptos_route debug '(true)' --network $NETWORK
# view log via curl or browser for http://localhost:4943
# curl http://bkyz2-fmaaa-aaaaa-qaaaq-cai.localhost:4943/logs | jq
# dfx canister call aptos_route start_schedule '(null)' 

# if required, update forward and multi_rpc_config
# forward=""
# dfx canister call aptos_route update_forward "(opt \"${forward}\")" --network $NETWORK
# dfx canister call aptos_route forward '()' --network $NETWORK
dfx canister call aptos_route multi_rpc_config '()' --network $NETWORK
rpc1="https://api.devnet.aptoslabs.com"
rpc2="https://api.devnet.aptoslabs.com"
rpc3="https://api.devnet.aptoslabs.com"
dfx canister call aptos_route update_multi_rpc "(record { 
    rpc_list = vec {\"${rpc1}\";
                     \"${rpc2}\";
                     \"${rpc3}\";};\
    minimum_response_count = 2:nat32;})" --network $NETWORK
dfx canister call aptos_route multi_rpc_config '()' --network $NETWORK


# if required, update key type
# dfx canister call aptos_route query_key_type "($KEYTYPE)" --network $NETWORK
# dfx canister call aptos_route update_key_type "($KEYTYPE)" --network $NETWORK

# if required, update provider
# provider=variant{Mainnet}
# provider="variant { record { "custom"; "url" }}"

# dfx canister call aptos_route rpc_provider '()' --network $NETWORK
# dfx canister call aptos_route update_rpc_provider "($provider)" --network $NETWORK

# if required, update fee account
# dfx canister call aptos_route get_fee_account '()' --network $NETWORK
# dfx canister call aptos_route update_fee_account "(\"${aptos_route_address}\")" --network $NETWORK

# if required, update forward
# forward="https://fullnode.testnet.sui.io:443"
# dfx canister call aptos_route forward '()' --network $NETWORK
# dfx canister call aptos_route update_forward "(opt \"${forward}\")" --network $NETWORK


echo "Deploy done!"

