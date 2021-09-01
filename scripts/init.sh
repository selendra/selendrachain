#!/usr/bin/env bash

set -e
echo "---> Initializing Rustup environment"
echo "---> Detect OS environment"
arch=$(uname -m)
kernel=$(uname -r)
if [ -n "$(command -v lsb_release)" ]; then
	distroname=$(lsb_release -s -d)
    echo "You are running ${distroname} OS environment"
    # check for root
    SUDO_PREFIX=''
    if [[ $EUID -ne 0 ]]; then
        echo "Running as sudo"
        SUDO_PREFIX='sudo'
    fi
    $SUDO_PREFIX pacman -Syu
    $SUDO_PREFIX pacman -Syu --needed --noconfirm cmake gcc openssl-1.0 pkgconf git clang
    export OPENSSL_LIB_DIR="/usr/lib/openssl-1.0"
    export OPENSSL_INCLUDE_DIR="/usr/include/openssl-1.0"
elif [ -f "/etc/os-release" ]; then
	distroname=$(grep PRETTY_NAME /etc/os-release | sed 's/PRETTY_NAME=//g' | tr -d '="')
    echo "this is ${distroname}"
elif [ -f "/etc/debian_version" ]; then
	distroname="Debian $(cat /etc/debian_version)"
    SUDO_PREFIX=''
    if [[ $EUID -ne 0 ]]; then
        echo "Running apt as sudo"
        SUDO_PREFIX='sudo'
    fi
    $SUDO_PREFIX apt update
    $SUDO_PREFIX apt install -y build-essential cmake pkg-config libssl-dev openssl git clang libclang-dev
elif [ -f "/etc/redhat-release" ]; then
	distroname=$(cat /etc/redhat-release)
else
	distroname="$(uname -s) $(uname -r)"
fi 

if [ "$OSTYPE" == "darwin"* ]; then
    echo "Found macbook"
    brew install cmake pkg-config openssl git llvm
fi

if [[ $(cargo --version) ]]; then
    echo "Found cargo"
else
    curl https://sh.rustup.rs -sSf | sh -s -- -y
    source $HOME/.cargo/env
    export PATH=$HOME/.cargo/bin:$PATH
fi

echo "*** Initializing WASM build environment"

if [ -z $CI_PROJECT_NAME ] ; then
   rustup update stable
   rustup install nightly-x86_64-unknown-linux-gnu
   rustup install nightly-2021-08-02
   rustup default nightly-2021-08-02
fi

rustup target add wasm32-unknown-unknown --toolchain nightly-2021-08-02

# Install wasm-gc. It's useful for stripping slimming down wasm binaries.
command -v wasm-gc || \
	cargo +nightly install --git https://github.com/alexcrichton/wasm-gc --force
