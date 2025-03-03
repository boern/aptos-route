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
dfx canister call aptos_route verfy_txn "(\"${recipient}\",${amount},$KEYTYPE)" --network $NETWORK

dfx canister call aptos_route transfer_aptos_from_route "(\"${recipient}\",${amount},${KEYTYPE})" --network $NETWORK

txn_hash="0x169238641c3f97f2bc0b4a46707faf12457de857015f0882c6b2635e17486e4a"
dfx canister call aptos_route get_transaction_by_hash "(\"${txn_hash}\")" --network $NETWORK


# set route address on port
aptos_port_addr=0x933707720e4def37d4a69c0f83366a6c5bc7fc6309891a60d4013777cdb81fc7
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
aptos_port_addr=0x933707720e4def37d4a69c0f83366a6c5bc7fc6309891a60d4013777cdb81fc7
aptos_route_address=0xcf56359a741035f960f82d3cc0454cbb228885d19b822fa6b610e450826fe097
module_id=aptos_port
# func_id=set_fee_address
aptos move run --function-id $aptos_port_addr::$module_id::set_fee_address --args \
address:${aptos_route_address} \
--profile omnity-devnet
# check 
aptos move view --function-id $aptos_port_addr::$module_id::get_fee_address --profile omnity-devnet


# update aptos port action info
port_pkg=0x933707720e4def37d4a69c0f83366a6c5bc7fc6309891a60d4013777cdb81fc7
dfx canister call aptos_route update_port_package "(\"${port_pkg}\")" --network $NETWORK

# update port info on route

port_owner=$aptos_port_addr
port_pkg=$aptos_port_addr
module=$module_id
fee_addr=$aptos_route_address
aptos_route=$aptos_route_address

# update aptos port action info to aptos route
dfx canister call aptos_route update_aptos_ports "(
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


# sumbit create fa req
token_id="Bitcoin-runes-FOUR•TOKEN"
token_name="Four Token"
symbol=FT
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
        record { name=\"${token_name}\"; 
                 symbol=\"${symbol}\";
                 decimals=9;
                 icon_uri=\"${icon_uri}\";
                 max_supply=null;
                 project_uri=\"${project_uri}\"
                } 
            } )" --network $NETWORK



txn_hash="0x49eddca2fb0e4f4682502ee6e3bd0a2b97ee6b382af9793c5030674ec8ce514a"
dfx canister call aptos_route get_transaction_by_hash "(\"${txn_hash}\")" --network $NETWORK

# query fa object id from port
aptos move view --function-id $aptos_port_addr::aptos_port::get_registry --profile omnity-devnet

# update aptos token info for "Bitcoin-runes-FOUR•TOKEN"
token_id="Bitcoin-runes-FOUR•TOKEN"
fa_obj="0xf6350330a6fd3735ff35c6665c7ed56ac38f2c3fd48602140b7e81bc7244d49b"
type_tag="0x1::fungible_asset::Metadata"

dfx canister call aptos_route update_aptos_token "(
    \"$token_id\",
    record {
       object_id = \"$fa_obj\";
       type_tag = \"$type_tag\";
    }
)" --network $NETWORK

dfx canister call aptos_route aptos_token "(\"$token_id\")" --network $NETWORK


# mint token
# aptos move run --function-id $aptos_port_addr::aptos_port::mint_fa_with_ticket --args \
#  "string:ticket-2" \
#  "address:0xdb0662d8cd74ac3539888d40c9d11411034758efdc1c7c286a801ad0324dc34e" \
#  "address:0x1961df628d2d224ecc91d56dfd0a4b9a545e9cf0ec9da2337c6c5c73f6171db8" \
#  "u64:888888"  \
#  --profile omnity-devnet
fa_obj="0xf6350330a6fd3735ff35c6665c7ed56ac38f2c3fd48602140b7e81bc7244d49b"
recipient=0x1961df628d2d224ecc91d56dfd0a4b9a545e9cf0ec9da2337c6c5c73f6171db8
timestamp=$(date +"%Y%m%d%H%M")
ticket_id=${token_id}-$timestamp
mint_acmount=8000000000
# KEYTYPE="variant { ChainKey }"
KEYTYPE="variant { Native }"
dfx canister call aptos_route submit_tx  "(variant { MintToken =
        record { 
                 ticket_id=\"${ticket_id}\";
                 fa_obj=\"${fa_obj}\";
                 recipient=\"${recipient}\"; 
                 mint_acmount=${mint_acmount};
                } 
        },
        $KEYTYPE
    )" --network $NETWORK

