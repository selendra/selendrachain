# Cumulus :cloud:

A set of tools for writing [Substrate](https://substrate.dev/)-based
[Polkadot](https://wiki.polkadot.network/en/)
[parachains](https://wiki.polkadot.network/docs/en/learn-parachains). Refer to the included
[overview](docs/overview.md) for architectural details, and the
[Cumulus workshop](https://substrate.dev/cumulus-workshop) for a hand-holding walkthrough
of using these tools.

It's easy to write blockchains using Substrate, and the overhead of writing parachains'
distribution, p2p, database, and synchronization layers should be just as low. This project aims to
make it easy to write parachains for Polkadot by leveraging the power of Substrate.

Cumulus clouds are shaped sort of like dots; together they form a system that is intricate,
beautiful and functional.

## Consensus

[`cumulus-consensus`](consensus) is a
[consensus engine](https://substrate.dev/docs/en/knowledgebase/advanced/consensus) for Substrate
that follows a Polkadot
[relay chain](https://wiki.polkadot.network/docs/en/learn-architecture#relay-chain). This will run a
Polkadot node internally, and dictate to the client and synchronization algorithms which chain to
follow,
[finalize](https://wiki.polkadot.network/docs/en/learn-consensus#probabilistic-vs-provable-finality),
and treat as best.
