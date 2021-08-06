#!/usr/bin/env bash

set -e

echo "*** Initializing WASM build environment"

if [ -z $CI_PROJECT_NAME ] ; then
   rustup update stable
   rustup install nightly-2021-08-02
   rustup default nightly-2021-08-02
fi

rustup target add wasm32-unknown-unknown --toolchain nightly-2021-08-02

# Install wasm-gc. It's useful for stripping slimming down wasm binaries.
command -v wasm-gc || \
	cargo +nightly install --git https://github.com/alexcrichton/wasm-gc --force