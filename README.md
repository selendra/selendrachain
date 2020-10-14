# Selendra Node &middot; [![GitHub license](https://img.shields.io/badge/license-GPL3%2FApache2-blue)](LICENSE) [![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](docs/CONTRIBUTING.adoc)

<p align="center">
  <img src="/docs/media/selendra.png">
</p>


Codebase for Selendra a multi-use cases blockchain super-app for the Internet 2.0

A special solution for identity management, ownership of assets distribution & management, decentralized e-commerce, finance, decentralize computing and storage, and IoT applications and more.

### Build

Once the development environment is set up, build the node template. This command will build the
[Wasm](https://substrate.dev/docs/en/knowledgebase/advanced/executor#wasm-execution) and
[native](https://substrate.dev/docs/en/knowledgebase/advanced/executor#native-execution) code:

Install Rust:

```
curl https://sh.rustup.rs -sSf | sh
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
./target/release/node-indracore \
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

```
./target/release/node-indracore \
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

### Single Node Development Chain

Purge any existing dev chain state:

```bash
./target/release/node-indracore purge-chain --dev
```

Start a dev chain:

```bash
./target/release/node-indracore --dev
```

Or, start a dev chain with detailed logging:

```bash
RUST_LOG=debug RUST_BACKTRACE=1 ./target/release/node-indracore -lruntime=debug --dev
```

### Multi-Node Local Testnet

If you want to see the multi-node consensus algorithm in action, refer to
[our Start a Private Network tutorial](https://substrate.dev/docs/en/tutorials/start-a-private-network/).

### Validate on Testnet
```
  ./target/release/node-indracore \
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
also replace the default command (`cargo build --release && ./target/release/node-indracore --dev --ws-external`)
by appending your own. A few useful ones are as follow.

```bash
# Run Indracore node without re-compiling
./scripts/docker_run.sh ./target/release/node-indracore --dev --ws-external

# Purge the local dev chain
./scripts/docker_run.sh ./target/release/node-indracore purge-chain --dev

# Check whether the code is compilable
./scripts/docker_run.sh cargo check
```


## Contributions & Code of Conduct

Please follow the contributions guidelines as outlined in [`docs/CONTRIBUTING.adoc`](docs/CONTRIBUTING.adoc). In all communications and contributions, this project follows the [Contributor Covenant Code of Conduct](docs/CODE_OF_CONDUCT.md).

## Security

The security policy and procedures can be found in [`docs/SECURITY.md`](docs/SECURITY.md).

## License

- Substrate Primitives (`sp-*`), Frame (`frame-*`) and the pallets (`pallets-*`), binaries (`/bin`) and all other utilities are licensed under [Apache 2.0](LICENSE-APACHE2).
- Substrate Client (`/client/*` / `sc-*`) is licensed under [GPL v3.0 with a classpath linking exception](LICENSE-GPL3).

The reason for the split-licensing is to ensure that for the vast majority of teams using Substrate to create feature-chains, then all changes can be made entirely in Apache2-licensed code, allowing teams full freedom over what and how they release and giving licensing clarity to commercial teams.

In the interests of the community, we require any deeper improvements made to Substrate's core logic (e.g. Substrate's internal consensus, crypto or database code) to be contributed back so everyone can benefit.
