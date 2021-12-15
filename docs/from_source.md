# Using source
Once the development environment is set up, build the node template. This command will build the [Wasm](https://substrate.dev/docs/en/knowledgebase/advanced/executor#wasm-execution) and [native](https://substrate.dev/docs/en/knowledgebase/advanced/executor#native-execution) code:

## Setup eviroment for build selendra
Install the necessary dependencies for compiling and running the Selendra node software.
#### For Debian
```
sudo apt update
sudo apt install make clang pkg-config libssl-dev build-essential
```

#### For ArchLinux
```
sudo pacman -Syu
sudo pacman -Syu --needed --noconfirm cmake gcc openssl-1.0 pkgconf git clang

export OPENSSL_LIB_DIR="/usr/lib/openssl-1.0"
export OPENSSL_INCLUDE_DIR="/usr/include/openssl-1.0"
```

#### For MacOs
```
brew install cmake pkg-config openssl git llvm
```

Install Rust:

```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Initialize your Wasm Build environment:
```
./scripts/init.sh
```

Build Wasm and native code:
```bash
cargo build --release
```

### Single Node Development Chain
Purge any existing dev chain state:

```bash
./target/release/selendra purge-chain --dev
```

Start a dev chain:
```bash
./target/release/selendra --dev
```

## Run Alice and Bob Start Blockchain

We'll start Alice's substrate node first on default TCP port 30333 with her chain database stored locally at /tmp/alice. The bootnode ID of her node is QmRpheLN4JWdAnY7HGJfWFNbfkQCb6tFf4vvA6hgjMZKrR, which is generated from the --node-key value that we specify below:

Purge any existing Alice chain state:

```bash
./target/release/selendra purge-chain --base-path /tmp/alice --chain local
```

Start Alice node
```
./target/release/selendra \
--base-path /tmp/alice \
--chain local \
--alice \
--port 30333 \
--ws-port 9944 \
--rpc-port 9933 \
--node-key 0000000000000000000000000000000000000000000000000000000000000001 \
--telemetry-url 'wss://telemetry.polkadot.io/submit/ 0' \
--validator
```

In the second terminal, we'll start Bob's substrate node on a different TCP port of 30334, and with his chain database stored locally at /tmp/bob. We'll specify a value for the --bootnodes option that will connect his node to Alice's bootnode ID on TCP port 30333:

Purge any existing Bob chain state:

```bash
./target/release/selendra purge-chain --base-path /tmp/bob --chain local
```

Start Bob node
```
./target/release/selendra \
--base-path /tmp/bob \
--chain local \
--bob \
--port 30334 \
--ws-port 9945 \
--rpc-port 9934 \
--telemetry-url 'wss://telemetry.polkadot.io/submit/ 0' \
--validator \
--bootnodes /ip4/127.0.0.1/tcp/30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp
```
### Multi-Node Local Testnet

If you want to see the multi-node consensus algorithm in action, refer to

[Substrate start a private Network tutorial](https://substrate.dev/docs/en/tutorials/start-a-private-network/).