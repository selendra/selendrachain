#!/usr/bin/env bash

echo "*** Initializing WASM build environment"

if [ -z $CI_PROJECT_NAME ] ; then
   rustup update stable
   rustup install nightly-2021-11-07
   rustup default nightly-2021-11-07
fi

rustup target add wasm32-unknown-unknown --toolchain nightly-2021-11-07