txn_hash="0x70587dee9d53d2d02b674f106616f59cd4215ae9023d0ef272c274ea6ba6b297"
dfx canister call aptos_route get_transaction_by_hash "(\"${txn_hash}\")" --network $NETWORK

# burn token
fa_obj="0xf6350330a6fd3735ff35c6665c7ed56ac38f2c3fd48602140b7e81bc7244d49b"
burn_acmount=2000000000
# KEYTYPE="variant { ChainKey }"
KEYTYPE="variant { Native }"
dfx canister call aptos_route submit_tx  "(variant { BurnToken =
        record { 
                 fa_obj=\"${fa_obj}\";
                 burn_acmount=${burn_acmount};
                } 
        },
        $KEYTYPE
    )" --network $NETWORK

txn_hash="0xfe83962f7d9b2994dec037410717f2af9335aed04a596785543548448e8b93a0"
dfx canister call aptos_route get_transaction_by_hash "(\"${txn_hash}\")" --network $NETWORK

# collect fee
fa_obj="0xf6350330a6fd3735ff35c6665c7ed56ac38f2c3fd48602140b7e81bc7244d49b"
fee_acmount=50000000
# KEYTYPE="variant { ChainKey }"
KEYTYPE="variant { Native }"
dfx canister call aptos_route submit_tx  "(variant { CollectFee =${fee_acmount}},
        $KEYTYPE
    )" --network $NETWORK

txn_hash="0xf3cceff6f3fd9c923628d6b16646b310ff3f0819b83d3d7a6a60d040e4e1965f"
dfx canister call aptos_route get_transaction_by_hash "(\"${txn_hash}\")" --network $NETWORK

# remove ticket
ticket_id=-202503011901
# KEYTYPE="variant { ChainKey }"
KEYTYPE="variant { Native }"
dfx canister call aptos_route submit_tx  "(variant { RemoveTicket =\"${ticket_id}\"},
        $KEYTYPE
    )" --network $NETWORK

txn_hash="0x43a161ae13cbf8733b6d85da3a0acd9f032f64f6050681676269c0ecd4b2d220"
dfx canister call aptos_route get_transaction_by_hash "(\"${txn_hash}\")" --network $NETWORK


# update token meta
fa_obj="0xf6350330a6fd3735ff35c6665c7ed56ac38f2c3fd48602140b7e81bc7244d49b"
token_id="Bitcoin-runes-FOUR•TOKEN"
token_name="Four Token"
symbol=FT
icon_uri="https://raw.githubusercontent.com/PanoraExchange/Aptos-Tokens/main/logos/APT.svg"
project_uri="https://www.omnity.network/"
KEYTYPE="variant { Native }"
dfx canister call aptos_route add_token "(record {
        token_id=\"${token_id}\";
        name=\"${token_name}\";
        symbol=\"${symbol}\";
        decimals=9:nat8;
        icon=opt \"${icon_uri}\";
        metadata = vec{ record {\"rune_id\" ; \"840000:888\"}};
})" --network $NETWORK

dfx canister call aptos_route get_token "(\"${token_id}\")" --network $NETWORK


dfx canister call aptos_route submit_tx  "(variant { UpdateMeta =
            record {
                 fa_obj=\"${fa_obj}\"; 
                 name=null; 
                 symbol=null;
                 decimals=null;
                 icon_uri=opt \"${icon_uri}\";
                 project_uri=null;
                } 
            },
            $KEYTYPE
    )" --network $NETWORK

txn_hash="0x93449319dce1cd62b7e067166e809c6868ec7cb9656b8e5334768fd50bb4a207"
dfx canister call aptos_route get_transaction_by_hash "(\"${txn_hash}\")" --network $NETWORK

token_id="Bitcoin-runes-FIVE•TOKEN"
aptos move run --function-id $aptos_port_addr::aptos_port::create_fa_v2 --args \
"string: Bitcoin-runes-FIVE•TOKEN" \
"string: Five Token" \
"string: FT" \
u8:5 \
"string:https://raw.githubusercontent.com/PanoraExchange/Aptos-Tokens/main/logos/APT.svg" \
"string:" \
u8:[]  \
--profile omnity-devnet

