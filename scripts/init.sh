#!/usr/bin/env bash

echo "*** Initializing WASM build environment"

if [ -z $CI_PROJECT_NAME ] ; then
   rustup update stable
   rustup install nightly-2022-02-09
   rustup default nightly-2022-02-09
fi

rustup target add wasm32-unknown-unknown --toolchain nightly-2022-02-09