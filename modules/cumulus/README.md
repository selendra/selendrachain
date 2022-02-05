# Cumulus :cloud:

A set of tools for writing [Substrate](https://substrate.io/)-based
[Selendra](https://wiki.selendra.network/en/)
[parachains](https://wiki.selendra.network/docs/en/learn-parachains). Refer to the included
[overview](docs/overview.md) for architectural details, and the
[Cumulus tutorial](https://docs.substrate.io/tutorials/v3/cumulus/start-relay) for a
guided walk-through of using these tools.

It's easy to write blockchains using Substrate, and the overhead of writing parachains'
distribution, p2p, database, and synchronization layers should be just as low. This project aims to
make it easy to write parachains for Selendra by leveraging the power of Substrate.

Cumulus clouds are shaped sort of like dots; together they form a system that is intricate,
beautiful and functional.

## Consensus

[`parachain-consensus`](https://github.com/paritytech/cumulus/blob/master/client/consensus/common/src/parachain_consensus.rs) is a
[consensus engine](https://docs.substrate.io/v3/advanced/consensus) for Substrate
that follows a Selendra
[relay chain](https://wiki.selendra.network/docs/en/learn-architecture#relay-chain). This will run
a Selendra node internally, and dictate to the client and synchronization algorithms which chain
to follow,
[finalize](https://wiki.selendra.network/docs/en/learn-consensus#probabilistic-vs-provable-finality),
and treat as best.

## Collator

A Selendra [collator](https://wiki.selendra.network/docs/en/learn-collator) for the parachain is
implemented by the `selendra-collator` binary.

# Indracore 🪙

This repository also contains the Indracore runtime (as well as the canary runtime Indracore and the
test runtime Indranet).
Indracore is a common good parachain providing an asset store for the Selendra ecosystem.

## Build & Launch a Node

To run a Indracore or Indranet node (Indracore is not deployed, yet) you will need to compile the
`selendra-collator` binary:

```bash
cargo build --release --locked -p selendra-collator
```

Once the executable is built, launch the parachain node via:

```bash
CHAIN=indranet # or indracore
./target/release/selendra-collator --chain $CHAIN
```

Refer to the [setup instructions below](#local-setup) to run a local network for development.

# Rococo :crown:

[Rococo](https://selendra.js.org/apps/?rpc=wss://rococo-rpc.selendra.io) is becoming a [Community Parachain Testbed](https://selendra.network/blog/rococo-revamp-becoming-a-community-parachain-testbed/) for parachain teams in the Selendra ecosystem. It supports multiple parachains with the differentiation of long-term connections and recurring short-term connections, to see which parachains are currently connected and how long they will be connected for [see here](https://selendra.js.org/apps/?rpc=wss%3A%2F%2Frococo-rpc.selendra.io#/parachains).

Rococo is an elaborate style of design and the name describes the painstaking effort that has gone
into this project.

## Build & Launch Rococo Collators

Collators are similar to validators in the relay chain. These nodes build the blocks that will
eventually be included by the relay chain for a parachain.

To run a Rococo collator you will need to compile the following binary:

```bash
cargo build --release --locked -p selendra-collator
```

Otherwise you can compile it with
[Parity CI docker image](https://github.com/paritytech/scripts/tree/master/dockerfiles/ci-linux):

```bash
docker run --rm -it -w /shellhere/cumulus \
                    -v $(pwd):/shellhere/cumulus \
                    paritytech/ci-linux:production cargo build --release --locked -p selendra-collator
sudo chown -R $(id -u):$(id -g) target/
```

If you want to reproduce other steps of CI process you can use the following
[guide](https://github.com/paritytech/scripts#gitlab-ci-for-building-docker-images).

Once the executable is built, launch collators for each parachain (repeat once each for chain
`tick`, `trick`, `track`):

```bash
./target/release/selendra-collator --chain $CHAIN --validator
```

## Parachains

- [Canvas - WASM Smart Contract](https://selendra.js.org/apps/?rpc=wss%3A%2F%2Frococo-canvas-rpc.selendra.io#/explorer)

The network uses horizontal message passing (HRMP) to enable communication between parachains and
the relay chain and, in turn, between parachains. This means that every message is sent to the relay
chain, and from the relay chain to its destination parachain.

## Local Setup

Launch a local setup including a Relay Chain and a Parachain.

### Launch the Relay Chain

```bash
# Compile Selendra with the real overseer feature
git clone https://github.com/paritytech/selendra
cargo build --release

# Generate a raw chain spec
./target/release/selendra build-spec --chain rococo-local --disable-default-bootnode --raw > rococo-local-cfde.json

# Alice
./target/release/selendra --chain rococo-local-cfde.json --alice --tmp

# Bob (In a separate terminal)
./target/release/selendra --chain rococo-local-cfde.json --bob --tmp --port 30334
```

### Launch the Parachain

```bash
# Compile
git clone https://github.com/paritytech/cumulus
cargo build --release

# Export genesis state
# --parachain-id 200 as an example that can be chosen freely. Make sure to everywhere use the same parachain id
./target/release/selendra-collator export-genesis-state --parachain-id 200 > genesis-state

# Export genesis wasm
./target/release/selendra-collator export-genesis-wasm > genesis-wasm

# Collator1
./target/release/selendra-collator --collator --alice --force-authoring --tmp --parachain-id <parachain_id_u32_type_range> --port 40335 --ws-port 9946 -- --execution wasm --chain ../selendra/rococo-local-cfde.json --port 30335

# Collator2
./target/release/selendra-collator --collator --bob --force-authoring --tmp --parachain-id <parachain_id_u32_type_range> --port 40336 --ws-port 9947 -- --execution wasm --chain ../selendra/rococo-local-cfde.json --port 30336

# Parachain Full Node 1
./target/release/selendra-collator --tmp --parachain-id <parachain_id_u32_type_range> --port 40337 --ws-port 9948 -- --execution wasm --chain ../selendra/rococo-local-cfde.json --port 30337
```

### Register the parachain

![image](https://user-images.githubusercontent.com/2915325/99548884-1be13580-2987-11eb-9a8b-20be658d34f9.png)

## Containerize

After building `selendra-collator` with cargo or with Parity CI image as documented in [this chapter](#build--launch-rococo-collators),
the following will allow producing a new docker image where the compiled binary is injected:

```bash
./docker/scripts/build-injected-image.sh
```

Alternatively, you can build an image with a builder pattern:

```bash
docker build --tag $OWNER/$IMAGE_NAME --file ./docker/selendra-collator_builder.Containerfile .

You may then run your new container:

```bash
docker run --rm -it $OWNER/$IMAGE_NAME --collator --tmp --parachain-id 1000 --execution wasm --chain /specs/indranet.json
```