token_id="Bitcoin-runes-FIVE•TOKEN"
token_name="FIVE•TOKEN Token"
symbol=FT
icon_uri="https://raw.githubusercontent.com/PanoraExchange/Aptos-Tokens/main/logos/APT.svg"
project_uri="https://www.omnity.network/"
# rune_id="840000:846"

dfx canister call aptos_route add_token "(record {
        token_id=\"${token_id}\";
        name=\"${token_name}\";
        symbol=\"${symbol}\";
        decimals=5:nat8;
        icon=opt \"${icon_uri}\";
        metadata = vec{ record {\"rune_id\" ; \"840000:555\"}};
})" --network $NETWORK

dfx canister call aptos_route get_token "(\"${token_id}\")" --network $NETWORK


dfx canister call aptos_route submit_tx  "(variant { CreateTokenV2 =
        record { 
                 token_id=\"${token_id}\"; 
                 name=\"${token_name}\"; 
                 symbol=\"${symbol}\";
                 decimals=5;
                 icon_uri=\"${icon_uri}\";
                 max_supply=null;
                 project_uri=\"${project_uri}\"
                } 
            },
        $KEYTYPE
     )" --network $NETWORK


dfx canister call aptos_route submit_tx  "(variant { CreateToken =
        record { 
                 name=\"${token_name}\"; 
                 symbol=\"${symbol}\";
                 decimals=5;
                 icon_uri=\"${icon_uri}\";
                 max_supply=null;
                 project_uri=\"${project_uri}\"
                } 
            },
        $KEYTYPE
     )" --network $NETWORK


txn_hash="0xb7d8c5b55eaecb5b256e051fff7c1d218eaf633cc260aff0e02aa2298e8f7a76"
dfx canister call aptos_route get_transaction_by_hash "(\"${txn_hash}\")" --network $NETWORK

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

# burn token via aptos route
# first split and transfer the burned coin to aptos route 
# obj_id=0xb2c28ea3fcedf0949530c6ab5b525ec72a8f997dc8ffa0a17fac46278de26478
# aptos client object $obj_id
# to=$aptos_route_address
# aptos client transfer --to $to --object-id $obj_id
aptos client objects $aptos_route_address
obj_id=0xc844a514ad21e4c12ac22b914650c38b6238040d643f089d435e9a6330faf28f
aptos client object $obj_id
# execute burn token
dfx canister call aptos_route burn_token "(
    \"$token_id\",
    \"$obj_id\",
)" --network $NETWORK 


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

# call collet_fee
func=collect_fee
aptos client ptb \
  --assign fee_amount $fee_amount \
  --assign recipient @$fee_account \
  --split-coins gas [fee_amount] \
  --assign fee_coins \
  --move-call $package::$module::$func fee_coins.0 recipient \
  --gas-budget 100000000 \
  --dry-run \
  --preview

# call redeem
target_chain_id=Bitcoin
target_chain_id=$(printf '%s' "$target_chain_id" | od -An -v -tuC -w1 | awk '{$1=$1;print}' | tr '\n' ',' | sed 's/,$//')
target_chain_id="[${target_chain_id}]"
echo "target_chain_id bytes: $target_chain_id"
token_id="Bitcoin-runes-APPLE•PIE"
token_id=$(printf '%s' "$token_id" | od -An -v -tuC -w1 | awk '{$1=$1;print}' | tr '\n' ',' | sed 's/,$//')
token_id="[${token_id}]"
echo "token_id bytes: $token_id"
burn_token_obj=0x145756516a5795b00bdebd531f81b42823ea89b0a281bea0b3544ff7b5159f4d
echo "burn token object id: $burn_token_obj"
receiver=bc1qmh0chcr9f73a3ynt90k0w8qsqlydr4a6espnj6
receiver=$(printf '%s' "$receiver" | od -An -v -tuC -w1 | awk '{$1=$1;print}' | tr '\n' ',' | sed 's/,$//')
receiver="[${receiver}]"
echo "recevier bytes: $receiver"
memo="This ticket is redeemed from aptos to Bitcoin"
memo=$(printf '%s' "$memo" | od -An -v -tuC -w1 | awk '{$1=$1;print}' | tr '\n' ',' | sed 's/,$//')
memo="[${memo}]"
echo "memo bytes: $memo"
route_address=$aptos_route_address
echo "aptos route address:$route_address"
redeem_amount=50000000
echo "redeem amount: $redeem_amount"


