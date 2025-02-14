#!/usr/bin/env bash


export DFX_WARNING="-mainnet_plaintext_identity"
CANISTER=aptos_route
CANISTER_WASM=target/wasm32-unknown-unknown/release/$CANISTER.wasm

# Build the canister
cargo build --release --target wasm32-unknown-unknown --package $CANISTER

# Extract the did file
echo "extractor did file ..."
candid-extractor $CANISTER_WASM > ./assets/$CANISTER.did

# dfx canister create aptos_route
dfx build aptos_route
cp ./.dfx/local/canisters/aptos_route/aptos_route.wasm.gz ./assets/aptos_route.wasm.gz
cp ./.dfx/local/canisters/aptos_route/service.did ./assets/aptos_route.did
cp ./.dfx/local/canisters/aptos_route/service.did.d.ts ./assets/service.did.d.ts
cp ./.dfx/local/canisters/aptos_route/service.did.js ./assets/service.did.js

echo "Build done !"
