#Created :  N/A
#Modifide:  15-MAR-2021
#!/usr/bin/env bash

set -e

echo "*** Initializing WASM build environment"

if [ -z $CI_PROJECT_NAME ] ; then
   # rustup update nightly
   # rustup update stable
#current version of Rust run on stable version 
   rustup default stable
fi

# rustup target add wasm32-unknown-unknown --toolchain nightly

# Install wasm-gc. It's useful for stripping slimming down wasm binaries.
# command -v wasm-gc || \
# 	cargo +nightly install --git https://github.com/alexcrichton/wasm-gc --force