dfx canister call aptos_route get_chain_list '()' --network $NETWORK
dfx canister call aptos_route get_token_list '()' --network $NETWORK
# get events
digest=7NozueMkxV5VvLTDapBi8uynvoy6GwG9MUKvKMR7HRqj
dfx canister call aptos_route get_events "(\"${digest}\")" --network $NETWORK
dfx canister call aptos_route valid_tx_from_multi_rpc "(\"${digest}\")" --network $NETWORK

# update coin meta
token_id="Bitcoin-runes-APPLE•PIE"
# update symbole
symbol=PIE
dfx canister call aptos_route update_token_meta "(
    \"$token_id\",
    variant {Symbol=\"$symbol\"})"
# update name
name=APPLE•PIE
dfx canister call aptos_route update_token_meta "(
    \"$token_id\",
    variant {Name=\"$name\"})"
# update icon
icon=https://arweave.net/tTTr14osgHDC2jBcvIM5FHi1H8kuUmQh4Tlknr5pG7U
dfx canister call aptos_route update_token_meta "(
    \"$token_id\",
    variant {Icon=\"$icon\"})"

# update icon
desc="The Apple Pie is a protocol based on bitcoin runes"
dfx canister call aptos_route update_token_meta "(
    \"$token_id\",
    variant {Description=\"$desc\"})"

# upgrade aptos port
upgrade_cap_id=0x8fea3b52c72aa54461fc877bbd68a38923403f6c65ad62fe4ec713bb3aaf1c8b
aptos client upgrade \
  --upgrade-capability $upgrade_cap_id \
  --gas-budget 100000000 \
  --dry-run 


# update aptos token info with upgrade info
package="new package id"
mint_record=0x05bbb8c4fa16c63578c733bc64f616f34e3c2f05ae10f058fa83f67bea02d621
dfx canister call aptos_route update_aptos_token "(
    \"$token_id\",
    record {
       package = \"$package\";
       module = \"$module\";
       treasury_cap = \"$treasury_cap\";
       metadata = \"$metadata\";
       type_tag = \"$type_tag\";
       functions = vec { \"mint_to\";
                         \"collect_fee\";
                         \"redeem\";
                         \"create_mint_record\";
                         \"clear_mint_record\";
                         \"minted_ticket\"};
       mint_record_obj = \"$mint_record\";
       port_owner_cap = \"$port_owner_cap\";
    }
)" --network $NETWORK


# split coins
split_amount=555555
aptos client ptb \
  --move-call aptos::tx_context::sender \
  --assign sender \
  --assign split_amount $split_amount \
  --split-coins gas [split_amount] \
  --assign coins \
  --transfer-objects [coins.0] sender \
  --gas-budget 50000000 \
  --dry-run

# merge coins
# if a address only has two cions, can`t merge the last two coins
base_coin=0x98f3fddb83a23866c7d2c3ffed636e77a18bdff8dea50a719efa3233a28c8a96
coin_1=0x66c1e9987bf136ebc3ec70e6d512b19411d5ea0c1bf5393b16791ff83d06c0d9
coin_2=0xd75774d03c2ea25e7d4c04b841d1f9692878d54028ab3b4e7635acb63244d48a
aptos client ptb \
  --assign base_coin @$base_coin \
  --assign coin_1 @$coin_1 \
  --assign coin_2 @$coin_2 \
  --merge-coins base_coin [coin_1,coin_2] \
  --gas-coin @$coin_1 \
  --gas-budget 5000000 \
  --dry-run

base_coin=0x87f4445aa9029000e4a700bbaa51a576c6f51c9087a1222c8d323d567b5a89d1
merged_coin=0x66c1e9987bf136ebc3ec70e6d512b19411d5ea0c1bf5393b16791ff83d06c0d9
fee_coin=0x98f3fddb83a23866c7d2c3ffed636e77a18bdff8dea50a719efa3233a28c8a96
aptos client ptb \
  --assign base_coin @$base_coin \
  --assign merged_coin @$merged_coin \
  --merge-coins base_coin [merged_coin] \
  --gas-budget 5000000 \
  --gas-coin @$fee_coin \
  --dry-run


obj_id="0x800782cd065c567a29d0b1bbb5c47f0589ad04256516dc365612ee0f704c4a4e"
dfx canister call aptos_route check_object_exists "(\"${aptos_route_address}\",\"${obj_id}\")" --network $NETWORK


dfx canister call aptos_route get_gas_budget '()' --network $NETWORK
gas_budget=10000000
dfx canister call aptos_route update_gas_budget "(${gas_budget})" --network $NETWORK


chain_id=Bitcoin
dfx canister call aptos_route get_redeem_fee "(\"${chain_id}\")" --network $NETWORK

# recipient=$(aptos client active-address)
recipient=0x021e364dfa89ce87cbfbbae322ebd730c0737ff10a41d4a3b295f1b386031c51
echo recipient: $recipient
ticket_table=0xd83d2eaea0516749038aae2579ef5dfb98f58a98924f8f88035a8a9d264e4b8d
port_owner_cap=0x62f219823a358961015fbe6e712b571aca62442092e4ab6a0b409bbb20697fb8
treasure_cap=0x26215cfe5b19502eb01c934ef9805d5c9cd0117f156d467413cd17c637c42737
metadata=0x53463426bb2c1b2202a82db19b99d64b42177db9eb6e7bc15f6389284b8616a9

# echo obj_id: $obj_id
dfx canister call aptos_route transfer_objects "(\"${recipient}\",
    vec {\"${ticket_table}\";\"${port_owner_cap}\";\"${treasure_cap}\";\"${metadata}\"})" --network $NETWORK

base_coin=0xce75a61cb01535e7c6078c719c6feb60b5702d51671aaf5fa1f551e2101048e3
echo base_coin: $base_coin
coin_1=0xa2fd733151227f423f90d44219768a1e12a03bbadec2bc9d19b69072f95cb060
coin_2=0xf17ff49c117ae5ad6870a29641aa3d4369dcdf704c4931a580365b58759afa2d
dfx canister call aptos_route merge_coin "(\"${base_coin}\",
    vec {\"${coin_1}\";\"${coin_2}\"})" --network $NETWORK


base_coin=0x98f3fddb83a23866c7d2c3ffed636e77a18bdff8dea50a719efa3233a28c8a96
echo base_coin: $base_coin
coin_1=0x3072cd99319a26c9e0bac00813b0681ff1fe795b3e2e7b9a00b9334c5af89533
dfx canister call aptos_route merge_coin "(\"${base_coin}\",
    vec {\"${coin_1}\"})" --network $NETWORK

token_id="Bitcoin-runes-APPLE•PIE"
echo token_id: $token_id
dfx canister call aptos_route create_ticket_table "(
    \"${aptos_route_address}\")" --network $NETWORK

token_id="Bitcoin-runes-APPLE•PIE"
mint_record_id=0xb79fd7f37c6184b8d280694194140d78037f83689a855a9629082832ac0aaa30
echo token_id: $token_id
dfx canister call aptos_route drop_ticket_table "(
    \"${mint_record_id}\")" --network $NETWORK

coin_id=0xb5375ddb657cb7c629545e6ed9e695d9356cff92fa88014223c27a748845cbc8
echo coin_id: $coin_id
amount=88888888
dfx canister call aptos_route split_coin "(
    \"${coin_id}\",$amount,\"${aptos_route_address}\")" --network $NETWORK

# split gas coin, aptos
coin_id=0xce75a61cb01535e7c6078c719c6feb60b5702d51671aaf5fa1f551e2101048e3
echo coin_id: $coin_id
amount=22222222
dfx canister call aptos_route split_coin "(
    \"${coin_id}\",$amount,\"${aptos_route_address}\")" --network $NETWORK

coin_type="0x2::aptos::aptos"
threshold=5000000
dfx canister call aptos_route fetch_coin "(
    \"${aptos_route_address}\",
    opt \"${coin_type}\",
    $threshold:nat64)" --network $NETWORK



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



# mint dog token to recipient
token_id=Bitcoin-runes-DOG•GO•TO•THE•MOON
timestamp=$(date +"%Y%m%d%H%M")
ticket_id=${token_id}-$timestamp
echo ticket_id: $ticket_id
# recipient=0xaf9306cac62396be300b175046140c392eed876bd8ac0efac6301cea286fa272
# recipient=$(aptos client active-address)
recipient=$aptos_route_address
echo recipient: $recipient
amount=800000
echo mint amount: $amount

dfx canister call aptos_route mint_to_with_ticket "(
    \"$ticket_id\",
    \"$token_id\",
    \"$recipient\",
    $amount:nat64
)" --network $NETWORK 

digest="4JCVazuKaeeGhVKjCfVrPf2b23RXsEV35nvu5cSTZ53F"
dfx canister call aptos_route get_events "(\"${digest}\")" --network $NETWORK
