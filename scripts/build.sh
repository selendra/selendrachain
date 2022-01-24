#!/usr/bin/env bash
set -e
echo "---> Initializing Build Selendra"

echo '[+] Build Selendra'
cargo build --release

echo '[+] Build Selendra chainspec'
./target/release/selendra build-spec --chain=selendra-staging --disable-default-bootnode --raw > node/service/res/selendra.json