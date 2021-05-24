# Selendra Node &middot; [![GitHub license](https://img.shields.io/badge/license-GPL3%2FApache2-blue)](LICENSE-APACHE2) [![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](docs/CONTRIBUTING.adoc)

Codebase for Selendra a multi-use cases blockchain super-app for the Internet 2.0

A special solution for identity management, ownership of assets distribution & management, decentralized e-commerce, finance, decentralize computing and storage, and IoT applications and more.

### Build

Once the development environment is set up, build the node template. This command will build the
[Wasm](https://substrate.dev/docs/en/knowledgebase/advanced/executor#wasm-execution) and
[native](https://substrate.dev/docs/en/knowledgebase/advanced/executor#native-execution) code:

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

## Run

We'll start Alice's substrate node first on default TCP port 30333 with her chain database stored locally at /tmp/alice. The bootnode ID of her node is QmRpheLN4JWdAnY7HGJfWFNbfkQCb6tFf4vvA6hgjMZKrR, which is generated from the --node-key value that we specify below:

```
./target/release/selendra \
  --base-path /tmp/alice \
  --chain selendra-local \
  --alice \
  --port 30333 \
  --ws-port 9944 \
  --rpc-port 9933 \
  --node-key 0000000000000000000000000000000000000000000000000000000000000001 \
  --telemetry-url 'wss://telemetry.polkadot.io/submit/ 0' \
  --validator
```

In the second terminal, we'll start Bob's substrate node on a different TCP port of 30334, and with his chain database stored locally at /tmp/bob. We'll specify a value for the --bootnodes option that will connect his node to Alice's bootnode ID on TCP port 30333:

```
./target/release/selendra \
  --base-path /tmp/bob \
  --chain selendra-local \
  --bob \
  --port 30334 \
  --ws-port 9945 \
  --rpc-port 9934 \
  --telemetry-url 'wss://telemetry.polkadot.io/submit/ 0' \
  --validator \
  --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp
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

Or, start a dev chain with detailed logging:

```bash
RUST_LOG=debug RUST_BACKTRACE=1 ./target/release/selendra -lruntime=debug --dev
```

### Multi-Node Local Testnet

If you want to see the multi-node consensus algorithm in action, refer to
[our Start a Private Network tutorial](https://substrate.dev/docs/en/tutorials/start-a-private-network/).

### Validate on Testnet

```
  ./target/release/selendra \
  --base-path <Path to store chian db> \
  --chain ./indraSpecRaw.json \
  --port 30334 \
  --ws-port 9945 \
  --rpc-port 9934 \
  --telemetry-url 'wss://telemetry.polkadot.io/submit/ 0' \
  --validator \
  --rpc-methods=Unsafe \
  --name <Node Name> \
  --bootnodes /ip4/<IP Address>/tcp/<Port>/p2p/<Peer ID>
```

### Run in Docker

First, install [Docker](https://docs.docker.com/get-docker/) and
[Docker Compose](https://docs.docker.com/compose/install/).

Then run the following command to start a single node development chain.

```bash
./scripts/docker_run.sh
```

This command will firstly compile your code, and then start a local development network. You can
also replace the default command (`cargo build --release && ./target/release/node-selendra --dev --ws-external`)
by appending your own. A few useful ones are as follow.

```bash
# Run selendra node without re-compiling
./scripts/docker_run.sh ./target/release/node-selendra --dev --ws-external

# Purge the local dev chain
./scripts/docker_run.sh ./target/release/node-selendra purge-chain --dev

# Check whether the code is compilable
./scripts/docker_run.sh cargo check
```

### Run Benchmarks
```bash
$ cargo run --release --features=runtime-benchmarks -- benchmark --chain=selendra-dev --steps=50 --repeat=20 --pallet=<frame_system> --extrinsic=* --execution=wasm --wasm-execution=compiled --heap-pages=4096 --header=./file_header.txt --output=./runtime/selendra/src/weights/

```

## License

selendra is implement from [Polkadot](https://github.com/paritytech/polkadot.git) under license [GPL 3.0 licensed](LICENSE-GPL3).
